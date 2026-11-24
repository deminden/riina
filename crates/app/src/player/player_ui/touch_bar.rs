//! Route native Touch Bar actions through the owning player's playback and profile.
use super::{PlayerContext, thumbnail_actions::NativeThumbnails};
use crate::{PlayerWindow, osc_preferences, platform::touch_bar as native, preference_store};
use riina_core::{
    Command, Phase, Playback,
    sync::lock,
    touch_bar::{ScrubOrigin, ScrubSession, Settings, seekable_duration},
};
use riina_ui::{PlayerMenuActions, TouchBarText};
use slint::{ComponentHandle, Model};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

#[derive(Default)]
pub(super) struct ApplicationTouchBars {
    players: RefCell<Vec<Weak<Session>>>,
}

impl ApplicationTouchBars {
    pub(super) fn install(
        self: &Rc<Self>,
        app: &PlayerWindow,
        context: PlayerContext,
        thumbnails: NativeThumbnails,
    ) -> Rc<Session> {
        let session = Rc::new(Session {
            mode: Cell::new(context.preferences().snapshot().touch_bar),
            previews: Rc::new(native::Preview::new(
                context.playback().clone(),
                thumbnails.controller,
            )),
            application: self.clone(),
            context,
            bar: RefCell::default(),
            scrub: RefCell::default(),
        });
        let mut players = self.players.borrow_mut();
        players.retain(|entry| entry.strong_count() != 0);
        if let Some(existing) = players.iter().filter_map(Weak::upgrade).find(|existing| {
            existing.context.preferences_path() == session.context.preferences_path()
        }) {
            session.adopt_mode(existing.mode.get());
        }
        players.push(Rc::downgrade(&session));
        drop(players);

        let target = Rc::downgrade(&session);
        app.on_touch_bar_presentation_changed(move || {
            if let Some(session) = target.upgrade() {
                session.refresh_current();
            }
        });
        let target = Rc::downgrade(&session);
        thumbnails.redraw.replace(Some(Box::new(move || {
            if let Some(session) = target.upgrade()
                && let Some(bar) = session.bar.borrow().as_ref()
            {
                bar.redraw_previews();
            }
        })));
        session.refresh_current();
        session
    }

    fn toggle(&self, context: &PlayerContext) -> Result<(), String> {
        let path = context.preferences_path();
        let baseline = context.preferences().snapshot();
        let mut committed = baseline.touch_bar;
        preference_store::update(path, &baseline, |latest| {
            latest.touch_bar.show_remaining_time = !latest.touch_bar.show_remaining_time;
            committed = latest.touch_bar;
            Ok(())
        })?;
        self.players.borrow_mut().retain(|entry| {
            let Some(player) = entry.upgrade() else {
                return false;
            };
            if player.context.preferences_path() == path {
                player.adopt_mode(committed);
                player.refresh_current();
            }
            true
        });
        Ok(())
    }
}

pub(super) struct Session {
    context: PlayerContext,
    application: Rc<ApplicationTouchBars>,
    previews: Rc<native::Preview>,
    bar: RefCell<Option<native::PlayerTouchBar>>,
    scrub: RefCell<ScrubSession>,
    mode: Cell<Settings>,
}

impl Session {
    fn adopt_mode(&self, mode: Settings) {
        self.context.preferences().adopt_touch_bar_mode(mode);
        self.mode.set(mode);
    }

    pub(super) fn observe(self: &Rc<Self>, app: &PlayerWindow, playback: &Playback) {
        if self.bar.borrow().is_none() {
            self.attach(app);
        }
        if let Some(bar) = self.bar.borrow().as_ref() {
            let (elapsed, other) = self
                .mode
                .get()
                .clocks(osc_preferences::player_controller_options(app), playback);
            let fraction = seekable_duration(playback).map(|duration| {
                playback
                    .position
                    .filter(|position| position.is_finite())
                    .map_or(0.0, |position| (position / duration).clamp(0.0, 1.0))
            });
            let loaded = active(playback);
            bar.refresh(
                native::State {
                    loaded,
                    paused: !loaded || playback.paused,
                    fraction,
                    elapsed,
                    other,
                },
                app.get_fullscreen() && app.get_escape_exits_fullscreen(),
            );
        }
    }

    fn refresh_current(self: &Rc<Self>) {
        if let Some(app) = self.context.app() {
            let playback = lock(self.context.playback()).clone();
            self.observe(&app, &playback);
        }
    }

    fn attach(self: &Rc<Self>, app: &PlayerWindow) {
        let captions = app
            .global::<TouchBarText>()
            .get_captions()
            .iter()
            .map(|item| (item.identifier.to_string(), item.caption.to_string()))
            .collect();
        let buttons = Rc::downgrade(self);
        let seeking = Rc::downgrade(self);
        let tracking = Rc::downgrade(self);
        let bar = native::PlayerTouchBar::attach(
            app,
            self.previews.clone(),
            captions,
            move |action| {
                if let Some(session) = buttons.upgrade() {
                    session.button(action);
                }
            },
            move |fraction| {
                if let Some(session) = seeking.upgrade() {
                    session.seek(fraction);
                }
            },
            move |tracking_now| {
                if let Some(session) = tracking.upgrade() {
                    session.track(tracking_now);
                }
            },
        );
        self.bar.replace(bar);
    }

    fn button(&self, action: native::Action) {
        use native::Action;
        let Some(app) = self.context.app() else {
            return;
        };
        let menu = app.global::<PlayerMenuActions>();
        match action {
            Action::TogglePlayback => menu.invoke_toggle_pause(),
            Action::VolumeUp => menu.invoke_adjust_volume(5.0),
            Action::VolumeDown => menu.invoke_adjust_volume(-5.0),
            Action::Forward15 => menu.invoke_seek_relative(15.0),
            Action::Forward30 => menu.invoke_seek_relative(30.0),
            Action::Back15 => menu.invoke_seek_relative(-15.0),
            Action::Back30 => menu.invoke_seek_relative(-30.0),
            Action::Next => menu.invoke_playlist_next(),
            Action::Previous => menu.invoke_playlist_previous(),
            Action::TogglePictureInPicture => menu.invoke_toggle_picture_in_picture(),
            Action::ExitFullscreen => menu.invoke_request_fullscreen(false),
            Action::ToggleTimeDisplay => {
                if let Err(error) = self.application.toggle(&self.context) {
                    self.context.report(error);
                }
            }
            Action::Rewind | Action::FastForward => {
                let playback = lock(self.context.playback()).clone();
                menu.invoke_speed(if action == Action::Rewind { 0.5 } else { 2.0 });
                if active(&playback) && playback.paused {
                    self.send(playback.generation, Command::SetPaused(false));
                }
            }
        }
    }

    fn seek(&self, fraction: f64) {
        let Some(app) = self.context.app() else {
            return;
        };
        let playback = lock(self.context.playback()).clone();
        if fraction.is_finite()
            && self.scrub.borrow().allows_seek(origin(&app, &playback))
            && let Some(duration) = seekable_duration(&playback)
        {
            // Seeking exactly to EOF can advance the playlist while the finger is down.
            let last_position = (duration - 0.001).max(0.0);
            self.send(
                playback.generation,
                Command::Seek {
                    seconds: (fraction.clamp(0.0, 1.0) * duration).min(last_position),
                    absolute: true,
                },
            );
        }
    }

    fn track(&self, tracking: bool) {
        let Some(app) = self.context.app() else {
            return;
        };
        let playback = lock(self.context.playback()).clone();
        let origin = origin(&app, &playback);
        let paused = if tracking {
            self.scrub
                .borrow_mut()
                .begin(
                    origin,
                    seekable_duration(&playback).is_some() && !playback.paused,
                )
                .then_some(true)
        } else {
            self.scrub.borrow_mut().end(origin).then_some(false)
        };
        if let Some(paused) = paused {
            self.send(playback.generation, Command::SetPaused(paused));
        }
    }

    fn send(&self, generation: u64, command: Command) {
        self.context.activity();
        if let Err(error) = self.context.commands().send_for_media(generation, command) {
            self.context.report(error);
        }
    }
}

fn active(playback: &Playback) -> bool {
    !matches!(
        playback.phase,
        Phase::Loading | Phase::Stopped | Phase::Failed
    )
}

fn origin(app: &PlayerWindow, playback: &Playback) -> ScrubOrigin {
    ScrubOrigin {
        media: playback.generation,
        open_request: app.get_media_request_generation(),
    }
}
