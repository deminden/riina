//! Native ownership for the Settings font chooser.
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "macos")]
pub(crate) use macos::{Modal, attach};
#[cfg(windows)]
pub(crate) use windows::{Modal, attach};
