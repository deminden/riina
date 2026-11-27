//! Hand new media to a player whose previous work has finished.
use super::{InitialMedia, UiSession, observer_state::ObserverId, startup};
use crate::{PlayerWindow, media_open, player::surface::PlayerDisplay};
use riina_mpv::CommandSender;
use slint::ComponentHandle;
use std::{cell::Cell, rc::Rc};

pub(crate) enum StopDisposition {
    Finished,
    OpenFailed {
        stream_attempt: Option<u64>,
        detail: String,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Availability {
    Unused,
    AfterStop(u32),
    Unavailable,
}

impl Availability {
    fn is_ready(self, request: u32, playback: &riina_core::Playback) -> bool {
        match self {
            Self::Unused => {
                request == 0
                    && playback.generation == 0
                    && playback.phase == riina_core::Phase::Stopped
                    && playback.playlist.is_empty()
            }
            Self::AfterStop(stop) => {
                request == stop && playback.playlist.is_empty() && playback.confirmed_stop(stop)
            }
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
    parked: Rc<Cell<bool>>,
}

impl UiSession {
    pub(crate) fn bind_stop(&self, callback: impl Fn(u32) + 'static) {
        self.observer_state.bind_stop(callback);
    }

    pub(crate) fn is_parked(&self) -> bool {
        self.observer_state.parked.get()
    }

    pub(crate) fn completed_stop(
        &self,
        app: &PlayerWindow,
        request: u32,
    ) -> Option<StopDisposition> {
        if !app.get_player_manager_ready()
            || app.get_media_request_generation().cast_unsigned() != request
            || app
                .global::<riina_ui::PlayerDialogState>()
                .get_dialog_open()
        {
            return None;
        }
        let playback = riina_core::sync::lock(&self.preference_state.latest_playback);
        if playback.generation == 0 || !playback.confirmed_stop(request) {
            return None;
        }
        self.observer_state
            .id()
            .stop_disposition(&playback, request)
    }

    pub(crate) fn park_after_stop(&self, app: &PlayerWindow, display: &PlayerDisplay) -> bool {
        if self.observer_state.id().idle_media().is_none()
            || app.get_fullscreen()
            || app.get_entering_fullscreen()
            || app.get_picture_in_picture_active()
            || app
                .global::<riina_ui::PlayerMusicModeState>()
                .get_music_mode()
        {
            // Fullscreen, PiP and compact presenters have independent window
            // transitions. Use their normal teardown before releasing the player.
            return false;
        }
        if let Err(error) = self.save(app) {
            crate::diagnostics::report(app, error.to_string());
        }
        self.observer_state.parked.set(true);
        app.set_player_manager_ready(false);
        app.invoke_dismiss_font_picker();
        app.global::<riina_ui::PlayerSidebarActions>()
            .invoke_dismiss_native_subtitle_color();
        if let Err(error) = display.hide_playback(app) {
            self.observer_state.parked.set(false);
            app.set_player_manager_ready(true);
            crate::diagnostics::report(app, error.to_string());
            return false;
        }
        true
    }

    pub(crate) fn idle_media_target(
        &self,
        app: &PlayerWindow,
        display: PlayerDisplay,
    ) -> Option<IdleMediaTarget> {
        let observer = self.observer_state.id();
        let available = observer.idle_media()?;
        let parked = &self.observer_state.parked;
        if (!parked.get() && !app.get_player_manager_ready())
            || app
                .global::<riina_ui::PlayerDialogState>()
                .get_dialog_open()
            || media_open::has_playlist(app)
            || !available.get().is_ready(
                app.get_media_request_generation().cast_unsigned(),
                &riina_core::sync::lock(&self.preference_state.latest_playback),
            )
            || (!parked.get() && display.switchable(app).is_none())
        {
            return None;
        }
        Some(IdleMediaTarget {
            app: app.clone_strong(),
            display,
            commands: self.player()?.command_sender(),
            observer,
            available,
            parked: parked.clone(),
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
        let parked = self.parked.get();
        let shown = if parked {
            self.display.show_playback(&self.app)
        } else {
            self.display.reveal(&self.app)
        };
        if let Err(error) = shown {
            if self.app.get_media_request_generation() == request {
                self.available.set(available);
            }
            return Err(error.to_string());
        }
        if parked {
            self.parked.set(false);
            self.app.set_player_manager_ready(true);
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
        assert!(!request.is_ready(4, &playback));
        playback.idle_active = true;
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
        playback.file_feedback = riina_core::FileFeedback::StoppedByEngine;
        assert!(playback.confirmed_stop(4));
        assert!(!request.is_ready(4, &playback));
        playback.playlist = Default::default();
        assert!(request.is_ready(4, &playback));
        assert!(!Availability::Unused.is_ready(0, &playback));
    }
}
