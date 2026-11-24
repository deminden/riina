//! Route the on-screen arrows and macOS controls through one player session.
use super::PlayerContext;
use crate::PlayerWindow;
use riina_core::{
    Command, Phase, Playback,
    osc_preferences::{ArrowAction, ArrowDirection, ArrowSpeed},
    sync::lock,
};
use riina_ui::{
    ControllerArrowPhase, PlayerControlActions, PlayerControlsState, PlayerMenuActions,
    PlayerPreferences,
};
use slint::ComponentHandle;
use std::{cell::RefCell, rc::Rc, time::Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Media {
    generation: u64,
    open_request: i32,
}

#[derive(Default)]
struct State {
    media: Option<Media>,
    speed: ArrowSpeed,
    pressed_at: Option<Instant>,
}

struct Session {
    context: PlayerContext,
    state: RefCell<State>,
}

pub(super) fn install(app: &PlayerWindow, context: PlayerContext) {
    let session = Rc::new(Session {
        context,
        state: RefCell::default(),
    });
    let reset = session.clone();
    app.global::<PlayerControlActions>()
        .on_reset_controller_arrow(move || reset.reset());
    let reset = session.clone();
    app.global::<PlayerPreferences>()
        .on_controller_arrow_action_changed(move || reset.reset());
    let native = session.clone();
    crate::platform::bind_controller_arrow_pressure(app, move |stage| {
        native.pressure(i64::from(stage))
    });
    app.global::<PlayerControlActions>()
        .on_controller_arrow(move |left, phase| {
            session.handle(
                if left {
                    ArrowDirection::Left
                } else {
                    ArrowDirection::Right
                },
                phase,
            );
        });
}

impl Session {
    fn handle(&self, direction: ArrowDirection, phase: ControllerArrowPhase) {
        let Some(app) = self.context.app() else {
            return;
        };
        let playback = lock(self.context.playback()).clone();
        if !has_active_media(&playback) {
            self.reset_for(&app, &playback);
            return;
        }
        let menu = app.global::<PlayerMenuActions>();
        match (arrow_action(&app), phase) {
            (ArrowAction::Speed, _) => self.change_speed(&app, &playback, direction, phase),
            (ArrowAction::Playlist, ControllerArrowPhase::Activated) => match direction {
                ArrowDirection::Left => menu.invoke_playlist_previous(),
                ArrowDirection::Right => menu.invoke_playlist_next(),
            },
            (ArrowAction::Seek, ControllerArrowPhase::Activated) => {
                menu.invoke_seek_relative(direction.seek_seconds());
            }
            _ => {}
        }
    }

    fn change_speed(
        &self,
        app: &PlayerWindow,
        playback: &Playback,
        direction: ArrowDirection,
        phase: ControllerArrowPhase,
    ) {
        let media = media(app, playback);
        let start = matches!(
            phase,
            ControllerArrowPhase::Pressed | ControllerArrowPhase::Activated
        );
        let mut state = self.state.borrow_mut();
        if state.media != Some(media) {
            *state = State {
                media: Some(media),
                ..State::default()
            };
        }
        if start && state.pressed_at.is_none() && playback.paused {
            state.speed.reset();
        }
        let previous = state.speed.speed().to_bits();
        let speed = match phase {
            ControllerArrowPhase::Pressed => {
                let speed = state.speed.press(direction);
                if speed.is_some() {
                    state.pressed_at = Some(Instant::now());
                }
                speed
            }
            ControllerArrowPhase::Released => {
                let elapsed = state
                    .pressed_at
                    .map(|press| press.elapsed())
                    .unwrap_or_default();
                let speed = state.speed.release(direction, elapsed);
                if speed.is_some() {
                    state.pressed_at = None;
                }
                speed
            }
            ControllerArrowPhase::Cancelled => {
                state.pressed_at = None;
                state.speed.cancel()
            }
            ControllerArrowPhase::Activated => state
                .speed
                .press(direction)
                .and_then(|_| state.speed.release(direction, Default::default())),
        };
        let pressed = state.pressed_at.is_some();
        drop(state);
        app.global::<PlayerControlsState>()
            .set_controller_arrow_pressed(pressed);
        if let Some(speed) = speed {
            if speed.to_bits() != previous || start {
                self.send(media, Command::Speed(speed));
            }
            if start && playback.paused {
                self.send(media, Command::SetPaused(false));
            }
        }
    }

    fn reset(&self) {
        if let Some(app) = self.context.app() {
            let playback = lock(self.context.playback()).clone();
            self.reset_for(&app, &playback);
        }
    }

    fn reset_for(&self, app: &PlayerWindow, playback: &Playback) {
        let previous = self.state.take();
        app.global::<PlayerControlsState>()
            .set_controller_arrow_pressed(false);
        if let Some(origin) = previous.media
            && previous.pressed_at.is_some()
            && origin == media(app, playback)
            && has_active_media(playback)
        {
            self.send(origin, Command::Speed(1.0));
        }
    }

    fn pressure(&self, stage: i64) -> bool {
        let Some(app) = self.context.app() else {
            return false;
        };
        let playback = lock(self.context.playback()).clone();
        let media = media(&app, &playback);
        let mut state = self.state.borrow_mut();
        if state.pressed_at.is_none() {
            return false;
        }
        if !has_active_media(&playback) || state.media != Some(media) {
            return true;
        }
        let speed = state.speed.pressure(stage);
        drop(state);
        if let Some(speed) = speed {
            self.send(media, Command::Speed(speed));
        }
        true
    }

    fn send(&self, media: Media, command: Command) {
        self.context.activity();
        if let Err(error) = self
            .context
            .commands()
            .send_for_media(media.generation, command)
        {
            self.context.report(error);
        }
    }
}

fn arrow_action(app: &PlayerWindow) -> ArrowAction {
    ArrowAction::from_index(app.global::<PlayerPreferences>().get_control_arrow_action())
        .unwrap_or_default()
}

fn media(app: &PlayerWindow, playback: &Playback) -> Media {
    Media {
        generation: playback.generation,
        open_request: app.get_media_request_generation(),
    }
}

fn has_active_media(playback: &Playback) -> bool {
    !matches!(
        playback.phase,
        Phase::Loading | Phase::Stopped | Phase::Failed
    )
}
