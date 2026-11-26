use super::playlist::{FileRemoval, remove_playlist_files};
use super::{PlaylistFile, local_media_path, reveal_playlist_files};
use crate::{PlaybackStore, PlayerWindow, diagnostics};
use riina_core::{Phase, Playback, PlaylistEntry, sync::lock};
use riina_mpv::CommandSender;
use riina_ui::{PlayerDialogState, PlayerMenuActions};
use slint::ComponentHandle;
use std::path::PathBuf;

pub(crate) enum CurrentMedia<'a> {
    None,
    NonLocal,
    Local {
        entry: &'a PlaylistEntry,
        path: PathBuf,
    },
}

pub(crate) fn current_media(playback: &Playback) -> CurrentMedia<'_> {
    let Some(entry) = playback
        .playlist
        .iter()
        .find(|entry| entry.playing)
        .or_else(|| playback.playlist.iter().find(|entry| entry.current))
    else {
        return CurrentMedia::None;
    };
    local_media_path(&entry.filename).map_or(CurrentMedia::NonLocal, |path| CurrentMedia::Local {
        entry,
        path,
    })
}

pub(crate) fn install(app: &PlayerWindow, commands: &CommandSender, playback: &PlaybackStore) {
    let menu = app.global::<PlayerMenuActions>();
    let weak = app.as_weak();
    let reveal_playback = playback.clone();
    menu.on_reveal_current_media(move || {
        let Some(app) = weak.upgrade() else { return };
        let file = {
            let playback = lock(&reveal_playback);
            match current_media(&playback) {
                CurrentMedia::Local { entry, path } => {
                    PlaylistFile::new(entry.id, entry.filename.clone(), path)
                }
                CurrentMedia::NonLocal => {
                    return app.set_status("Only local media files can be revealed".into());
                }
                CurrentMedia::None => {
                    return app.set_status("There is no current media file to reveal".into());
                }
            }
        };
        reveal_playlist_files(&app, vec![file]);
    });

    let weak = app.as_weak();
    let playback = playback.clone();
    let commands = commands.clone();
    let remove = move |removal| {
        if let Some(app) = weak.upgrade() {
            remove_current(&app, &playback, commands.clone(), removal);
        }
    };
    let trash = remove.clone();
    menu.on_delete_current_media(move || trash(FileRemoval::Trash));
    menu.on_delete_current_media_permanently(move || remove(FileRemoval::Permanent));
}

fn remove_current(
    app: &PlayerWindow,
    playback: &PlaybackStore,
    commands: CommandSender,
    removal: FileRemoval,
) {
    if app.global::<PlayerDialogState>().get_dialog_open() {
        return;
    }
    let file = match removal_target(playback) {
        Ok(file) => file,
        Err(error) => return diagnostics::report(app, error.to_owned()),
    };
    remove_playlist_files(app, vec![file], commands, removal);
}

fn removal_target(playback: &PlaybackStore) -> Result<PlaylistFile, &'static str> {
    let snapshot = playback
        .lock()
        .map_err(|_| "Could not read the current media item")?;
    if snapshot.phase == Phase::Loading {
        return Err("Wait for the current media to load before deleting it");
    }
    match current_media(&snapshot) {
        CurrentMedia::Local { entry, path } => {
            Ok(PlaylistFile::new(entry.id, entry.filename.clone(), path))
        }
        CurrentMedia::NonLocal => Err("Only local media files can be deleted"),
        CurrentMedia::None => Err("There is no current media file to delete"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn removal_captures_the_playing_identity_and_rejects_loading_or_network_media() {
        let entry = |id, filename: &str, current, playing| PlaylistEntry {
            id,
            filename: filename.into(),
            title: None,
            current,
            playing,
        };
        let mut playback = Playback {
            phase: Phase::Paused,
            playlist: Arc::new(vec![
                entry(12, "next.mkv", true, false),
                entry(13, "playing.mkv", false, true),
            ]),
            ..Playback::default()
        };
        let CurrentMedia::Local {
            entry: selected,
            path,
        } = current_media(&playback)
        else {
            panic!("The playing entry is local");
        };
        assert_eq!(selected.id, 13);
        assert_eq!(path, PathBuf::from("playing.mkv"));
        assert!(removal_target(&Arc::new(Mutex::new(Arc::new(playback.clone())))).is_ok());

        playback.phase = Phase::Loading;
        assert!(removal_target(&Arc::new(Mutex::new(Arc::new(playback.clone())))).is_err());
        playback.phase = Phase::Paused;
        playback.playlist = Arc::new(vec![entry(14, "https://example.test/test.mkv", true, true)]);
        assert!(removal_target(&Arc::new(Mutex::new(Arc::new(playback)))).is_err());
    }
}
