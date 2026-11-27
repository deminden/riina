//! Hand new media to a player whose previous work has finished.
use super::{InitialMedia, UiSession, observer_state::ObserverId, startup};
use crate::{PlayerWindow, media_open, player::surface::PlayerDisplay};
use riina_mpv::CommandSender;
use slint::ComponentHandle;
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Availability {
    Unused,
    AfterStop(u32),
    Unavailable,
}

impl Availability {
    fn is_ready(self, request: u32, playback: &riina_core::Playback) -> bool {
        playback.phase == riina_core::Phase::Stopped
            && playback.playlist.is_empty()
            && playback.active_playlist_entry.is_none()
            && match self {
                Self::Unused => request == 0 && playback.generation == 0,
                Self::AfterStop(stop) => request == stop && playback.stopped_by == Some(stop),
                Self::Unavailable => false,
            }
    }
}

pub(crate) struct IdleMediaTarget {
    app: PlayerWindow,
    display: PlayerDisplay,
    commands: CommandSender,
    observer: ObserverId,
    available: Rc<Cell<Availability>>,
}

impl UiSession {
    pub(crate) fn idle_media_target(
        &self,
        app: &PlayerWindow,
        display: PlayerDisplay,
    ) -> Option<IdleMediaTarget> {
        let observer = self.observer_state.id();
        let available = observer.idle_media()?;
        if !app.get_player_manager_ready()
            || app
                .global::<riina_ui::PlayerDialogState>()
                .get_dialog_open()
            || media_open::has_playlist(app)
            || !available.get().is_ready(
                app.get_media_request_generation().cast_unsigned(),
                &riina_core::sync::lock(&self.preference_state.latest_playback),
            )
            || display.switchable(app).is_none()
        {
            return None;
        }
        Some(IdleMediaTarget {
            app: app.clone_strong(),
            display,
            commands: self.player()?.command_sender(),
            observer,
            available,
        })
    }
}

impl IdleMediaTarget {
    pub(crate) fn open(self, media: InitialMedia) -> Result<(), String> {
        // Reserve before making the playback window visible. Showing it can
        // deliver window callbacks before the media request is dispatched.
        let request = self.app.get_media_request_generation();
        let available = self.available.replace(Availability::Unavailable);
        if available == Availability::Unavailable {
            return Err("The player is already opening media".into());
        }
        if let Err(error) = self.display.reveal(&self.app) {
            if self.app.get_media_request_generation() == request {
                self.available.set(available);
            }
            return Err(error.to_string());
        }
        startup::open_initial_media(&self.app, media, self.commands, self.observer);
        self.display.focus(&self.app);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_core::{Phase, Playback, PlaylistEntry};

    #[test]
    fn reuse_waits_for_the_matching_stop_and_an_empty_session() {
        let mut playback = Playback::default();
        assert!(Availability::Unused.is_ready(0, &playback));
        let request = Availability::AfterStop(4);
        assert!(!request.is_ready(4, &playback));
        playback.generation = 2;
        playback.stopped_by = Some(3);
        assert!(!request.is_ready(4, &playback));
        playback.stopped_by = Some(4);
        assert!(!request.is_ready(5, &playback));
        playback.phase = Phase::Ended;
        assert!(!request.is_ready(4, &playback));
        playback.phase = Phase::Stopped;
        std::sync::Arc::make_mut(&mut playback.playlist).push(PlaylistEntry {
            id: 3,
            filename: "pending.mkv".into(),
            title: None,
            playing: false,
            current: false,
        });
        assert!(!request.is_ready(4, &playback));
        playback.playlist = Default::default();
        assert!(request.is_ready(4, &playback));
        assert!(!Availability::Unused.is_ready(0, &playback));
    }
}
