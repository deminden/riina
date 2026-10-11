use riina_core::CapturedFrame;
use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    thread::{self, JoinHandle},
};
use windows::{
    Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap},
    Media::Ocr::OcrEngine,
    Storage::Streams::{DataWriter, InMemoryRandomAccessStream},
    Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Region {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// The WinRT recognizer and its frame conversion stay on a dedicated MTA worker.
pub(crate) fn check_availability(
    complete: impl FnOnce(bool) + Send + 'static,
) -> std::io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("riina-text-availability".into())
        .spawn(move || {
            complete(
                with_apartment(|| {
                    OcrEngine::TryCreateFromUserProfileLanguages()
                        .map(|_| ())
                        .map_err(|error| error.to_string())
                })
                .is_ok(),
            );
        })
}

/// OCR starts independently of playback and rendering; callers reject stale results.
pub(crate) fn recognize(
    frame: CapturedFrame,
    complete: impl FnOnce(Result<Vec<Region>, String>) + Send + 'static,
) -> std::io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("riina-live-text".into())
        .spawn(move || complete(with_apartment(|| recognize_frame(&frame))))
}

fn with_apartment<T>(operation: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    // Each worker owns its WinRT apartment, avoiding assumptions about Slint's UI apartment.
    unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.map_err(|error| error.to_string())?;
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            // SAFETY: this guard is created only after this thread initialized WinRT.
            unsafe { RoUninitialize() };
        }
    }
    let _apartment = Apartment;
    operation()
}

fn recognize_frame(frame: &CapturedFrame) -> Result<Vec<Region>, String> {
    let engine =
        OcrEngine::TryCreateFromUserProfileLanguages().map_err(|error| error.to_string())?;
    let max_dimension = OcrEngine::MaxImageDimension().map_err(|error| error.to_string())?;
    let scale = (max_dimension as f32 / frame.width as f32)
        .min(max_dimension as f32 / frame.height as f32)
        .min(1.0);
    let width = ((frame.width as f32 * scale).floor() as u32).max(1);
    let height = ((frame.height as f32 * scale).floor() as u32).max(1);
    // WinRT OCR accepts BGRA8. mpv supplies RGBA, so pack and downscale on this
    // worker before crossing the WinRT boundary; playback never waits on OCR.
    let mut bgra = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        let source_y = (u64::from(y) * u64::from(frame.height) / u64::from(height)) as usize;
        for x in 0..width {
            let source_x = (u64::from(x) * u64::from(frame.width) / u64::from(width)) as usize;
            let offset = (source_y * frame.width as usize + source_x) * 4;
            bgra.extend_from_slice(&[
                frame.rgba[offset + 2],
                frame.rgba[offset + 1],
                frame.rgba[offset],
                frame.rgba[offset + 3],
            ]);
        }
    }
    let stream = InMemoryRandomAccessStream::new().map_err(|error| error.to_string())?;
    let writer = DataWriter::CreateDataWriter(&stream).map_err(|error| error.to_string())?;
    writer
        .WriteBytes(&bgra)
        .map_err(|error| error.to_string())?;
    let buffer = writer.DetachBuffer().map_err(|error| error.to_string())?;
    let bitmap = SoftwareBitmap::CreateCopyFromBuffer(
        &buffer,
        BitmapPixelFormat::Bgra8,
        width as i32,
        height as i32,
    )
    .map_err(|error| error.to_string())?;
    let operation = engine
        .RecognizeAsync(&bitmap)
        .map_err(|error| error.to_string())?;
    let result = block_on(async move { operation.await }).map_err(|error| error.to_string())?;
    let lines = result.Lines().map_err(|error| error.to_string())?;
    let mut regions = Vec::with_capacity(lines.Size().map_err(|error| error.to_string())? as usize);
    for line_index in 0..lines.Size().map_err(|error| error.to_string())? {
        let line = lines.GetAt(line_index).map_err(|error| error.to_string())?;
        let words = line.Words().map_err(|error| error.to_string())?;
        let mut left = f32::INFINITY;
        let mut top = f32::INFINITY;
        let mut right = f32::NEG_INFINITY;
        let mut bottom = f32::NEG_INFINITY;
        for word_index in 0..words.Size().map_err(|error| error.to_string())? {
            let bounds = words
                .GetAt(word_index)
                .and_then(|word| word.BoundingRect())
                .map_err(|error| error.to_string())?;
            left = left.min(bounds.X);
            top = top.min(bounds.Y);
            right = right.max(bounds.X + bounds.Width);
            bottom = bottom.max(bounds.Y + bounds.Height);
        }
        let text = line.Text().map_err(|error| error.to_string())?.to_string();
        if text.trim().is_empty() || !left.is_finite() || !top.is_finite() {
            continue;
        }
        left = left.clamp(0.0, frame.width as f32);
        right = right.clamp(left, frame.width as f32);
        top = top.clamp(0.0, frame.height as f32);
        bottom = bottom.clamp(top, frame.height as f32);
        if right > left && bottom > top {
            regions.push(Region {
                text,
                x: left / scale,
                y: top / scale,
                width: (right - left) / scale,
                height: (bottom - top) / scale,
            });
        }
    }
    Ok(regions)
}

fn block_on<F: Future>(future: F) -> F::Output {
    struct ThreadWake(thread::Thread);
    impl Wake for ThreadWake {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }

    let waker = Waker::from(Arc::new(ThreadWake(thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => thread::park(),
        }
    }
}
