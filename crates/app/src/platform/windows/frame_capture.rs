//! Timestamped Windows Graphics Capture samples for playback-backed materials.

use riina_core::CapturedFrame;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use windows::{
    Foundation::TypedEventHandler,
    Graphics::{
        Capture::{
            Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem,
            GraphicsCaptureSession,
        },
        DirectX::{Direct3D11::IDirect3DDevice, DirectXPixelFormat},
        SizeInt32,
    },
    Win32::{
        Foundation::{HMODULE, HWND, POINT, RECT},
        Graphics::{
            Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0},
            Direct3D11::{
                D3D11_BOX, D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ,
                D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC,
                D3D11_USAGE_STAGING, D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext,
                ID3D11Texture2D,
            },
            Dxgi::{Common::DXGI_FORMAT_B8G8R8A8_UNORM, IDXGIAdapter, IDXGIDevice},
            Gdi::ClientToScreen,
        },
        System::WinRT::{
            Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess},
            Graphics::Capture::IGraphicsCaptureItemInterop,
            RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize,
        },
        UI::WindowsAndMessaging::{GetClientRect, GetWindowRect},
    },
    core::{IInspectable, Interface, factory},
};

// Read back at up to 30 Hz while the Windows video-backed material is visible.
// The bounded 320×180 WGC sample follows compositor updates without increasing
// the separate, more expensive mpv screenshot fallback cadence.
const CAPTURE_INTERVAL: Duration = Duration::from_millis(33);
const MAX_CAPTURE_DIMENSION: u32 = 16_384;
const MATERIAL_SAMPLE_SIZE: (u32, u32) = (320, 180);

#[derive(Clone)]
pub(crate) struct MaterialFrameSample {
    pub(crate) sequence: u64,
    pub(crate) frame: Arc<CapturedFrame>,
    pub(crate) captured_at: Instant,
    compositor_time: Option<i64>,
}

struct CaptureState {
    enabled: AtomicBool,
    epoch: AtomicU64,
    sequence: AtomicU64,
    last_capture: Mutex<Instant>,
    latest: Mutex<Option<MaterialFrameSample>>,
    failed_readback_logged: AtomicBool,
}

impl CaptureState {
    fn new() -> Self {
        Self {
            enabled: AtomicBool::new(false),
            epoch: AtomicU64::new(0),
            sequence: AtomicU64::new(0),
            last_capture: Mutex::new(Instant::now() - CAPTURE_INTERVAL),
            latest: Mutex::new(None),
            failed_readback_logged: AtomicBool::new(false),
        }
    }
}

struct CaptureOwner {
    state: Arc<CaptureState>,
    shutdown: Mutex<Option<Sender<()>>>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl Drop for CaptureOwner {
    fn drop(&mut self) {
        if let Ok(mut shutdown) = self.shutdown.lock()
            && let Some(shutdown) = shutdown.take()
        {
            let _ = shutdown.send(());
        }
        if let Ok(mut worker) = self.worker.lock()
            && let Some(worker) = worker.take()
        {
            let _ = worker.join();
        }
    }
}

#[derive(Clone)]
pub(crate) struct FrameCapture(Arc<CaptureOwner>);

impl FrameCapture {
    pub(crate) fn start(hwnd: HWND) -> Result<Self, String> {
        let state = Arc::new(CaptureState::new());
        let (shutdown_sender, shutdown_receiver) = mpsc::channel();
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let worker_state = state.clone();
        let hwnd = hwnd.0 as isize;
        let worker = thread::Builder::new()
            .name("riina-wgc-material".into())
            .spawn(move || {
                capture_worker(
                    HWND(hwnd as *mut _),
                    worker_state,
                    shutdown_receiver,
                    ready_sender,
                )
            })
            .map_err(|error| format!("Could not start Windows video capture: {error}"))?;

        match ready_receiver.recv_timeout(Duration::from_secs(8)) {
            Ok(Ok(())) => Ok(Self(Arc::new(CaptureOwner {
                state,
                shutdown: Mutex::new(Some(shutdown_sender)),
                worker: Mutex::new(Some(worker)),
            }))),
            Ok(Err(error)) => {
                let _ = shutdown_sender.send(());
                let _ = worker.join();
                Err(error)
            }
            Err(error) => {
                let _ = shutdown_sender.send(());
                let _ = worker.join();
                Err(format!("Windows video capture did not initialize: {error}"))
            }
        }
    }

    pub(crate) fn set_enabled(&self, enabled: bool) {
        let previous = self.0.state.enabled.swap(enabled, Ordering::AcqRel);
        if previous != enabled {
            // A readback may be in flight while the surface is hidden or
            // re-enabled. Give each visibility period its own publish epoch.
            self.0.state.epoch.fetch_add(1, Ordering::AcqRel);
            if !enabled && let Ok(mut latest) = self.0.state.latest.lock() {
                *latest = None;
            }
        }
    }

    pub(crate) fn clear_latest(&self) {
        // Prevent a frame from the previous media generation being published
        // after this clear while its GPU readback is still in progress.
        self.0.state.epoch.fetch_add(1, Ordering::AcqRel);
        if let Ok(mut latest) = self.0.state.latest.lock() {
            *latest = None;
        }
    }

    pub(crate) fn latest(&self) -> Option<MaterialFrameSample> {
        self.0.state.latest.lock().ok()?.clone()
    }
}

struct ReadbackContext {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    staging: Option<(u32, u32, ID3D11Texture2D)>,
}

// D3D11 device/context interfaces are not apartment-bound. The immediate
// context is accessed only through the mutex containing this owner.
unsafe impl Send for ReadbackContext {}

struct AgileCaptureDevice(IDirect3DDevice);

// The WinRT Direct3D device is agile and may be used by the free-threaded
// frame-pool callback on another thread.
unsafe impl Send for AgileCaptureDevice {}

impl AgileCaptureDevice {
    fn recreate(
        &self,
        pool: &Direct3D11CaptureFramePool,
        format: DirectXPixelFormat,
        size: SizeInt32,
    ) -> windows::core::Result<()> {
        pool.Recreate(&self.0, format, 2, size)
    }
}

struct ActiveCapture {
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    frame_arrived_token: i64,
}

impl ActiveCapture {
    fn close(self) {
        let _ = self.pool.RemoveFrameArrived(self.frame_arrived_token);
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}

fn capture_worker(
    hwnd: HWND,
    state: Arc<CaptureState>,
    shutdown: Receiver<()>,
    ready: mpsc::SyncSender<Result<(), String>>,
) {
    // The frame-pool callback is free-threaded, so its initialization and WinRT
    // object lifetime belong to a dedicated MTA thread rather than Slint's UI.
    if let Err(error) = unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
        let _ = ready.send(Err(format!(
            "Could not initialize Windows Runtime: {error}"
        )));
        return;
    }

    match start_capture(hwnd, state) {
        Ok(capture) => {
            let _ = ready.send(Ok(()));
            let _ = shutdown.recv();
            capture.close();
        }
        Err(error) => {
            let _ = ready.send(Err(error));
        }
    }
    unsafe { RoUninitialize() };
}

fn start_capture(hwnd: HWND, state: Arc<CaptureState>) -> Result<ActiveCapture, String> {
    let mut device = None;
    let mut immediate_context = None;
    let feature_levels = [D3D_FEATURE_LEVEL_11_0];
    // SAFETY: all outputs point to initialized COM interface slots, and the
    // requested BGRA support is required for Windows Graphics Capture surfaces.
    unsafe {
        D3D11CreateDevice(
            None::<&IDXGIAdapter>,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&feature_levels),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut immediate_context),
        )
    }
    .map_err(|error| format!("Could not create a D3D11 capture device: {error}"))?;
    let device = device.ok_or("D3D11 did not return a capture device")?;
    let context = immediate_context.ok_or("D3D11 did not return an immediate context")?;
    let dxgi_device: IDXGIDevice = device
        .cast()
        .map_err(|error| format!("Could not query the capture DXGI device: {error}"))?;
    let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device) }
        .map_err(|error| format!("Could not expose the D3D11 device to WinRT: {error}"))?;
    let winrt_device: IDirect3DDevice = inspectable
        .cast()
        .map_err(|error| format!("Could not query the WinRT D3D11 device: {error}"))?;

    let interop: IGraphicsCaptureItemInterop =
        factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
            .map_err(|error| format!("Could not load Windows Graphics Capture interop: {error}"))?;
    let item: GraphicsCaptureItem = unsafe { interop.CreateForWindow(hwnd) }
        .map_err(|error| format!("Could not capture the native video window: {error}"))?;
    let size = item
        .Size()
        .map_err(|error| format!("Could not read the native video capture size: {error}"))?;
    validate_size(size)?;
    let pixel_format = DirectXPixelFormat::B8G8R8A8UIntNormalized;
    let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(&winrt_device, pixel_format, 2, size)
        .map_err(|error| format!("Could not create a free-threaded video frame pool: {error}"))?;
    let session = pool
        .CreateCaptureSession(&item)
        .map_err(|error| format!("Could not create the native video capture session: {error}"))?;
    // This capture reads only Riina's own window. Suppress Windows' capture
    // border where the platform supports doing so.
    let _ = session.SetIsBorderRequired(false);

    let pool_size = Arc::new(Mutex::new((size.Width, size.Height)));
    let readback = Arc::new(Mutex::new(ReadbackContext {
        device,
        context,
        staging: None,
    }));
    let callback_state = state;
    let callback_readback = readback;
    let callback_pool_size = pool_size;
    let callback_hwnd = hwnd.0 as isize;
    let callback_device = AgileCaptureDevice(winrt_device);
    let frame_arrived =
        TypedEventHandler::<Direct3D11CaptureFramePool, IInspectable>::new(move |sender, _| {
            let Some(pool) = sender.as_ref() else {
                return Ok(());
            };
            let Some(frame) = newest_frame(pool) else {
                return Ok(());
            };
            let content_size = match frame.ContentSize() {
                Ok(size) if size.Width > 0 && size.Height > 0 => size,
                _ => {
                    let _ = frame.Close();
                    return Ok(());
                }
            };
            if let Ok(mut pool_size) = callback_pool_size.lock()
                && *pool_size != (content_size.Width, content_size.Height)
            {
                let size = SizeInt32 {
                    Width: content_size.Width,
                    Height: content_size.Height,
                };
                if validate_size(size).is_ok()
                    && callback_device.recreate(pool, pixel_format, size).is_ok()
                {
                    *pool_size = (content_size.Width, content_size.Height);
                }
                let _ = frame.Close();
                return Ok(());
            }

            if !callback_state.enabled.load(Ordering::Acquire) {
                let _ = frame.Close();
                return Ok(());
            }
            let capture_epoch = callback_state.epoch.load(Ordering::Acquire);
            let mut last_capture = match callback_state.last_capture.lock() {
                Ok(last_capture) => last_capture,
                Err(_) => {
                    let _ = frame.Close();
                    return Ok(());
                }
            };
            if last_capture.elapsed() < CAPTURE_INTERVAL {
                let _ = frame.Close();
                return Ok(());
            }
            *last_capture = Instant::now();
            drop(last_capture);

            let captured_at = Instant::now();
            let compositor_time = frame.SystemRelativeTime().ok().map(|time| time.Duration);
            let readback_started = Instant::now();
            let capture_result =
                frame
                    .Surface()
                    .map_err(|error| error.to_string())
                    .and_then(|surface| {
                        let access: IDirect3DDxgiInterfaceAccess =
                            surface.cast().map_err(|error| error.to_string())?;
                        let source: ID3D11Texture2D =
                            unsafe { access.GetInterface() }.map_err(|error| error.to_string())?;
                        let mut readback = callback_readback
                            .lock()
                            .map_err(|_| "Capture readback state was poisoned".to_owned())?;
                        readback_sample(HWND(callback_hwnd as *mut _), &source, &mut readback)
                    });
            if std::env::var_os("RIINA_TRACE_MATERIAL").is_some() {
                eprintln!(
                    "Windows material WGC readback_ms={}",
                    readback_started.elapsed().as_millis(),
                );
            }
            let _ = frame.Close();
            match capture_result {
                Ok(frame) => {
                    if !callback_state.enabled.load(Ordering::Acquire)
                        || callback_state.epoch.load(Ordering::Acquire) != capture_epoch
                    {
                        return Ok(());
                    }
                    if let Ok(mut latest) = callback_state.latest.lock() {
                        let newer_than_published = latest.as_ref().is_none_or(|previous| {
                            match (compositor_time, previous.compositor_time) {
                                (Some(current), Some(previous)) => current > previous,
                                (Some(_), None) => true,
                                (None, Some(_)) => false,
                                (None, None) => captured_at > previous.captured_at,
                            }
                        });
                        if newer_than_published
                            && callback_state.enabled.load(Ordering::Acquire)
                            && callback_state.epoch.load(Ordering::Acquire) == capture_epoch
                        {
                            let sequence =
                                callback_state.sequence.fetch_add(1, Ordering::AcqRel) + 1;
                            *latest = Some(MaterialFrameSample {
                                sequence,
                                frame: Arc::new(frame),
                                captured_at,
                                compositor_time,
                            });
                        }
                    }
                }
                Err(error) => {
                    if !callback_state
                        .failed_readback_logged
                        .swap(true, Ordering::AcqRel)
                    {
                        eprintln!("Windows video material capture fell back to mpv: {error}");
                    }
                }
            }
            Ok(())
        });
    let frame_arrived_token = pool
        .FrameArrived(&frame_arrived)
        .map_err(|error| format!("Could not subscribe to video frames: {error}"))?;
    session
        .StartCapture()
        .map_err(|error| format!("Could not start native video capture: {error}"))?;
    Ok(ActiveCapture {
        pool,
        session,
        frame_arrived_token,
    })
}

fn newest_frame(pool: &Direct3D11CaptureFramePool) -> Option<Direct3D11CaptureFrame> {
    let mut newest = None;
    while let Ok(frame) = pool.TryGetNextFrame() {
        if let Some(previous) = newest.replace(frame) {
            let _ = previous.Close();
        }
    }
    newest
}

fn validate_size(size: SizeInt32) -> Result<(), String> {
    if size.Width <= 0
        || size.Height <= 0
        || size.Width as u32 > MAX_CAPTURE_DIMENSION
        || size.Height as u32 > MAX_CAPTURE_DIMENSION
    {
        Err("Windows returned invalid native video capture dimensions".into())
    } else {
        Ok(())
    }
}

fn readback_sample(
    hwnd: HWND,
    source: &ID3D11Texture2D,
    readback: &mut ReadbackContext,
) -> Result<CapturedFrame, String> {
    let mut description = D3D11_TEXTURE2D_DESC::default();
    // SAFETY: D3D11 writes the description of this live capture texture.
    unsafe { source.GetDesc(&mut description) };
    if description.Format != DXGI_FORMAT_B8G8R8A8_UNORM {
        return Err(format!(
            "Windows returned an unsupported video capture format {:?}",
            description.Format
        ));
    }
    let (left, top, width, height) = client_crop(hwnd, description.Width, description.Height)?;
    let staging = match &readback.staging {
        Some((staging_width, staging_height, texture))
            if *staging_width == width && *staging_height == height =>
        {
            texture.clone()
        }
        _ => {
            let description = D3D11_TEXTURE2D_DESC {
                Width: width,
                Height: height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
            };
            let mut texture = None;
            // SAFETY: the staging texture is CPU-readable, has no bind flags,
            // and exactly matches the source crop format and dimensions.
            unsafe {
                readback
                    .device
                    .CreateTexture2D(&description, None, Some(&mut texture))
            }
            .map_err(|error| format!("Could not allocate video sample readback: {error}"))?;
            let texture = texture.ok_or("D3D11 did not return a staging texture")?;
            readback.staging = Some((width, height, texture.clone()));
            texture
        }
    };
    let crop = D3D11_BOX {
        left,
        top,
        front: 0,
        right: left + width,
        bottom: top + height,
        back: 1,
    };
    // SAFETY: crop is within the live source texture and staging dimensions
    // match the selected source rectangle.
    unsafe {
        readback
            .context
            .CopySubresourceRegion(&staging, 0, 0, 0, 0, source, 0, Some(&crop));
    }
    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    // SAFETY: staging was created for CPU readback and no other callback maps it.
    unsafe {
        readback
            .context
            .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
    }
    .map_err(|error| format!("Could not map the captured video frame: {error}"))?;
    let sampled = downsample_mapped_bgra(
        width,
        height,
        mapped,
        MATERIAL_SAMPLE_SIZE.0,
        MATERIAL_SAMPLE_SIZE.1,
    );
    // SAFETY: this callback mapped the same staging subresource above.
    unsafe { readback.context.Unmap(&staging, 0) };
    sampled
}

fn downsample_mapped_bgra(
    width: u32,
    height: u32,
    mapped: D3D11_MAPPED_SUBRESOURCE,
    max_width: u32,
    max_height: u32,
) -> Result<CapturedFrame, String> {
    const MAX_DIMENSION: u32 = 16_384;
    const MAX_BYTES: usize = 512 * 1024 * 1024;
    if width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || max_width == 0
        || max_height == 0
    {
        return Err("Windows returned invalid video sample dimensions".into());
    }
    let row_bytes = (width as usize)
        .checked_mul(4)
        .ok_or("The captured video row is too large")?;
    let stride = mapped.RowPitch as usize;
    let source_len = stride
        .checked_mul(height as usize)
        .filter(|length| *length <= MAX_BYTES)
        .ok_or("The captured video sample is too large")?;
    if mapped.pData.is_null() || stride < row_bytes {
        return Err("D3D11 returned an invalid captured video buffer".into());
    }
    // SAFETY: Map keeps pData valid for RowPitch × Height bytes until Unmap;
    // the caller unmaps only after this function has copied the buffer.
    let source = unsafe { std::slice::from_raw_parts(mapped.pData.cast::<u8>(), source_len) };
    let scale = (max_width as f64 / f64::from(width))
        .min(max_height as f64 / f64::from(height))
        .min(1.0);
    let sample_width = (f64::from(width) * scale).round().max(1.0) as u32;
    let sample_height = (f64::from(height) * scale).round().max(1.0) as u32;
    let output_stride = (sample_width as usize)
        .checked_mul(4)
        .ok_or("The captured video sample is too large")?;
    let output_len = output_stride
        .checked_mul(sample_height as usize)
        .ok_or("The captured video sample is too large")?;
    if output_len > MAX_BYTES {
        return Err("The captured video sample is too large".into());
    }
    let mut rgba = vec![0_u8; output_len];
    let channel_offsets = [2, 1, 0, 3];
    for y in 0..sample_height {
        let source_y = ((f64::from(y) + 0.5) * f64::from(height) / f64::from(sample_height) - 0.5)
            .clamp(0.0, f64::from(height - 1));
        let y0 = source_y.floor() as usize;
        let y1 = (y0 + 1).min(height as usize - 1);
        let fy = source_y - y0 as f64;
        for x in 0..sample_width {
            let source_x = ((f64::from(x) + 0.5) * f64::from(width) / f64::from(sample_width)
                - 0.5)
                .clamp(0.0, f64::from(width - 1));
            let x0 = source_x.floor() as usize;
            let x1 = (x0 + 1).min(width as usize - 1);
            let fx = source_x - x0 as f64;
            let indices = [
                y0 * stride + x0 * 4,
                y0 * stride + x1 * 4,
                y1 * stride + x0 * 4,
                y1 * stride + x1 * 4,
            ];
            let destination = (y as usize * sample_width as usize + x as usize) * 4;
            for (channel, source_channel) in channel_offsets.iter().copied().enumerate() {
                let top = f64::from(source[indices[0] + source_channel]) * (1.0 - fx)
                    + f64::from(source[indices[1] + source_channel]) * fx;
                let bottom = f64::from(source[indices[2] + source_channel]) * (1.0 - fx)
                    + f64::from(source[indices[3] + source_channel]) * fx;
                rgba[destination + channel] = (top * (1.0 - fy) + bottom * fy).round() as u8;
            }
        }
    }
    Ok(CapturedFrame {
        width: sample_width,
        height: sample_height,
        rgba: rgba.into(),
    })
}

fn client_crop(
    hwnd: HWND,
    source_width: u32,
    source_height: u32,
) -> Result<(u32, u32, u32, u32), String> {
    let mut outer = RECT::default();
    let mut client = RECT::default();
    // SAFETY: both output rectangles belong to the live captured HWND.
    unsafe {
        GetWindowRect(hwnd, &mut outer).map_err(|error| error.to_string())?;
        GetClientRect(hwnd, &mut client).map_err(|error| error.to_string())?;
    }
    let client_width = client.right.saturating_sub(client.left).max(0) as u32;
    let client_height = client.bottom.saturating_sub(client.top).max(0) as u32;
    if client_width == 0 || client_height == 0 {
        return Err("The native video client has no capturable area".into());
    }
    if source_width == client_width && source_height == client_height {
        return Ok((0, 0, source_width, source_height));
    }
    let mut origin = POINT {
        x: client.left,
        y: client.top,
    };
    // SAFETY: ClientToScreen converts the captured window's client origin.
    if !unsafe { ClientToScreen(hwnd, &mut origin) }.as_bool() {
        return Err("Could not locate the video client inside the capture".into());
    }
    let outer_width = outer.right.saturating_sub(outer.left).max(1) as f64;
    let outer_height = outer.bottom.saturating_sub(outer.top).max(1) as f64;
    let scale_x = f64::from(source_width) / outer_width;
    let scale_y = f64::from(source_height) / outer_height;
    let left = ((origin.x - outer.left).max(0) as f64 * scale_x).round() as u32;
    let top = ((origin.y - outer.top).max(0) as f64 * scale_y).round() as u32;
    let right = ((origin.x - outer.left + client_width as i32).max(0) as f64 * scale_x)
        .round()
        .clamp(0.0, f64::from(source_width)) as u32;
    let bottom = ((origin.y - outer.top + client_height as i32).max(0) as f64 * scale_y)
        .round()
        .clamp(0.0, f64::from(source_height)) as u32;
    let right = right.min(source_width);
    let bottom = bottom.min(source_height);
    let width = right.saturating_sub(left);
    let height = bottom.saturating_sub(top);
    if width == 0 || height == 0 {
        return Err("The native video client is outside the captured window".into());
    }
    Ok((left, top, width, height))
}
