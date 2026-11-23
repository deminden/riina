//! Platform Picture-in-Picture setup and delivery lifetime for the shared player.
use crate::{PlaybackStore, PlayerWindow};
use riina_mpv::CommandSender;
use std::sync::{Arc, atomic::AtomicBool};

#[cfg(not(target_os = "macos"))]
pub(crate) struct PictureInPictureSession;

#[cfg(not(target_os = "macos"))]
impl PictureInPictureSession {
    pub(crate) fn new(app: &PlayerWindow) -> Self {
        #[cfg(target_os = "linux")]
        {
            use slint::ComponentHandle;
            let weak = app.as_weak();
            app.global::<riina_ui::PlayerMenuActions>()
                .on_toggle_picture_in_picture(move || {
                    if let Some(app) = weak.upgrade() {
                        app.set_status(
                            "Picture-in-Picture is unavailable on this presentation path".into(),
                        );
                    }
                });
        }
        #[cfg(target_os = "windows")]
        let _ = app;
        Self
    }

    pub(crate) fn bind(
        &self,
        _app: &PlayerWindow,
        _player: &CommandSender,
        _playback: &PlaybackStore,
    ) {
    }

    pub(crate) fn render_extras(&self) -> crate::platform::SharedVideoExtras {
        crate::platform::SharedVideoExtras::default()
    }

    pub(crate) fn notification_flag(&self) -> Option<Arc<AtomicBool>> {
        None
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use crate::platform::MacPictureInPicture;
    use slint::{ComponentHandle, Timer, TimerMode};
    use std::{cell::RefCell, rc::Rc, sync::atomic::Ordering, time::Duration};

    pub(crate) struct PictureInPictureSession {
        manager: Rc<RefCell<Option<MacPictureInPicture>>>,
        timer: Timer,
        dirty: Arc<AtomicBool>,
    }

    impl PictureInPictureSession {
        pub(crate) fn new(_app: &PlayerWindow) -> Self {
            Self {
                manager: Rc::new(RefCell::new(None)),
                timer: Timer::default(),
                dirty: Arc::new(AtomicBool::new(false)),
            }
        }

        pub(crate) fn notification_flag(&self) -> Option<Arc<AtomicBool>> {
            Some(self.dirty.clone())
        }

        pub(crate) fn render_extras(&self) -> crate::platform::SharedVideoExtras {
            crate::platform::SharedVideoExtras::with_picture_in_picture(self.manager.clone())
        }

        pub(crate) fn bind(
            &self,
            app: &PlayerWindow,
            player: &CommandSender,
            playback: &PlaybackStore,
        ) {
            let manager = objc2::MainThreadMarker::new()
                .ok_or_else(|| std::io::Error::other("PiP setup must run on the macOS main thread"))
                .and_then(|main_thread| {
                    MacPictureInPicture::new(player.clone(), playback.clone(), main_thread)
                        .map_err(std::io::Error::other)
                });
            *self.manager.borrow_mut() = match manager {
                Ok(manager) => Some(manager),
                Err(error) => {
                    crate::diagnostics::report(app, format!("macOS PiP unavailable: {error}"));
                    None
                }
            };
            let weak_app = app.as_weak();
            let toggle_manager = self.manager.clone();
            app.global::<riina_ui::PlayerMenuActions>()
                .on_toggle_picture_in_picture(move || {
                    let result = toggle_manager
                        .borrow_mut()
                        .as_mut()
                        .ok_or_else(|| "macOS PiP is unavailable on this system".to_owned())
                        .and_then(MacPictureInPicture::toggle);
                    if let Err(error) = result
                        && let Some(app) = weak_app.upgrade()
                    {
                        crate::diagnostics::report(&app, error);
                    }
                });

            if self.manager.borrow().is_none() {
                self.timer.stop();
                app.set_picture_in_picture_active(false);
                return;
            }

            let manager = self.manager.clone();
            let dirty = self.dirty.clone();
            let weak_app = app.as_weak();
            self.timer
                .start(TimerMode::Repeated, Duration::from_millis(100), move || {
                    let mut active = false;
                    if let Some(manager) = manager.borrow_mut().as_mut() {
                        active = manager.is_active();
                        if dirty.swap(false, Ordering::AcqRel) {
                            manager.synchronize_playback();
                        }
                    }
                    if let Some(app) = weak_app.upgrade()
                        && app.get_picture_in_picture_active() != active
                    {
                        app.set_picture_in_picture_active(active);
                    }
                });
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) use macos::PictureInPictureSession;
