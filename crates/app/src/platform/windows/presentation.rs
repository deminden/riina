//! A presentation owns its native target; the shared controller never handles HWNDs.
use super::frame_capture::FrameCapture;
use crate::NativeVideoHost;
use riina_core::Playback;
use riina_mpv::Player;
use slint::winit_030::WinitWindowAccessor;
use slint::winit_030::winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};
use std::sync::Arc;
use windows::{
    Win32::{
        Foundation::HWND,
        Graphics::{
            Dxgi::{
                Common::DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020, CreateDXGIFactory1,
                DXGI_ERROR_NOT_FOUND, IDXGIFactory1, IDXGIOutput6,
            },
            Gdi::{MONITOR_DEFAULTTONEAREST, MonitorFromWindow},
        },
    },
    core::Interface,
};

#[derive(Clone)]
pub(crate) struct NativePresentation {
    // UiSession drops the player before this retained target. Decoder callbacks
    // cannot outlive its HWND, even if another owner releases its window first.
    _window: Arc<Window>,
    hwnd: isize,
    wid: u32,
    host: slint::Weak<NativeVideoHost>,
    material_capture: Option<FrameCapture>,
}

impl NativePresentation {
    pub(super) fn new(
        window: Arc<Window>,
        host: slint::Weak<NativeVideoHost>,
    ) -> Result<Self, String> {
        let hwnd = match window
            .window_handle()
            .map_err(|error| error.to_string())?
            .as_raw()
        {
            RawWindowHandle::Win32(handle) => handle.hwnd.get(),
            _ => return Err("Native video requires a Windows HWND".into()),
        };
        let wid = u32::try_from(hwnd as usize).map_err(|error| error.to_string())?;
        let material_capture = match FrameCapture::start(HWND(hwnd as *mut _)) {
            Ok(capture) => Some(capture),
            Err(error) => {
                eprintln!(
                    "Windows video material capture unavailable; using mpv sampling: {error}"
                );
                None
            }
        };
        Ok(Self {
            _window: window,
            hwnd,
            wid,
            host,
            material_capture,
        })
    }

    pub(super) fn owner_handle(&self) -> isize {
        self.hwnd
    }

    pub(crate) fn webview_parent_window(&self) -> Arc<Window> {
        self._window.clone()
    }

    pub(crate) fn host(&self) -> slint::Weak<NativeVideoHost> {
        self.host.clone()
    }

    pub(crate) fn material_capture(&self) -> Option<FrameCapture> {
        self.material_capture.clone()
    }

    pub(crate) fn hdr_display_active(&self) -> bool {
        hdr_display_active_for(self._window.as_ref())
    }

    pub(crate) fn start_engine(
        &self,
        options: Vec<(String, String)>,
        notify: impl Fn(Playback) + Send + 'static,
    ) -> Result<Player, String> {
        Player::start_native_window(
            &super::services::playback_library(),
            self.wid,
            options,
            notify,
        )
    }
}

pub(super) fn hdr_display_active_for(window: &Window) -> bool {
    let Ok(raw) = window.window_handle() else {
        return false;
    };
    let RawWindowHandle::Win32(raw) = raw.as_raw() else {
        return false;
    };
    // Windows exposes the active output color space through DXGI. Matching the
    // window's monitor avoids enabling HDR controls for another attached screen.
    let monitor =
        unsafe { MonitorFromWindow(HWND(raw.hwnd.get() as *mut _), MONITOR_DEFAULTTONEAREST) };
    if monitor.0.is_null() {
        return false;
    }
    let Ok(factory) = (unsafe { CreateDXGIFactory1::<IDXGIFactory1>() }) else {
        return false;
    };
    let mut adapter_index = 0;
    loop {
        let adapter = match unsafe { factory.EnumAdapters1(adapter_index) } {
            Ok(adapter) => adapter,
            Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
            Err(_) => break,
        };
        let mut output_index = 0;
        loop {
            let output = match unsafe { adapter.EnumOutputs(output_index) } {
                Ok(output) => output,
                Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
                Err(_) => break,
            };
            output_index += 1;
            let Ok(output) = output.cast::<IDXGIOutput6>() else {
                continue;
            };
            let Ok(description) = (unsafe { output.GetDesc1() }) else {
                continue;
            };
            if description.Monitor == monitor
                && description.AttachedToDesktop.as_bool()
                && description.ColorSpace == DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020
            {
                return true;
            }
        }
        adapter_index += 1;
    }
    false
}

pub(super) fn hdr_display_active_for_slint(window: &slint::Window) -> bool {
    window
        .with_winit_window(hdr_display_active_for)
        .unwrap_or(false)
}
