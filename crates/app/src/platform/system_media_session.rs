//! Keep Now Playing controls and application focus events with the player session.
use crate::{PlaybackStore, PlayerWindow};
use riina_mpv::CommandSender;

#[cfg(not(target_os = "macos"))]
pub(crate) struct SystemMediaSession;

#[cfg(not(target_os = "macos"))]
impl SystemMediaSession {
    pub(crate) fn new(_: &PlayerWindow, _: PlaybackStore) -> Self {
        Self
    }
    pub(crate) fn bind(&self, _: &CommandSender) {}
    pub(crate) fn prepare_to_close(&self) {}
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use crate::{
        platform, platform::player_input::some_player_window_is_focused,
        player_ui::lifecycle::AutomaticPause,
    };
    use riina_core::{Command, Phase};
    use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
    use std::{
        cell::RefCell,
        rc::Rc,
        time::{Duration, Instant},
    };

    fn dispatch_system_media_action(
        app: &PlayerWindow,
        action: platform::RemoteMediaAction,
        latest_playback: &crate::PlaybackStore,
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
                    app.global::<riina_ui::PlayerControlActions>()
                        .invoke_set_paused(false);
                }
            }
            Action::Pause => {
                if !crate::key_binding_preferences::dispatch_system_media_key(
                    app,
                    "PAUSEONLY",
                    latest_playback,
                ) {
                    app.global::<riina_ui::PlayerControlActions>()
                        .invoke_set_paused(true);
                }
            }
            Action::Toggle => {
                if !crate::key_binding_preferences::dispatch_system_media_key(
                    app,
                    "PLAYPAUSE",
                    latest_playback,
                ) {
                    app.global::<PlayerMenuActions>().invoke_toggle_pause();
                }
            }
            Action::Stop => {
                if !crate::key_binding_preferences::dispatch_system_media_key(
                    app,
                    "STOP",
                    latest_playback,
                ) {
                    app.global::<PlayerMenuActions>().invoke_stop();
                }
            }
            Action::Next => {
                if !crate::key_binding_preferences::dispatch_system_media_key(
                    app,
                    "NEXT",
                    latest_playback,
                ) {
                    app.global::<PlayerMenuActions>().invoke_playlist_next();
                }
            }
            Action::Previous => {
                if !crate::key_binding_preferences::dispatch_system_media_key(
                    app,
                    "PREV",
                    latest_playback,
                ) {
                    app.global::<PlayerMenuActions>().invoke_playlist_previous();
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
                    app.global::<riina_ui::PlayerControlActions>()
                        .invoke_execute_mpv_input_action(format!("seek {seconds} exact").into());
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
                    app.global::<riina_ui::PlayerControlActions>()
                        .invoke_execute_mpv_input_action(format!("seek -{seconds} exact").into());
                }
            }
            Action::Position(seconds) if seconds.is_finite() && seconds >= 0.0 => {
                app.global::<PlayerMenuActions>()
                    .invoke_seek(seconds as f32);
            }
            Action::Position(_) => {}
            Action::Rate(rate) if rate.is_finite() && rate > 0.0 => {
                let _ = app
                    .global::<riina_ui::PlayerSidebarActions>()
                    .invoke_set_speed_text(rate.to_string().into());
            }
            Action::Rate(_) => {}
            Action::Repeat => {
                if let Err(error) = player.send(Command::CycleLoop) {
                    app.set_status(error.into());
                }
            }
            Action::Inactive => {
                *application_active = false;
                if app
                    .global::<riina_ui::PlayerPreferences>()
                    .get_pause_when_inactive()
                    && (player_window_was_focused || some_player_window_is_focused())
                    && !paused_by_inactive.is_pending()
                {
                    let state = riina_core::sync::lock(latest_playback).clone();
                    if paused_by_inactive.pause(&state) {
                        app.global::<riina_ui::PlayerControlActions>()
                            .invoke_set_paused(true);
                    }
                }
            }
            Action::Active => *application_active = true,
            Action::Sleep
                if app
                    .global::<riina_ui::PlayerPreferences>()
                    .get_pause_when_goes_to_sleep() =>
            {
                let should_pause = {
                    let state = riina_core::sync::lock(latest_playback);
                    state.phase == Phase::Playing && !state.paused
                };
                if should_pause {
                    app.global::<riina_ui::PlayerControlActions>()
                        .invoke_set_paused(true);
                }
            }
            Action::Sleep => {}
        }
    }

    pub(crate) struct SystemMediaSession {
        controls: Rc<RefCell<platform::SystemMediaControls>>,
        _timer: Rc<slint::Timer>,
        sender: Rc<RefCell<Option<CommandSender>>>,
    }

    impl SystemMediaSession {
        pub(crate) fn new(app: &PlayerWindow, latest_playback: PlaybackStore) -> Self {
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
                        let profile = app
                            .global::<riina_ui::PlayerPreferences>()
                            .get_key_binding_current_profile()
                            .to_string();
                        let active = app.get_loaded()
                            && app
                                .global::<riina_ui::PlayerPreferences>()
                                .get_key_binding_use_system_media_controls();
                        if active
                            && (skip_intervals
                                .as_ref()
                                .is_none_or(|(cached_profile, _, _)| cached_profile != &profile)
                                || Instant::now() >= refresh_skip_intervals_at)
                        {
                            let (forward, backward) =
                                crate::key_binding_preferences::system_skip_intervals_for_profile(
                                    &profile,
                                );
                            skip_intervals = Some((profile.clone(), forward, backward));
                            refresh_skip_intervals_at = Instant::now() + Duration::from_secs(1);
                        }
                        let (forward, backward) = skip_intervals
                            .as_ref()
                            .filter(|_| active)
                            .map_or((None, None), |(_, forward, backward)| (*forward, *backward));
                        controls_tick
                            .borrow_mut()
                            .synchronize(active, &profile, forward, backward);
                        if active {
                            let current = riina_core::sync::lock(&playback).clone();
                            controls_tick.borrow_mut().publish_playback(
                                &current,
                                app.global::<riina_ui::PlayerPreferences>()
                                    .get_thumbnail_preview_enabled(),
                            );
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
                        if app
                            .global::<riina_ui::PlayerPreferences>()
                            .get_pause_when_inactive()
                        {
                            if application_active
                                && !focused
                                && some_player_window_is_focused()
                                && !paused_by_inactive.is_pending()
                            {
                                let state = riina_core::sync::lock(&playback).clone();
                                if paused_by_inactive.pause(&state) {
                                    app.global::<riina_ui::PlayerControlActions>()
                                        .invoke_set_paused(true);
                                }
                            } else if application_active && focused {
                                let state = riina_core::sync::lock(&playback).clone();
                                if paused_by_inactive.resume(&state) {
                                    app.global::<riina_ui::PlayerControlActions>()
                                        .invoke_set_paused(false);
                                }
                            }
                        }
                        if !app
                            .global::<riina_ui::PlayerPreferences>()
                            .get_pause_when_inactive()
                        {
                            paused_by_inactive.cancel();
                        }
                        player_window_was_focused = focused;
                    },
                );
                (controls, timer)
            };
            Self {
                controls: system_media_controls,
                _timer: system_media_controls_timer,
                sender: system_media_sender,
            }
        }

        pub(crate) fn bind(&self, sender: &CommandSender) {
            *self.sender.borrow_mut() = Some(sender.clone());
        }

        pub(crate) fn prepare_to_close(&self) {
            self.controls
                .borrow_mut()
                .synchronize(false, "", None, None);
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) use macos::SystemMediaSession;
