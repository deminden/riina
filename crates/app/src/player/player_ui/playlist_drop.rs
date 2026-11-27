//! Resolve the visible insertion gap before directory discovery and command dispatch.
use crate::{
    PlaybackStore, PlayerWindow, media_open, platform,
    player::playlist_edits::{self, Insertion},
};
use riina_core::sync::lock;
use riina_mpv::CommandSender;
use riina_ui::{PlayerCatalogState, PlayerSidebarState};
use slint::{ComponentHandle, Model};
use std::ffi::OsString;

pub(super) enum Target {
    Player(Option<u64>),
    Playlist {
        before_id: Option<i64>,
        generation: i32,
    },
}

pub(super) fn register_desktop(
    app: &PlayerWindow,
    sender: &CommandSender,
    playback: PlaybackStore,
    hover: std::rc::Rc<super::playlist_hover::PlaylistHover>,
) -> platform::desktop_drop::Registration {
    let weak = app.as_weak();
    let sender = sender.clone();
    platform::desktop_drop::register(
        app,
        move |event| hover.on_desktop(event),
        move |position| {
            let app = weak.upgrade()?;
            let target = target_at(&app, position, &playback);
            let generation = app.get_media_request_generation();
            let weak = weak.clone();
            let sender = sender.clone();
            Some(Box::new(move |paths| {
                if let Some(app) = weak.upgrade()
                    && app.window().is_visible()
                    && app.get_media_request_generation() == generation
                {
                    handle(&app, &sender, target, paths);
                }
            }))
        },
    )
}

pub(super) fn target(
    app: &PlayerWindow,
    window: &slint::Window,
    playback: &PlaybackStore,
) -> Result<Target, String> {
    let Some(position) = platform::file_drop_position(window) else {
        return Ok(player_target(playback));
    };
    target_at(app, position, playback)
}

fn target_at(
    app: &PlayerWindow,
    position: slint::LogicalPosition,
    playback: &PlaybackStore,
) -> Result<Target, String> {
    let sidebar = app.global::<PlayerSidebarState>();
    let Ok(index) = usize::try_from(sidebar.invoke_playlist_drop_slot(position.x, position.y))
    else {
        return Ok(player_target(playback));
    };
    let rows = app.global::<PlayerCatalogState>().get_playlist();
    let before_id = if index == rows.row_count() {
        None
    } else {
        Some(
            rows.row_data(index)
                .and_then(|row| row.token.parse().ok())
                .ok_or("The playlist drop position is no longer available")?,
        )
    };
    Ok(Target::Playlist {
        before_id,
        generation: app.get_media_request_generation(),
    })
}

fn player_target(playback: &PlaybackStore) -> Target {
    let snapshot = lock(playback);
    let generation = (snapshot.generation != 0 && snapshot.phase.has_loaded_file())
        .then_some(snapshot.generation);
    Target::Player(generation)
}

pub(super) fn handle(
    app: &PlayerWindow,
    sender: &CommandSender,
    target: Result<Target, String>,
    files: Vec<OsString>,
) {
    let (before_id, generation) = match target {
        Ok(Target::Player(media)) => {
            media_open::dropped(app, files, media, sender.clone());
            return;
        }
        Ok(Target::Playlist {
            before_id,
            generation,
        }) => (before_id, generation),
        Err(error) => {
            app.set_status(error.into());
            return;
        }
    };
    if app.get_media_request_generation() != generation {
        return;
    }
    let result = playlist_edits::prepare(app, sender, Insertion::Before(before_id), move || {
        files
            .into_iter()
            .map(OsString::into_string)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "A media path contains invalid Unicode".to_owned())
    });
    if let Err(error) = result {
        app.set_status(error.into());
    }
}

pub(super) fn paste(
    app: &PlayerWindow,
    sender: &CommandSender,
    before_id: Option<i64>,
    paths: platform::clipboard::PlaylistPaste,
) -> Result<(), String> {
    playlist_edits::prepare(app, sender, Insertion::Before(before_id), move || {
        Ok(paths.into_paths())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_core::{Phase, Playback};
    use std::sync::Arc;

    #[test]
    fn a_drop_captures_the_live_loaded_generation_before_its_payload_arrives() {
        let playback = PlaybackStore::default();
        assert!(matches!(player_target(&playback), Target::Player(None)));
        *lock(&playback) = Arc::new(Playback {
            phase: Phase::Paused,
            generation: 42,
            ..Playback::default()
        });
        let accepted = player_target(&playback);
        *lock(&playback) = Arc::new(Playback {
            phase: Phase::Loading,
            generation: 43,
            ..Playback::default()
        });
        assert!(matches!(accepted, Target::Player(Some(42))));
        assert!(matches!(player_target(&playback), Target::Player(None)));
    }
}
