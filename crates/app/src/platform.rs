//! Native services are isolated here; core state and the shared UI own behavior.
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub(crate) use windows::{NativePresentation, file_dialog, native_window, services::*, ui_dialogs};
