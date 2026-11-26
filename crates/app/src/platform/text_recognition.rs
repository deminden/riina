//! Access native text recognition without making paused-frame logic platform-specific.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[cfg(target_os = "macos")]
pub(crate) use super::macos::text_recognition::{check_availability, recognize};
#[cfg(target_os = "linux")]
pub(crate) use super::unix::text_recognition::{check_availability, recognize};
#[cfg(target_os = "windows")]
pub(crate) use super::windows::text_recognition::{check_availability, recognize};

pub(crate) const SUPPORTED: bool = cfg!(any(
    target_os = "linux",
    target_os = "windows",
    target_os = "macos"
));
pub(crate) const SELECTABLE: bool = cfg!(any(target_os = "linux", target_os = "macos"));

#[derive(Clone, Default)]
pub(crate) struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub(crate) fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Region {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub(crate) fn check_availability(
    _complete: impl FnOnce(bool) + Send + 'static,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    Err(std::io::ErrorKind::Unsupported.into())
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub(crate) fn recognize(
    _frame: riina_core::CapturedFrame,
    _cancellation: Cancellation,
    _complete: impl FnOnce(Result<Vec<Region>, String>) + Send + 'static,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    Err(std::io::ErrorKind::Unsupported.into())
}
