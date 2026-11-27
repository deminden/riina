//! Bind the Windows shell media session to the real player toplevel.
mod actions;
mod registration;
mod suspend;

use super::{CommandSender, NativeVideoHost, PlaybackStore, PlayerWindow, pause_for_suspend};
use crate::platform::inactive_playback;
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
    native_host: Option<Weak<NativeVideoHost>>,
    focus: RefCell<Option<inactive_playback::Registration>>,
    timer: slint::Timer,
    registration: Rc<RefCell<Option<Registration>>>,
    suspend: Rc<RefCell<Option<suspend::Registration>>>,
}

impl SystemMediaSession {
    pub(crate) fn new(
        app: &PlayerWindow,
        latest: PlaybackStore,
        native_host: Option<Weak<NativeVideoHost>>,
    ) -> Self {
        Self {
            app: app.as_weak(),
            focus: RefCell::new(Some(inactive_playback::Registration::new(
                app,
                native_host.clone(),
                latest.clone(),
            ))),
            latest,
            native_host,
            timer: slint::Timer::default(),
            registration: Rc::default(),
            suspend: Rc::default(),
        }
    }

    pub(crate) fn bind(&self, _: &CommandSender) {
        let app = self.app.clone();
        let native_host = self.native_host.clone();
        let state = self.registration.clone();
        let latest = self.latest.clone();
        let suspend = self.suspend.clone();
        let mut suspend_attempted = false;
        let mut failed = false;
        self.timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(250),
            move || {
                let Some(window) = app.upgrade() else { return };
                if !suspend_attempted
                    && let Some(handle) = owning_hwnd(&window, native_host.as_ref())
                {
                    suspend_attempted = true;
                    match suspend::Registration::open(handle, app.clone(), latest.clone()) {
                        Ok(open) => *suspend.borrow_mut() = Some(open),
                        Err(error) => crate::diagnostics::warn!(
                            "Could not observe Windows sleep notifications: {error}"
                        ),
                    }
                }
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
                if registration.is_none()
                    && let Some(handle) = owning_hwnd(&window, native_host.as_ref())
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
        self.suspend.borrow_mut().take();
        self.focus.borrow_mut().take();
    }
}

fn owning_hwnd(app: &PlayerWindow, native_host: Option<&Weak<NativeVideoHost>>) -> Option<HWND> {
    let host = native_host.and_then(Weak::upgrade);
    let owner = host
        .as_ref()
        .map_or_else(|| app.window(), ComponentHandle::window);
    owner
        .with_winit_window(|native| match native.window_handle().ok()?.as_raw() {
            RawWindowHandle::Win32(handle) => Some(HWND(handle.hwnd.get() as _)),
            _ => None,
        })
        .flatten()
}
