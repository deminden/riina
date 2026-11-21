//! A window may resume only the media session it paused automatically.
use crate::PlayerWindow;
use riina_core::{Phase, Playback};
use slint::ComponentHandle;
use std::{rc::Rc, time::Duration};

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
) -> Rc<slint::Timer> {
    let timer = Rc::new(slint::Timer::default());
    let weak_app = app.as_weak();
    let mut minimized = false;
    let mut pause = AutomaticPause::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(50),
        move || {
            let Some(app) = weak_app.upgrade() else {
                return;
            };
            // Native Windows playback has a framed owner and a separate controls
            // surface. Only the owner's minimization represents the player.
            let now_minimized = native_host
                .as_ref()
                .and_then(slint::Weak::upgrade)
                .map_or_else(
                    || app.window().is_minimized(),
                    |host| host.window().is_minimized(),
                );
            let state = riina_core::sync::lock(&playback).clone();
            if now_minimized && !minimized && app.get_pause_when_minimized() && pause.pause(&state)
            {
                app.invoke_set_paused(true);
            } else if !now_minimized && minimized {
                if app.get_pause_when_minimized() && pause.resume(&state) {
                    app.invoke_set_paused(false);
                } else {
                    pause.cancel();
                }
            }
            minimized = now_minimized;
        },
    );
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
