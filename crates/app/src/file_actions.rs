use crate::{PlayerWindow, key_binding_preferences::CurrentMediaLocation};
use riina_core::{Command, Playback};
use riina_mpv::CommandSender;
use slint::ComponentHandle;
use slint::winit_030::WinitWindowAccessor;
use std::sync::{Arc, Mutex};

pub(crate) fn downloaded_subtitle_path(playback: &Playback) -> Option<std::path::PathBuf> {
    let cache_root = dirs::cache_dir()?.join("riina").join("online-subtitles");
    let track = playback.tracks.iter().find(|track| {
        track.kind == riina_core::TrackKind::Sub && track.is_primary_selected() && track.external
    })?;
    let path = std::path::PathBuf::from(track.external_filename.as_deref()?);
    (path.starts_with(cache_root) && path.is_file()).then_some(path)
}

pub(crate) fn save_downloaded_subtitle(
    app: &PlayerWindow,
    host: Option<slint::Weak<crate::NativeVideoHost>>,
    latest_playback: &Arc<Mutex<Playback>>,
) {
    let path = latest_playback
        .lock()
        .ok()
        .and_then(|playback| downloaded_subtitle_path(&playback));
    let Some(path) = path else {
        app.set_status("Select a subtitle downloaded by Riina before saving it".into());
        return;
    };
    let contents = match std::fs::read(&path) {
        Ok(contents) => contents,
        Err(error) => {
            app.set_status(format!("Could not read downloaded subtitle: {error}").into());
            return;
        }
    };
    let file_name = path.file_name().map_or_else(
        || "subtitle.srt".into(),
        |name| name.to_string_lossy().into_owned(),
    );
    let weak_app = app.as_weak();
    app.set_dialog_open(true);
    app.invoke_activity();
    let failure = weak_app.clone();
    let started = slint::spawn_local(async move {
        let Some(app) = weak_app.upgrade() else {
            return;
        };
        let saved = async {
            let owner = if let Some(host) = host.and_then(|host| host.upgrade()) {
                host.window()
                    .winit_window()
                    .await
                    .map_err(|error| error.to_string())?
            } else {
                app.window()
                    .winit_window()
                    .await
                    .map_err(|error| error.to_string())?
            };
            let mut dialog = rfd::AsyncFileDialog::new()
                .set_title("Save Downloaded Subtitle")
                .set_file_name(&file_name)
                .set_parent(owner.as_ref());
            if let Some(extension) = path.extension().and_then(std::ffi::OsStr::to_str) {
                dialog = dialog.add_filter("Subtitle", &[extension]);
            }
            if let Some(file) = dialog.save_file().await {
                file.write(&contents)
                    .await
                    .map_err(|error| error.to_string())?;
                Ok::<bool, String>(true)
            } else {
                Ok(false)
            }
        }
        .await;
        app.set_dialog_open(false);
        app.invoke_activity();
        match saved {
            Ok(true) => app.set_status("Downloaded subtitle saved".into()),
            Ok(false) => {}
            Err(error) => {
                app.set_status(format!("Could not save downloaded subtitle: {error}").into())
            }
        }
    });
    if let Err(error) = started
        && let Some(app) = failure.upgrade()
    {
        app.set_dialog_open(false);
        app.set_status(error.to_string().into());
    }
}

/// Moves the current local item to the platform trash, then removes its stable
/// playlist identity only if that entry still exists in the observed session.
pub(crate) fn delete_current_media(
    app: &PlayerWindow,
    latest_playback: &Arc<Mutex<Playback>>,
    player: CommandSender,
) {
    let snapshot = latest_playback.lock().ok().map(|playback| playback.clone());
    let Some(snapshot) = snapshot else {
        app.set_status("Could not read the current media item".into());
        return;
    };
    let Some(entry) = snapshot
        .playlist
        .iter()
        .find(|entry| entry.playing)
        .or_else(|| snapshot.playlist.iter().find(|entry| entry.current))
    else {
        app.set_status("There is no current media file to delete".into());
        return;
    };
    let entry_id = entry.id;
    let filename = entry.filename.clone();
    let path = match crate::key_binding_preferences::current_media_location(&snapshot) {
        CurrentMediaLocation::Local(path) => path,
        CurrentMediaLocation::NoMedia => {
            app.set_status("There is no current media file to delete".into());
            return;
        }
        CurrentMediaLocation::NonLocal => {
            app.set_status(
                format!(
                    "Only local media files can be moved to the {}",
                    crate::platform::system_trash_name()
                )
                .into(),
            );
            return;
        }
    };
    if !path.is_file() {
        app.set_status("The current media file is not available to delete".into());
        return;
    }

    let weak_app = app.as_weak();
    let observed_playback = latest_playback.clone();
    let trash_name = crate::platform::system_trash_name();
    app.set_status(format!("Moving current media to the {trash_name}…").into());
    let started = std::thread::Builder::new()
        .name("riina-file-trash".into())
        .spawn(move || {
            let result = trash::delete(&path).map_err(|error| error.to_string());
            let _ = slint::invoke_from_event_loop(move || {
                let Some(app) = weak_app.upgrade() else {
                    return;
                };
                match result {
                    Ok(()) => {
                        let still_listed = observed_playback.lock().is_ok_and(|playback| {
                            playback
                                .playlist
                                .iter()
                                .any(|entry| entry.id == entry_id && entry.filename == filename)
                        });
                        if still_listed
                            && let Err(error) =
                                player.send(Command::RemovePlaylistId(entry_id))
                        {
                            app.set_status(format!(
                                "File moved to the {trash_name}, but its playlist item could not be removed: {error}"
                            ).into());
                            return;
                        }
                        app.set_status(format!("Moved current media to the {trash_name}").into());
                    }
                    Err(error) => app.set_status(
                        format!("Could not move the current media to the {trash_name}: {error}")
                            .into(),
                    ),
                }
            });
        });
    if let Err(error) = started {
        app.set_status(format!("Could not start the file Trash operation: {error}").into());
    }
}
