//! Keep Now Playing controls and application focus events with the player session.
use crate::{NativeVideoHost, PlaybackStore, PlayerWindow};
use riina_mpv::CommandSender;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub(crate) struct SystemMediaSession;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
impl SystemMediaSession {
    pub(crate) fn new(_: &PlayerWindow, _: PlaybackStore) -> Self {
        Self
    }
    pub(crate) fn bind(&self, _: &CommandSender, _: Option<slint::Weak<NativeVideoHost>>) {}
    pub(crate) fn prepare_to_close(&self) {}
}

#[cfg(any(target_os = "macos", test))]
pub(super) mod ownership;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub(crate) use macos::SystemMediaSession;
#[cfg(target_os = "macos")]
pub(super) use macos::wake_application;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::SystemMediaSession;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub(crate) use linux::SystemMediaSession;

impl SystemMediaSession {
    pub(crate) fn bind_timeline_samples(&self, samples: &super::player_thumbnails::Samples) {
        #[cfg(target_os = "macos")]
        self.store_timeline_samples(samples);
        #[cfg(not(target_os = "macos"))]
        let _ = samples;
    }
}
