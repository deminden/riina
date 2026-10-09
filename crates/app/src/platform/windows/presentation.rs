//! A presentation owns its native target; the shared controller never handles HWNDs.
use super::services;
use crate::NativeVideoHost;
use riina_core::Playback;
use riina_mpv::Player;
use slint::winit_030::winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};
use std::sync::Arc;

pub(crate) struct NativePresentation {
    // UiSession drops the player before this retained target. Decoder callbacks
    // cannot outlive its HWND, even if another owner releases its window first.
    _window: Arc<Window>,
    hwnd: isize,
    wid: u32,
    host: slint::Weak<NativeVideoHost>,
}

impl NativePresentation {
    pub(super) fn new(
        window: Arc<Window>,
        host: slint::Weak<NativeVideoHost>,
    ) -> Result<Self, String> {
        let hwnd = match window
            .window_handle()
            .map_err(|error| error.to_string())?
            .as_raw()
        {
            RawWindowHandle::Win32(handle) => handle.hwnd.get(),
            _ => return Err("Native video requires a Windows HWND".into()),
        };
        let wid = u32::try_from(hwnd as usize).map_err(|error| error.to_string())?;
        Ok(Self {
            _window: window,
            hwnd,
            wid,
            host,
        })
    }

    pub(super) fn owner_handle(&self) -> isize {
        self.hwnd
    }

    pub(crate) fn host(&self) -> slint::Weak<NativeVideoHost> {
        self.host.clone()
    }

    pub(crate) fn start_engine(
        &self,
        notify: impl Fn(Playback) + Send + 'static,
    ) -> Result<Player, String> {
        // Runtime diagnostics belong to the user's application directory, not to
        // the compiler's source checkout, which may not exist on another machine.
        let log = services::preferences_path()
            .map_err(|error| error.to_string())?
            .with_file_name("native-player.log");
        Player::start_native_window(&services::playback_library(), self.wid, &log, notify)
    }
}
