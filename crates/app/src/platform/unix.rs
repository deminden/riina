//! macOS/Linux use the shared GL surface; native HDR presentation is still pending.
mod dialogs;
pub(crate) mod services;
pub(crate) use dialogs::open as open_dialog;

// No constructor exists until a native surface has a verified ownership contract.
// An uninhabited type makes the shared session shape usable without a dummy handle.
pub(crate) enum NativePresentation {}

impl NativePresentation {
    pub(crate) fn host(&self) -> slint::Weak<crate::NativeVideoHost> {
        match *self {}
    }

    pub(crate) fn start_engine(
        &self,
        _notify: impl Fn(riina_core::Playback) + Send + 'static,
    ) -> Result<riina_mpv::Player, String> {
        match *self {}
    }
}
