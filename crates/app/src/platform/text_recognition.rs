//! Access native text recognition without making paused-frame logic platform-specific.

#[cfg(target_os = "macos")]
pub(crate) use super::macos::text_recognition::{check_availability, recognize};
#[cfg(target_os = "windows")]
pub(crate) use super::windows::text_recognition::{check_availability, recognize};

pub(crate) const SUPPORTED: bool = cfg!(any(target_os = "windows", target_os = "macos"));
pub(crate) const NATIVE_SELECTION: bool = cfg!(target_os = "macos");

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Region {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub(crate) fn check_availability(
    _complete: impl FnOnce(bool) + Send + 'static,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    Err(std::io::ErrorKind::Unsupported.into())
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub(crate) fn recognize(
    _frame: riina_core::CapturedFrame,
    _complete: impl FnOnce(Result<Vec<Region>, String>) + Send + 'static,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    Err(std::io::ErrorKind::Unsupported.into())
}
