//! Dispatch shell requests through the active input profile on the UI thread.
use super::{PlaybackStore, PlayerWindow};
use ::windows::Media::SystemMediaTransportControlsButton as Button;
use riina_core::sync::lock;
use riina_ui::{PlayerControlActions, PlayerMenuActions, PlayerPreferences};
use slint::{ComponentHandle, Weak};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

#[derive(Clone)]
pub(super) struct Destination {
    app: Weak<PlayerWindow>,
    latest: PlaybackStore,
    enabled: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
}

impl Destination {
    pub(super) fn new(
        app: Weak<PlayerWindow>,
        latest: PlaybackStore,
        enabled: Arc<AtomicBool>,
        generation: Arc<AtomicU64>,
    ) -> Self {
        Self {
            app,
            latest,
            enabled,
            generation,
        }
    }

    pub(super) fn post(&self, action: Action) {
        if !self.enabled.load(Ordering::Acquire) {
            return;
        }
        let pending = self.clone();
        let generation = self.generation.load(Ordering::Acquire);
        let _ = slint::invoke_from_event_loop(move || {
            if pending.enabled.load(Ordering::Acquire)
                && let Some(app) = pending.app.upgrade()
                && app.get_loaded()
                && app
                    .global::<PlayerPreferences>()
                    .get_key_binding_use_system_media_controls()
                && lock(&pending.latest).generation == generation
            {
                action.apply(&app, &pending.latest);
            }
        });
    }
}

#[derive(Clone, Copy)]
pub(super) enum Action {
    Button(Button),
    Position(f64),
}

impl Action {
    fn apply(self, app: &PlayerWindow, latest: &PlaybackStore) {
        use super::super::key_actions::{
            dispatch_system_media_key, system_skip_intervals_for_profile,
        };
        let controls = app.global::<PlayerControlActions>();
        let menu = app.global::<PlayerMenuActions>();
        let Action::Button(button) = self else {
            if let Action::Position(seconds) = self
                && lock(latest).seekable
            {
                menu.invoke_seek(seconds as f32);
            }
            return;
        };
        let key = match button {
            Button::Play => "PLAYONLY",
            Button::Pause => "PAUSEONLY",
            Button::Stop => "STOP",
            Button::Next => "NEXT",
            Button::Previous => "PREV",
            Button::FastForward => "FORWARD",
            Button::Rewind => "REWIND",
            _ => return,
        };
        if dispatch_system_media_key(app, key) {
            return;
        }
        match button {
            Button::Play => controls.invoke_set_paused(false),
            Button::Pause => controls.invoke_set_paused(true),
            Button::Stop => menu.invoke_stop(),
            Button::Next => menu.invoke_playlist_next(),
            Button::Previous => menu.invoke_playlist_previous(),
            Button::FastForward | Button::Rewind => {
                let profile = app
                    .global::<PlayerPreferences>()
                    .get_key_binding_current_profile();
                let (forward, backward) = system_skip_intervals_for_profile(profile.as_str());
                let seconds = if button == Button::FastForward {
                    forward.unwrap_or(15.0)
                } else {
                    -backward.unwrap_or(15.0)
                };
                controls.invoke_execute_mpv_input_action(format!("seek {seconds} exact").into());
            }
            _ => {}
        }
    }
}
