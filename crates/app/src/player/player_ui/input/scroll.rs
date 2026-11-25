//! Normalize native wheel phases before routing shared playback gestures.
use crate::{PlayerWindow, platform};
use riina_core::{Command, Phase};
use riina_ui::{MusicModeVolumePanel, PlayerControlActions, PlayerMusicModeState};
use slint::{
    ComponentHandle,
    winit_030::winit::event::{TouchPhase, WindowEvent},
};
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Copy, Default)]
pub(super) struct ScrollInputState {
    axis: Option<bool>,
    native_delta: Option<(f64, f64)>,
    precise: Option<bool>,
    natural_direction: Option<bool>,
    phase_empty: bool,
    phase_began: bool,
    phase_ended: bool,
    momentum: bool,
    momentum_allowed: bool,
    mouse_in_window: bool,
    suppress_current: bool,
    appkit: bool,
}

impl ScrollInputState {
    fn show_music_volume(self, app: &PlayerWindow) {
        let mode = app.global::<PlayerMusicModeState>();
        let current = mode.get_music_mode_volume_panel();
        if !app.get_loaded()
            || !mode.get_music_mode()
            || self.phase_ended
            || self.momentum
            || current == MusicModeVolumePanel::Manual
        {
            return;
        }
        if self.precise == Some(true)
            && (self.appkit || self.phase_began || current == MusicModeVolumePanel::Trackpad)
        {
            mode.set_music_mode_volume_panel(MusicModeVolumePanel::Trackpad);
        } else {
            mode.set_music_mode_volume_panel(MusicModeVolumePanel::Wheel);
            mode.set_music_mode_wheel_volume_revision(
                mode.get_music_mode_wheel_volume_revision().wrapping_add(1),
            );
        }
    }
}

fn finish_music_volume_scroll(app: &PlayerWindow) {
    let mode = app.global::<PlayerMusicModeState>();
    if mode.get_music_mode_volume_panel() == MusicModeVolumePanel::Trackpad {
        mode.set_music_mode_volume_panel(MusicModeVolumePanel::Closed);
    }
}

impl ScrollInputState {
    pub(super) fn note_window_event(
        &mut self,
        window: &slint::Window,
        app: &slint::Weak<PlayerWindow>,
        event: &WindowEvent,
    ) {
        match event {
            WindowEvent::CursorEntered { .. } => self.mouse_in_window = true,
            WindowEvent::CursorLeft { .. } | WindowEvent::Focused(false) => {
                self.mouse_in_window = false;
                self.momentum_allowed = false;
                if let Some(app) = app.upgrade() {
                    finish_music_volume_scroll(&app);
                }
            }
            _ => {}
        }
        if let Some(wheel) = platform::winit_wheel_input(window, event) {
            self.record_winit(wheel);
            if self.gesture_ended()
                && let Some(app) = app.upgrade()
            {
                finish_music_volume_scroll(&app);
            }
        }
    }

    fn record_winit(&mut self, wheel: platform::WinitWheelInput) {
        let platform::WinitWheelInput {
            delta_x,
            delta_y,
            precise,
            phase,
        } = wheel;
        self.appkit = false;
        self.momentum = false;
        self.native_delta = Some((delta_x, delta_y));
        self.precise = Some(precise);
        self.natural_direction = None;
        self.phase_empty = !precise;
        self.phase_began = phase == TouchPhase::Started;
        self.phase_ended = matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled);

        // A wheel event chooses its own axis. A trackpad gesture keeps its
        // first nonzero axis through diagonal movement.
        if !precise || self.phase_began || (!self.phase_ended && self.axis.is_none()) {
            self.axis = first_axis(delta_x, delta_y);
        } else if self.phase_ended {
            self.axis = None;
        }
    }

    pub(super) fn gesture_ended(self) -> bool {
        !self.phase_empty && self.phase_ended
    }

    pub(super) fn suppressed(self) -> bool {
        self.suppress_current
    }
}

fn first_axis(delta_x: f64, delta_y: f64) -> Option<bool> {
    (delta_x != 0.0 || delta_y != 0.0).then_some(delta_x != 0.0)
}

pub(super) fn install_monitor(
    app: &PlayerWindow,
    native_input: Rc<Cell<ScrollInputState>>,
) -> platform::PlayerScrollMonitor {
    let window = app.as_weak();
    platform::PlayerScrollMonitor::install(app, move |event| {
        let mut input = native_input.get();
        input.appkit = true;
        // AppKit's coarse wheel semantics use direction only. Other hosts
        // preserve fractional wheel lines because they lack its precise flag.
        input.native_delta = Some(if event.precise {
            (event.delta_x, event.delta_y)
        } else {
            let unit = |value: f64| if value == 0.0 { 0.0 } else { value.signum() };
            (unit(event.delta_x), unit(event.delta_y))
        });
        input.precise = Some(event.precise);
        input.natural_direction = Some(event.natural_direction);
        input.phase_empty = event.phase_empty;
        input.phase_began = event.phase_began;
        input.phase_ended = event.phase_ended;
        input.momentum = !event.momentum_phase_empty;
        if input.phase_ended
            && let Some(app) = window.upgrade()
        {
            finish_music_volume_scroll(&app);
        }
        if !event.momentum_phase_empty && !input.momentum_allowed {
            input.suppress_current = true;
            native_input.set(input);
            return;
        }
        input.suppress_current = false;
        input.momentum_allowed = event.phase_ended || input.mouse_in_window;
        if event.phase_empty || event.phase_began {
            input.axis = first_axis(event.delta_x, event.delta_y);
        } else if event.phase_ended {
            input.axis = None;
        }
        native_input.set(input);
    })
}

pub(super) fn install_handlers(
    app: &PlayerWindow,
    player: &riina_mpv::CommandSender,
    playback: crate::PlaybackStore,
    scroll_input: Rc<Cell<ScrollInputState>>,
    trackpad_pause_generation: Rc<Cell<Option<u64>>>,
) {
    let scroll_player = player.clone();
    let scroll_playback = playback;
    let scroll_pause_generation = trackpad_pause_generation;
    app.global::<PlayerControlActions>()
        .on_scroll_action_request(scroll_action_handler(
            app.as_weak(),
            scroll_player.clone(),
            scroll_playback.clone(),
            scroll_input.clone(),
            scroll_pause_generation.clone(),
            None,
        ));
    app.global::<PlayerControlActions>()
        .on_seek_scroll_action_request(scroll_action_handler(
            app.as_weak(),
            scroll_player.clone(),
            scroll_playback.clone(),
            scroll_input.clone(),
            scroll_pause_generation.clone(),
            Some(riina_core::control_preferences::ScrollAction::Seek),
        ));
    app.global::<PlayerControlActions>()
        .on_volume_scroll_action_request(scroll_action_handler(
            app.as_weak(),
            scroll_player,
            scroll_playback,
            scroll_input,
            scroll_pause_generation,
            Some(riina_core::control_preferences::ScrollAction::Volume),
        ));
}

fn scroll_action_handler(
    app: slint::Weak<PlayerWindow>,
    player: riina_mpv::CommandSender,
    playback: crate::PlaybackStore,
    scroll_input: Rc<Cell<ScrollInputState>>,
    trackpad_pause_generation: Rc<Cell<Option<u64>>>,
    override_action: Option<riina_core::control_preferences::ScrollAction>,
) -> impl FnMut(f32, f32) -> bool + 'static {
    move |delta_x, delta_y| {
        let Some(app) = app.upgrade() else {
            return false;
        };
        if app.get_scroll_input_suppressed() {
            return true;
        }
        let input = scroll_input.get();
        let horizontal = if input.appkit {
            input.axis.unwrap_or(false)
        } else {
            input.axis.unwrap_or_else(|| delta_x.abs() > delta_y.abs())
        };
        let precise = input
            .precise
            .unwrap_or_else(|| delta_x.abs().max(delta_y.abs()) < 60.0);
        let (mut delta_x, mut delta_y) = input.native_delta.unwrap_or_else(|| {
            let divisor = if precise { 1.0 } else { 60.0 };
            (f64::from(delta_x) / divisor, f64::from(delta_y) / divisor)
        });
        if input.appkit {
            if input.natural_direction == Some(true) {
                delta_y = -delta_y;
            } else {
                delta_x = -delta_x;
            }
        }
        let delta = if horizontal { delta_x } else { delta_y };
        if delta == 0.0 {
            return false;
        }
        let Ok(settings) = crate::control_preferences::current_player(&app) else {
            return false;
        };
        let action = override_action.unwrap_or(if horizontal {
            settings.horizontal_scroll_action
        } else {
            settings.vertical_scroll_action
        });
        let effect = riina_core::control_preferences::scroll_effect_for_input(
            &settings,
            action,
            horizontal,
            delta,
            precise,
            input.phase_empty,
        );
        let command = match effect {
            riina_core::control_preferences::ScrollEffect::None => return false,
            riina_core::control_preferences::ScrollEffect::AdjustVolume(delta) => {
                Command::AdjustVolume {
                    delta,
                    boost: app
                        .global::<riina_ui::PlayerPreferences>()
                        .get_volume_boost(),
                }
            }
            riina_core::control_preferences::ScrollEffect::Seek(seconds) => {
                if !input.phase_empty
                    && input.phase_began
                    && trackpad_pause_generation.get().is_none()
                {
                    let generation = {
                        let state = riina_core::sync::lock(&playback);
                        (state.phase == Phase::Playing && !state.paused).then_some(state.generation)
                    };
                    if let Some(generation) = generation {
                        match player.set_paused(true) {
                            Ok(()) => trackpad_pause_generation.set(Some(generation)),
                            Err(error) => crate::diagnostics::report(&app, error),
                        }
                    }
                }
                Command::SeekWithType {
                    seconds,
                    seek_type: settings.seek_type,
                }
            }
            riina_core::control_preferences::ScrollEffect::MultiplySpeed(factor) => {
                let current = f64::from(
                    app.global::<riina_ui::PlaybackTimelineState>()
                        .get_speed_value(),
                )
                .clamp(0.05, 4.0);
                let target = (current * factor).clamp(0.05, 4.0);
                Command::MultiplySpeed(target / current)
            }
        };
        app.global::<PlayerControlActions>().invoke_activity();
        let show_music_volume =
            override_action.is_none() && matches!(command, Command::AdjustVolume { .. });
        if let Err(error) = player.send(command) {
            crate::diagnostics::report(&app, error);
        } else if show_music_volume {
            input.show_music_volume(&app);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_trackpad_feedback_closes_on_end_without_retiring_manual_volume()
    -> Result<(), slint::PlatformError> {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new()?;
        app.set_loaded(true);
        let mode = app.global::<PlayerMusicModeState>();
        mode.set_music_mode(true);

        let mut input = ScrollInputState::default();
        input.record_winit(platform::WinitWheelInput {
            delta_x: 0.0,
            delta_y: 12.0,
            precise: true,
            phase: TouchPhase::Started,
        });
        input.show_music_volume(&app);
        assert_eq!(
            mode.get_music_mode_volume_panel(),
            MusicModeVolumePanel::Trackpad
        );
        finish_music_volume_scroll(&app);
        assert_eq!(
            mode.get_music_mode_volume_panel(),
            MusicModeVolumePanel::Closed
        );

        input.momentum = true;
        input.show_music_volume(&app);
        assert_eq!(
            mode.get_music_mode_volume_panel(),
            MusicModeVolumePanel::Closed
        );
        mode.set_music_mode_volume_panel(MusicModeVolumePanel::Manual);
        finish_music_volume_scroll(&app);
        assert_eq!(
            mode.get_music_mode_volume_panel(),
            MusicModeVolumePanel::Manual
        );
        Ok(())
    }
}
