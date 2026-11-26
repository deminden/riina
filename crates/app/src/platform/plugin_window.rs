//! Native outer frames and display identity for the plugin window proxy.
#[cfg(target_os = "macos")]
mod appkit;
#[cfg(not(target_os = "macos"))]
mod desktop;
#[cfg(not(windows))]
mod resize_request;

#[cfg(target_os = "macos")]
pub(crate) use appkit::{observe, set_frame, set_minimized};
#[cfg(not(target_os = "macos"))]
pub(crate) use desktop::{observe, set_frame, set_minimized};
use serde::Serialize;

pub(crate) fn accepts_frame_resize(
    window: &slint::Window,
    size: riina_core::window_aspect::PixelSize,
) -> bool {
    #[cfg(not(windows))]
    return resize_request::take(window, size);
    #[cfg(windows)]
    {
        let _ = (window, size);
        false
    }
}

#[derive(Clone, Copy, Serialize)]
pub(crate) struct Frame {
    pub(crate) x: Option<f64>,
    pub(crate) y: Option<f64>,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

impl Frame {
    pub(crate) fn positioned(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x: Some(x),
            y: Some(y),
            width,
            height,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct Screen {
    pub(crate) frame: Frame,
    pub(crate) main: bool,
    pub(crate) current: bool,
}

pub(crate) struct Observation {
    pub(crate) frame: Frame,
    pub(crate) current_screen: Option<String>,
    pub(crate) screens: Vec<Screen>,
    pub(crate) visible: Option<bool>,
    pub(crate) minimized: Option<bool>,
    pub(crate) main: bool,
}
