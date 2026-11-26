//! Optional, local Linux text recognition using the distribution's Tesseract ABI.
mod api;
mod languages;

use crate::platform::text_recognition::{Cancellation, Region};
use api::Api;
use riina_core::CapturedFrame;
use std::{io, sync::OnceLock, thread::JoinHandle};

fn api() -> Result<&'static Api, String> {
    static API: OnceLock<Result<Api, String>> = OnceLock::new();
    API.get_or_init(Api::load).as_ref().map_err(Clone::clone)
}

pub(crate) fn check_availability(
    complete: impl FnOnce(bool) + Send + 'static,
) -> io::Result<JoinHandle<()>> {
    crate::background::spawn_named("riina-text-availability", move || {
        complete(
            api()
                .and_then(|api| api.open(&languages::configuration()))
                .is_ok(),
        );
    })
}

pub(crate) fn recognize(
    frame: CapturedFrame,
    cancellation: Cancellation,
    complete: impl FnOnce(Result<Vec<Region>, String>) + Send + 'static,
) -> io::Result<JoinHandle<()>> {
    crate::background::spawn_named("riina-live-text", move || {
        complete(recognize_frame(&frame, &cancellation));
    })
}

fn recognize_frame(
    frame: &CapturedFrame,
    cancellation: &Cancellation,
) -> Result<Vec<Region>, String> {
    if cancellation.is_cancelled() {
        return Ok(Vec::new());
    }
    const MAX_SIDE: u32 = 2560;
    let pixels = frame
        .downsample_to_fit(MAX_SIDE, MAX_SIDE)
        .map_err(|error| error.to_string())?;
    if cancellation.is_cancelled() {
        return Ok(Vec::new());
    }
    let mut engine = api()?.open(&languages::configuration())?;
    let lines = engine.lines(&pixels, cancellation)?;
    let scale_x = frame.width as f32 / pixels.width as f32;
    let scale_y = frame.height as f32 / pixels.height as f32;
    Ok(lines
        .into_iter()
        .filter_map(|line| {
            let [left, top, right, bottom] = line.bounds;
            let left = left.clamp(0, pixels.width as i32);
            let right = right.clamp(left, pixels.width as i32);
            let top = top.clamp(0, pixels.height as i32);
            let bottom = bottom.clamp(top, pixels.height as i32);
            (right > left && bottom > top).then(|| Region {
                text: line.text,
                x: left as f32 * scale_x,
                y: top as f32 * scale_y,
                width: (right - left) as f32 * scale_x,
                height: (bottom - top) as f32 * scale_y,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests;
