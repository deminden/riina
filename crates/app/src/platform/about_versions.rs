//! Read engine versions when the native application menu opens About.
#[cfg(target_os = "macos")]
mod native;

#[cfg(target_os = "macos")]
pub(crate) use native::Probe;

#[cfg(not(target_os = "macos"))]
#[derive(Default)]
pub(crate) struct Probe;

#[cfg(not(target_os = "macos"))]
impl Probe {
    pub(crate) fn use_player(&self) {}
}
