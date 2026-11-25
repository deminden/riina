//! Bind the Windows shell media session to the real player toplevel.
mod actions;
mod registration;

use super::{CommandSender, NativeVideoHost, PlaybackStore, PlayerWindow};
use ::windows::Win32::Foundation::HWND;
use registration::Registration;
use riina_core::sync::lock;
use riina_ui::PlayerPreferences;
use slint::{
    ComponentHandle, Weak,
    winit_030::{
        WinitWindowAccessor,
        winit::raw_window_handle::{HasWindowHandle, RawWindowHandle},
    },
};
use std::{cell::RefCell, rc::Rc, time::Duration};

pub(crate) struct SystemMediaSession {
    app: Weak<PlayerWindow>,
    latest: PlaybackStore,
    timer: slint::Timer,
    registration: Rc<RefCell<Option<Registration>>>,
}

impl SystemMediaSession {
    pub(crate) fn new(app: &PlayerWindow, latest: PlaybackStore) -> Self {
        Self {
            app: app.as_weak(),
            latest,
            timer: slint::Timer::default(),
            registration: Rc::default(),
        }
    }

    pub(crate) fn bind(&self, _: &CommandSender, native_host: Option<Weak<NativeVideoHost>>) {
        let app = self.app.clone();
        let state = self.registration.clone();
        let latest = self.latest.clone();
        let mut failed = false;
        self.timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(250),
            move || {
                let Some(window) = app.upgrade() else { return };
                let mut registration = state.borrow_mut();
                if !window.get_loaded()
                    || !window
                        .global::<PlayerPreferences>()
                        .get_key_binding_use_system_media_controls()
                {
                    registration.take();
                    failed = false;
                    return;
                }
                if failed {
                    return;
                }
                if registration.is_none() {
                    let host = native_host.as_ref().and_then(Weak::upgrade);
                    let owner = host
                        .as_ref()
                        .map_or_else(|| window.window(), ComponentHandle::window);
                    if let Some(handle) = owner
                        .with_winit_window(|native| match native.window_handle().ok()?.as_raw() {
                            RawWindowHandle::Win32(handle) => Some(HWND(handle.hwnd.get() as _)),
                            _ => None,
                        })
                        .flatten()
                    {
                        match Registration::open(handle, app.clone(), latest.clone()) {
                            Ok(open) => *registration = Some(open),
                            Err(error) => {
                                crate::diagnostics::warn!(
                                    "Could not register Windows media controls: {error}"
                                );
                                failed = true;
                            }
                        }
                    }
                }
                if let Some(open) = registration.as_mut() {
                    let playback = lock(&latest).clone();
                    if let Err(error) = open.synchronize(&playback) {
                        crate::diagnostics::warn!(
                            "Could not update Windows media controls: {error}"
                        );
                        registration.take();
                        failed = true;
                    }
                }
            },
        );
    }

    pub(crate) fn prepare_to_close(&self) {
        self.timer.stop();
        self.registration.borrow_mut().take();
    }
}
