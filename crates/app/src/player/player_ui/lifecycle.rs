//! A window may resume only the media session it paused automatically.
use crate::PlayerWindow;
use riina_core::{Phase, Playback};
use slint::ComponentHandle;
use std::{cell::RefCell, rc::Rc, time::Duration};

#[derive(Default)]
pub(crate) struct AutomaticPause {
    generation: Option<u64>,
}

impl AutomaticPause {
    pub(crate) fn pause(&mut self, playback: &Playback) -> bool {
        if !self.is_pending() && playback.phase == Phase::Playing && !playback.paused {
            self.generation = Some(playback.generation);
            true
        } else {
            false
        }
    }

    pub(crate) fn resume(&mut self, playback: &Playback) -> bool {
        self.generation.take().is_some_and(|generation| {
            // The resume intent may follow pause before its native observation.
            // Both commands still execute in order on the playback worker.
            playback.generation == generation
                && matches!(
                    playback.phase,
                    Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Seeking
                )
        })
    }

    pub(crate) fn is_pending(&self) -> bool {
        self.generation.is_some()
    }

    pub(crate) fn cancel(&mut self) {
        self.generation = None;
    }
}

#[derive(Default)]
struct MinimizeWatch {
    minimized: bool,
    pause: AutomaticPause,
}

impl MinimizeWatch {
    fn observe(
        &mut self,
        minimized: bool,
        enabled: bool,
        playback: &crate::PlaybackStore,
    ) -> Option<bool> {
        let was_minimized = std::mem::replace(&mut self.minimized, minimized);
        if minimized == was_minimized {
            return None;
        }
        if minimized && enabled {
            let snapshot = riina_core::sync::lock(playback);
            self.pause.pause(&snapshot).then_some(true)
        } else if enabled {
            let snapshot = riina_core::sync::lock(playback);
            self.pause.resume(&snapshot).then_some(false)
        } else {
            self.pause.cancel();
            None
        }
    }
}

fn player_is_minimized(
    app: &PlayerWindow,
    native_host: Option<&slint::Weak<crate::NativeVideoHost>>,
) -> bool {
    // Native Windows playback has a framed owner and a separate controls
    // surface. Only the owner's minimization represents the player.
    native_host.and_then(slint::Weak::upgrade).map_or_else(
        || app.window().is_minimized(),
        |host| host.window().is_minimized(),
    )
}

pub(super) fn install_pause_on_minimize(
    app: &PlayerWindow,
    native_host: Option<slint::Weak<crate::NativeVideoHost>>,
    playback: crate::PlaybackStore,
) -> Rc<slint::Timer> {
    let timer = Rc::new(slint::Timer::default());
    let watch = Rc::new(RefCell::new(MinimizeWatch::default()));
    let weak_app = app.as_weak();
    let weak_timer = Rc::downgrade(&timer);
    let watch_for_tick = watch.clone();
    let host_for_tick = native_host.clone();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(50),
        move || {
            let Some(app) = weak_app.upgrade() else {
                return;
            };
            let enabled = app.get_pause_when_minimized();
            let paused = watch_for_tick.borrow_mut().observe(
                player_is_minimized(&app, host_for_tick.as_ref()),
                enabled,
                &playback,
            );
            if let Some(paused) = paused {
                app.invoke_set_paused(paused);
            }
            if !app.get_pause_when_minimized()
                && !watch_for_tick.borrow().pause.is_pending()
                && let Some(timer) = weak_timer.upgrade()
            {
                timer.stop();
            }
        },
    );
    if !app.get_pause_when_minimized() {
        timer.stop();
    }
    let weak_app = app.as_weak();
    let weak_timer = Rc::downgrade(&timer);
    app.on_pause_when_minimized_changed(move |enabled| {
        let (Some(app), Some(timer)) = (weak_app.upgrade(), weak_timer.upgrade()) else {
            return;
        };
        if enabled && !timer.running() {
            // Enabling the setting for an already minimized window should
            // wait for a new minimize edge.
            watch.borrow_mut().minimized = player_is_minimized(&app, native_host.as_ref());
            timer.restart();
        } else if !enabled && !watch.borrow().pause.is_pending() {
            timer.stop();
        }
    });
    timer
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playing() -> Playback {
        Playback {
            generation: 4,
            phase: Phase::Playing,
            paused: false,
            ..Playback::default()
        }
    }

    #[test]
    fn minimize_timer_runs_only_when_enabled_or_holding_a_pause() -> Result<(), slint::PlatformError>
    {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new()?;
        let playback = std::sync::Arc::new(std::sync::Mutex::new(std::sync::Arc::new(playing())));
        let timer = install_pause_on_minimize(&app, None, playback);
        let decisions = Rc::new(RefCell::new(Vec::new()));
        let observed = decisions.clone();
        app.on_set_paused(move |paused| observed.borrow_mut().push(paused));
        assert!(!timer.running());

        app.window().set_minimized(true);
        app.set_pause_when_minimized(true);
        i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
        assert!(timer.running());
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
        assert!(decisions.borrow().is_empty());

        app.window().set_minimized(false);
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
        app.window().set_minimized(true);
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
        assert_eq!(*decisions.borrow(), [true]);

        app.set_pause_when_minimized(false);
        i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
        assert!(timer.running());
        app.window().set_minimized(false);
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
        assert!(!timer.running());
        assert_eq!(*decisions.borrow(), [true]);
        Ok(())
    }

    #[test]
    fn restoring_a_window_resumes_its_automatically_paused_media_once() {
        let mut policy = AutomaticPause::default();
        assert!(policy.pause(&playing()));
        let paused = Playback {
            phase: Phase::Paused,
            paused: true,
            ..playing()
        };
        assert!(policy.resume(&paused));
        assert!(!policy.resume(&paused));
    }

    #[test]
    fn restore_before_pause_acknowledgement_still_queues_resume() {
        let mut policy = AutomaticPause::default();
        assert!(policy.pause(&playing()));
        assert!(policy.resume(&playing()));
    }

    #[test]
    fn automatic_resume_does_not_restart_replaced_or_stopped_media() {
        for state in [
            Playback {
                generation: 5,
                ..playing()
            },
            Playback {
                phase: Phase::Stopped,
                ..playing()
            },
        ] {
            let mut policy = AutomaticPause::default();
            assert!(policy.pause(&playing()));
            assert!(!policy.resume(&state));
            assert!(!policy.resume(&playing()));
        }
        let mut policy = AutomaticPause::default();
        assert!(!policy.pause(&Playback {
            phase: Phase::Paused,
            paused: true,
            ..playing()
        }));
        assert!(!policy.resume(&playing()));
    }

    #[test]
    fn resume_handles_paused_buffering_and_cancellation() {
        for phase in [Phase::Buffering, Phase::Seeking] {
            let mut policy = AutomaticPause::default();
            assert!(policy.pause(&playing()));
            assert!(policy.resume(&Playback {
                phase,
                paused: true,
                ..playing()
            }));
        }
        let mut policy = AutomaticPause::default();
        assert!(policy.pause(&playing()));
        policy.cancel();
        assert!(!policy.resume(&playing()));
    }
}
