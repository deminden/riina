//! macOS/Linux use the shared GL surface; native HDR presentation is still pending.
#[cfg(target_os = "linux")]
mod session;
#[cfg(target_os = "linux")]
pub(crate) use session::SessionExtras;
#[cfg(target_os = "linux")]
pub(crate) mod accessibility;
#[cfg(target_os = "linux")]
pub(super) mod compact_resize;
mod dialogs;
#[cfg(target_os = "linux")]
pub(super) mod fullscreen;
#[cfg(target_os = "linux")]
pub(super) mod offscreen_webview;
#[cfg(target_os = "linux")]
pub(super) mod pip;
#[cfg(target_os = "linux")]
pub(crate) mod plugin_shortcuts;
pub(crate) mod services;
#[cfg(target_os = "linux")]
pub(crate) mod visibility;
pub(crate) use dialogs::{open as open_dialog, save_playlist_dialog};

// Unix currently uses the shared GL renderer. A real presentation value keeps
// shared setup closures reachable to rustc while reserving native window APIs
// for a later adapter with a verified surface lifetime.
#[derive(Clone)]
pub(crate) struct NativePresentation;

impl NativePresentation {
    pub(crate) fn hdr_display_active(&self) -> bool {
        false
    }

    pub(crate) fn host(&self) -> slint::Weak<crate::NativeVideoHost> {
        slint::Weak::default()
    }

    pub(crate) fn start_engine(
        &self,
        _options: Vec<(String, String)>,
        _notify: impl Fn(std::sync::Arc<riina_core::Playback>) + Send + 'static,
    ) -> Result<riina_mpv::Player, riina_mpv::EngineError> {
        Err(riina_mpv::EngineError::InvalidArgument {
            reason: "Native-window playback is unavailable on this platform",
        })
    }
}
