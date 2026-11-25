//! Platform Picture-in-Picture setup and delivery lifetime for the shared player.
#[cfg(not(target_os = "linux"))]
use crate::{PlaybackStore, PlayerWindow};
#[cfg(not(target_os = "linux"))]
use riina_mpv::CommandSender;
#[cfg(not(target_os = "linux"))]
use std::sync::{Arc, atomic::AtomicBool};

#[cfg(target_os = "linux")]
pub(crate) use super::unix::pip::PictureInPictureSession;

#[cfg(target_os = "windows")]
pub(crate) struct PictureInPictureSession;

#[cfg(target_os = "windows")]
impl PictureInPictureSession {
    pub(crate) fn new(_app: &PlayerWindow) -> Self {
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
    use crate::platform::{
        MacPictureInPicture, active_poll::ActivePoll, macos::pip_window::MinimizeObservation,
    };
    use slint::ComponentHandle;
    use std::{cell::RefCell, rc::Rc, sync::atomic::Ordering, time::Duration};

    const ONSCREEN_INTERVAL: Duration = Duration::from_millis(100);
    const OFFSCREEN_INTERVAL: Duration = Duration::from_millis(33);

    pub(crate) struct PictureInPictureSession {
        poll: Rc<PresentationPoll>,
        minimize_observation: RefCell<Option<MinimizeObservation>>,
    }

    struct PresentationPoll {
        app: slint::Weak<PlayerWindow>,
        manager: Rc<RefCell<Option<MacPictureInPicture>>>,
        timer: ActivePoll,
        dirty: Arc<AtomicBool>,
    }

    impl PictureInPictureSession {
        pub(crate) fn new(app: &PlayerWindow) -> Self {
            Self {
                poll: Rc::new(PresentationPoll {
                    app: app.as_weak(),
                    manager: Rc::default(),
                    timer: ActivePoll::default(),
                    dirty: Arc::default(),
                }),
                minimize_observation: RefCell::new(None),
            }
        }

        pub(crate) fn notification_flag(&self) -> Option<Arc<AtomicBool>> {
            Some(self.poll.dirty.clone())
        }

        pub(crate) fn render_extras(&self) -> crate::platform::SharedVideoExtras {
            crate::platform::SharedVideoExtras::with_picture_in_picture(self.poll.manager.clone())
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
            *self.poll.manager.borrow_mut() = match manager {
                Ok(manager) => Some(manager),
                Err(error) => {
                    crate::diagnostics::report(app, format!("macOS PiP unavailable: {error}"));
                    None
                }
            };
            let toggle = Rc::downgrade(&self.poll);
            app.global::<riina_ui::PlayerMenuActions>()
                .on_toggle_picture_in_picture(move || {
                    if let Some(poll) = toggle.upgrade() {
                        poll.toggle();
                    }
                });

            if self.poll.manager.borrow().is_none() {
                return;
            }

            let minimize = Rc::downgrade(&self.poll);
            self.minimize_observation
                .replace(Some(MinimizeObservation::new(app, move |minimized| {
                    if let Some(poll) = minimize.upgrade() {
                        poll.minimized(minimized);
                    }
                })));
        }

        pub(crate) fn prepare_to_close(&self) {
            self.poll.timer.stop();
            self.minimize_observation.replace(None);
            if let Some(manager) = self.poll.manager.borrow().as_ref() {
                manager.close();
            }
        }
    }

    impl PresentationPoll {
        fn toggle(self: &Rc<Self>) {
            let result = self
                .manager
                .borrow_mut()
                .as_mut()
                .ok_or_else(|| "macOS PiP is unavailable on this system".to_owned())
                .and_then(MacPictureInPicture::toggle);
            if let Err(error) = result
                && let Some(app) = self.app.upgrade()
            {
                crate::diagnostics::report(&app, error);
            }
            self.start_if_needed();
        }

        fn minimized(self: &Rc<Self>, minimized: bool) {
            if let Some(app) = self.app.upgrade()
                && let Ok(mut manager) = self.manager.try_borrow_mut()
                && let Some(manager) = manager.as_mut()
            {
                manager.minimized(&app, minimized);
            }
            self.start_if_needed();
        }

        fn start_if_needed(self: &Rc<Self>) {
            if !self.manager.try_borrow().is_ok_and(|manager| {
                manager
                    .as_ref()
                    .is_some_and(MacPictureInPicture::needs_presentation_poll)
            }) {
                return;
            }
            let weak = Rc::downgrade(self);
            self.timer.start_if_idle(ONSCREEN_INTERVAL, move || {
                weak.upgrade().and_then(|poll| poll.tick())
            });
        }

        fn tick(&self) -> Option<Duration> {
            let app = self.app.upgrade()?;
            let Ok(mut manager) = self.manager.try_borrow_mut() else {
                return Some(ONSCREEN_INTERVAL);
            };
            let manager = manager.as_mut()?;
            if self.dirty.swap(false, Ordering::AcqRel) {
                manager.synchronize_playback();
            }
            let interval = match manager.poll_source(&app) {
                Ok(true) => OFFSCREEN_INTERVAL,
                Ok(false) => ONSCREEN_INTERVAL,
                Err(error) => {
                    crate::diagnostics::report(&app, format!("Picture-in-Picture: {error}"));
                    ONSCREEN_INTERVAL
                }
            };
            manager.needs_presentation_poll().then_some(interval)
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) use macos::PictureInPictureSession;
