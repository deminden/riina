//! A window may resume only the media session it paused automatically.
use crate::PlayerWindow;
#[cfg(target_os = "macos")]
use crate::platform;
use riina_core::{Phase, Playback};
use slint::ComponentHandle;
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};
#[cfg(target_os = "macos")]
use {
    super::input::some_player_window_is_focused,
    riina_core::Command,
    slint::winit_030::WinitWindowAccessor,
    std::{cell::RefCell, time::Instant},
};

#[derive(Default)]
pub(super) struct AutomaticPause {
    generation: Option<u64>,
}

impl AutomaticPause {
    pub(super) fn pause(&mut self, playback: &Playback) -> bool {
        if self.generation.is_none() && playback.phase == Phase::Playing && !playback.paused {
            self.generation = Some(playback.generation);
            true
        } else {
            false
        }
    }

    pub(super) fn resume(&mut self, playback: &Playback) -> bool {
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

    #[cfg(target_os = "macos")]
    fn is_pending(&self) -> bool {
        self.generation.is_some()
    }

    fn cancel(&mut self) {
        self.generation = None;
    }
}

pub(super) fn install_pause_on_minimize(
    app: &PlayerWindow,
    native_host: Option<slint::Weak<crate::NativeVideoHost>>,
    playback: Arc<Mutex<Playback>>,
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
            let Some(state) = playback.lock().ok().map(|state| state.clone()) else {
                return;
            };
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

#[cfg(target_os = "macos")]
fn dispatch_system_media_action(
    app: &PlayerWindow,
    action: platform::RemoteMediaAction,
    latest_playback: &Arc<Mutex<Playback>>,
    player: &riina_mpv::CommandSender,
    paused_by_inactive: &mut AutomaticPause,
    application_active: &mut bool,
    player_window_was_focused: bool,
) {
    use platform::RemoteMediaAction as Action;

    match action {
        Action::Play => {
            if !crate::key_binding_preferences::dispatch_system_media_key(
                app,
                "PLAYONLY",
                latest_playback,
            ) {
                app.invoke_set_paused(false);
            }
        }
        Action::Pause => {
            if !crate::key_binding_preferences::dispatch_system_media_key(
                app,
                "PAUSEONLY",
                latest_playback,
            ) {
                app.invoke_set_paused(true);
            }
        }
        Action::Toggle => {
            if !crate::key_binding_preferences::dispatch_system_media_key(
                app,
                "PLAYPAUSE",
                latest_playback,
            ) {
                app.invoke_toggle_pause();
            }
        }
        Action::Stop => {
            if !crate::key_binding_preferences::dispatch_system_media_key(
                app,
                "STOP",
                latest_playback,
            ) {
                app.invoke_stop();
            }
        }
        Action::Next => {
            if !crate::key_binding_preferences::dispatch_system_media_key(
                app,
                "NEXT",
                latest_playback,
            ) {
                app.invoke_playlist_next();
            }
        }
        Action::Previous => {
            if !crate::key_binding_preferences::dispatch_system_media_key(
                app,
                "PREV",
                latest_playback,
            ) {
                app.invoke_playlist_previous();
            }
        }
        Action::Forward(seconds) => {
            if !crate::key_binding_preferences::dispatch_system_media_key(
                app,
                "FORWARD",
                latest_playback,
            ) && seconds.is_finite()
                && seconds > 0.0
            {
                app.invoke_execute_mpv_input_action(format!("seek {seconds} exact").into());
            }
        }
        Action::Backward(seconds) => {
            if !crate::key_binding_preferences::dispatch_system_media_key(
                app,
                "REWIND",
                latest_playback,
            ) && seconds.is_finite()
                && seconds > 0.0
            {
                app.invoke_execute_mpv_input_action(format!("seek -{seconds} exact").into());
            }
        }
        Action::Position(seconds) if seconds.is_finite() && seconds >= 0.0 => {
            app.invoke_seek(seconds as f32);
        }
        Action::Position(_) => {}
        Action::Rate(rate) if rate.is_finite() && rate > 0.0 => {
            let _ = app.invoke_set_speed_text(rate.to_string().into());
        }
        Action::Rate(_) => {}
        Action::Repeat => {
            if let Err(error) = player.send(Command::CycleLoop) {
                app.set_status(error.into());
            }
        }
        Action::Inactive => {
            *application_active = false;
            if app.get_pause_when_inactive()
                && (player_window_was_focused || some_player_window_is_focused())
                && !paused_by_inactive.is_pending()
            {
                let state = latest_playback.lock().ok().map(|state| state.clone());
                if let Some(state) = state
                    && paused_by_inactive.pause(&state)
                {
                    app.invoke_set_paused(true);
                }
            }
        }
        Action::Active => *application_active = true,
        Action::Sleep if app.get_pause_when_goes_to_sleep() => {
            if latest_playback
                .lock()
                .is_ok_and(|state| state.phase == Phase::Playing && !state.paused)
            {
                app.invoke_set_paused(true);
            }
        }
        Action::Sleep => {}
    }
}

#[cfg(target_os = "macos")]
pub(super) struct SystemMedia {
    pub(super) sender: Rc<RefCell<Option<riina_mpv::CommandSender>>>,
    pub(super) controls: Rc<RefCell<platform::SystemMediaControls>>,
    pub(super) timer: Rc<slint::Timer>,
}

#[cfg(target_os = "macos")]
pub(super) fn install_system_media(
    app: &PlayerWindow,
    latest_playback: Arc<Mutex<Playback>>,
) -> SystemMedia {
    let system_media_sender = Rc::new(RefCell::new(None::<riina_mpv::CommandSender>));
    let (system_media_controls, system_media_controls_timer) = {
        let controls = Rc::new(RefCell::new(platform::SystemMediaControls::new(
            platform::playback_library(),
        )));
        let timer = Rc::new(slint::Timer::default());
        let controls_tick = controls.clone();
        let weak = app.as_weak();
        let playback = latest_playback.clone();
        let player = system_media_sender.clone();
        let mut skip_intervals = None::<(String, Option<f64>, Option<f64>)>;
        let mut paused_by_inactive = AutomaticPause::default();
        let mut application_active = true;
        let mut player_window_was_focused = app
            .window()
            .with_winit_window(|window| window.has_focus())
            .unwrap_or(false);
        let mut refresh_skip_intervals_at = Instant::now();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(50),
            move || {
                let Some(app) = weak.upgrade() else { return };
                let profile = app.get_key_binding_current_profile().to_string();
                let active = app.get_loaded() && app.get_key_binding_use_system_media_controls();
                if active
                    && (skip_intervals
                        .as_ref()
                        .is_none_or(|(cached_profile, _, _)| cached_profile != &profile)
                        || Instant::now() >= refresh_skip_intervals_at)
                {
                    let (forward, backward) =
                        crate::key_binding_preferences::system_skip_intervals_for_profile(&profile);
                    skip_intervals = Some((profile.clone(), forward, backward));
                    refresh_skip_intervals_at = Instant::now() + Duration::from_secs(1);
                }
                let (forward, backward) = if active {
                    let (_, forward, backward) = skip_intervals
                        .as_ref()
                        .expect("the active key profile's skip intervals are initialized");
                    (*forward, *backward)
                } else {
                    (None, None)
                };
                controls_tick
                    .borrow_mut()
                    .synchronize(active, &profile, forward, backward);
                let current = playback.lock().ok().map(|state| state.clone());
                if active && let Some(current) = current.as_ref() {
                    controls_tick
                        .borrow_mut()
                        .publish_playback(current, app.get_thumbnail_preview_enabled());
                }
                let actions = controls_tick.borrow().drain_actions().collect::<Vec<_>>();
                for action in actions {
                    if let Some(player) = player.borrow().as_ref() {
                        dispatch_system_media_action(
                            &app,
                            action,
                            &playback,
                            player,
                            &mut paused_by_inactive,
                            &mut application_active,
                            player_window_was_focused,
                        );
                    }
                }
                let focused = app
                    .window()
                    .with_winit_window(|window| window.has_focus())
                    .unwrap_or(false);
                if app.get_pause_when_inactive() {
                    if application_active
                        && !focused
                        && some_player_window_is_focused()
                        && !paused_by_inactive.is_pending()
                    {
                        let state = playback.lock().ok().map(|state| state.clone());
                        if let Some(state) = state
                            && paused_by_inactive.pause(&state)
                        {
                            app.invoke_set_paused(true);
                        }
                    } else if application_active
                        && focused
                        && let Some(state) = playback.lock().ok().map(|state| state.clone())
                        && paused_by_inactive.resume(&state)
                    {
                        app.invoke_set_paused(false);
                    }
                }
                if !app.get_pause_when_inactive() {
                    paused_by_inactive.cancel();
                }
                player_window_was_focused = focused;
            },
        );
        (controls, timer)
    };
    SystemMedia {
        sender: system_media_sender,
        controls: system_media_controls,
        timer: system_media_controls_timer,
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
