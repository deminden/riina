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
                    MacPictureInPicture::new(
                        app.as_weak(),
                        player.clone(),
                        playback.clone(),
                        main_thread,
                    )
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
                        .borrow()
                        .as_ref()
                        .ok_or_else(|| "macOS PiP is unavailable on this system".to_owned())
                        .map(MacPictureInPicture::toggle);
                    if let Err(error) = result
                        && let Some(app) = weak_app.upgrade()
                    {
                        crate::diagnostics::report(&app, error);
                    }
                });

            if self.manager.borrow().is_none() {
                self.timer.stop();
                return;
            }

            let manager = self.manager.clone();
            let dirty = self.dirty.clone();
            self.timer
                .start(TimerMode::Repeated, Duration::from_millis(100), move || {
                    if let Some(manager) = manager.borrow_mut().as_mut() {
                        if dirty.swap(false, Ordering::AcqRel) {
                            manager.synchronize_playback();
                        }
                        // AVKit availability can change while playback has no new frames.
                        manager.advance_start();
                    }
                });
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) use macos::PictureInPictureSession;
