use crate::PlayerWindow;
use riina_core::{Playback, sync::lock};
use slint::ComponentHandle;
use slint::winit_030::WinitWindowAccessor;

mod current;
mod playlist;
pub(crate) use current::{CurrentMedia, current_media, install};
pub(crate) use playlist::{
    PlaylistFile, local_media_path, reveal_playlist_files, trash_playlist_files,
};

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
    latest_playback: &crate::PlaybackStore,
) {
    let path = downloaded_subtitle_path(&lock(latest_playback));
    let Some(path) = path else {
        app.set_status("Select a subtitle downloaded by Riina before saving it".into());
        return;
    };
    let contents = match std::fs::read(&path) {
        Ok(contents) => contents,
        Err(error) => {
            crate::diagnostics::report(app, format!("Could not read downloaded subtitle: {error}"));
            return;
        }
    };
    let file_name = path.file_name().map_or_else(
        || "subtitle.srt".into(),
        |name| name.to_string_lossy().into_owned(),
    );
    let weak_app = app.as_weak();
    app.global::<riina_ui::PlayerDialogState>()
        .set_dialog_open(true);
    app.global::<riina_ui::PlayerControlActions>()
        .invoke_activity();
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
        app.global::<riina_ui::PlayerDialogState>()
            .set_dialog_open(false);
        app.global::<riina_ui::PlayerControlActions>()
            .invoke_activity();
        match saved {
            Ok(true) => app.set_status("Downloaded subtitle saved".into()),
            Ok(false) => {}
            Err(error) => crate::diagnostics::report(
                &app,
                format!("Could not save downloaded subtitle: {error}"),
            ),
        }
    });
    if let Err(error) = started
        && let Some(app) = failure.upgrade()
    {
        app.global::<riina_ui::PlayerDialogState>()
            .set_dialog_open(false);
        app.set_status(error.to_string().into());
    }
}
