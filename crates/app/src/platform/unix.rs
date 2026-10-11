//! macOS/Linux use the shared GL surface; native HDR presentation is still pending.
mod dialogs;
pub(crate) mod services;
pub(crate) use dialogs::open as open_dialog;

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
        _notify: impl Fn(riina_core::Playback) + Send + 'static,
    ) -> Result<riina_mpv::Player, String> {
        Err("Native-window playback is unavailable on this platform".into())
    }
}
