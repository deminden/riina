//! A window may resume only the media session it paused automatically.
use crate::PlayerWindow;
use riina_core::{Phase, Playback};
use slint::ComponentHandle;
use std::cell::RefCell;

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

pub(super) fn install_pause_on_minimize(
    app: &PlayerWindow,
    native_host: Option<slint::Weak<crate::NativeVideoHost>>,
    playback: crate::PlaybackStore,
) {
    let weak_app = app.as_weak();
    let pause = RefCell::new(AutomaticPause::default());
    let on_minimized = move |minimized: bool| {
        let Some(app) = weak_app.upgrade() else {
            return;
        };
        let action = {
            let mut pause = pause.borrow_mut();
            if app
                .global::<riina_ui::PlayerPreferences>()
                .get_pause_when_minimized()
            {
                let state = riina_core::sync::lock(&playback);
                if minimized {
                    pause.pause(&state).then_some(true)
                } else {
                    pause.resume(&state).then_some(false)
                }
            } else {
                pause.cancel();
                None
            }
        };
        if let Some(paused) = action {
            app.global::<riina_ui::PlayerControlActions>()
                .invoke_set_paused(paused);
        }
    };
    // The framed owner represents Windows playback. The separate control
    // popup can hide or minimize without changing the player's pause policy.
    if let Some(host) = native_host.and_then(|host| host.upgrade()) {
        host.on_window_minimized_changed(on_minimized);
    } else {
        app.on_window_minimized_changed(on_minimized);
    }
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
    fn native_owner_minimize_pauses_and_resumes_the_playing_generation()
    -> Result<(), slint::PlatformError> {
        use std::{rc::Rc, time::Duration};

        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new()?;
        let owner = crate::NativeVideoHost::new()?;
        let playback = std::sync::Arc::new(std::sync::Mutex::new(std::sync::Arc::new(playing())));
        install_pause_on_minimize(&app, Some(owner.as_weak()), playback);
        let decisions = Rc::new(RefCell::new(Vec::new()));
        let observed = decisions.clone();
        app.global::<riina_ui::PlayerControlActions>()
            .on_set_paused(move |paused| observed.borrow_mut().push(paused));
        let flush_changes = || i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);

        app.global::<riina_ui::PlayerPreferences>()
            .set_pause_when_minimized(true);
        app.window().set_minimized(true);
        flush_changes();
        assert!(decisions.borrow().is_empty());

        owner.window().set_minimized(true);
        flush_changes();
        assert_eq!(*decisions.borrow(), [true]);
        owner.window().set_minimized(false);
        flush_changes();
        assert_eq!(*decisions.borrow(), [true, false]);

        app.global::<riina_ui::PlayerPreferences>()
            .set_pause_when_minimized(false);
        owner.window().set_minimized(true);
        flush_changes();
        app.global::<riina_ui::PlayerPreferences>()
            .set_pause_when_minimized(true);
        flush_changes();
        assert_eq!(*decisions.borrow(), [true, false]);
        owner.window().set_minimized(false);
        flush_changes();
        assert_eq!(*decisions.borrow(), [true, false]);
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
