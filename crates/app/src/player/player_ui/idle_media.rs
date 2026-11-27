//! Hand new media to an unused player that is already waiting for a file.
use super::{InitialMedia, UiSession, observer_state::ObserverId, startup};
use crate::{PlayerWindow, media_open, player::surface::PlayerDisplay};
use riina_mpv::CommandSender;
use slint::ComponentHandle;
use std::{cell::Cell, rc::Rc};

pub(crate) struct IdleMediaTarget {
    app: PlayerWindow,
    display: PlayerDisplay,
    commands: CommandSender,
    observer: ObserverId,
    available: Rc<Cell<bool>>,
}

impl UiSession {
    pub(crate) fn idle_media_target(
        &self,
        app: &PlayerWindow,
        display: PlayerDisplay,
    ) -> Option<IdleMediaTarget> {
        let available = self.idle_media_available.as_ref()?;
        if !available.get()
            || !app.get_player_manager_ready()
            || app.get_media_request_generation() != 0
            || app
                .global::<riina_ui::PlayerDialogState>()
                .get_dialog_open()
            || media_open::has_playlist(app)
            || riina_core::sync::lock(&self.preference_state.latest_playback).phase
                != riina_core::Phase::Stopped
            || display.switchable(app).is_none()
        {
            return None;
        }
        Some(IdleMediaTarget {
            app: app.clone_strong(),
            display,
            commands: self.player()?.command_sender(),
            observer: self.observer_state.id(),
            available: available.clone(),
        })
    }
}

impl IdleMediaTarget {
    pub(crate) fn open(self, media: InitialMedia) -> Result<(), String> {
        // Reserve before making the playback window visible. Showing it can
        // deliver window callbacks before the media request is dispatched.
        if !self.available.replace(false) {
            return Err("The player is already opening media".into());
        }
        if let Err(error) = self.display.reveal(&self.app) {
            self.available.set(true);
            return Err(error.to_string());
        }
        startup::open_initial_media(&self.app, media, self.commands, self.observer);
        self.display.focus(&self.app);
        Ok(())
    }
}
