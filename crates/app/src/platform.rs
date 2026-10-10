//! Native services are isolated here; core state and the shared UI own behavior.
pub(crate) mod language;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub(crate) use windows::{
    NativePresentation, file_dialog, native_window, open_dialog, services::*,
};

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod unix;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) use unix::{NativePresentation, open_dialog, services::*};
