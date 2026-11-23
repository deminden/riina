use super::Assignments;
use crate::{CatalogRow, NativeVideoHost, PlaybackStore, PlayerWindow, diagnostics, file_actions};
use riina_core::{file_selection, sync::lock};
use riina_ui::{PlayerControlActions, PlayerDialogState};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};

pub(super) fn open(
    app: &PlayerWindow,
    row: CatalogRow,
    assignments: Assignments,
    playback: PlaybackStore,
    host: Option<slint::Weak<NativeVideoHost>>,
) {
    if app.global::<PlayerDialogState>().get_dialog_open() {
        return;
    }
    let Ok(id) = row.token.parse::<i64>() else {
        return;
    };
    let epoch = lock(&assignments.0).epoch;
    app.global::<PlayerDialogState>().set_dialog_open(true);
    let weak = app.as_weak();
    let started = slint::spawn_local(async move {
        let Some(app) = weak.upgrade() else { return };
        let selection = pick(&app, host, &row.path).await;
        app.global::<PlayerDialogState>().set_dialog_open(false);
        app.global::<PlayerControlActions>().invoke_activity();
        match selection {
            Ok(paths) => {
                // A native dialog may outlive the playlist entry that launched it.
                let snapshot = lock(&playback).clone();
                if snapshot
                    .playlist
                    .iter()
                    .any(|entry| entry.id == id && entry.filename == row.path.as_str())
                {
                    assignments.add(&row.path, epoch, snapshot.generation, paths);
                }
            }
            Err(error) => diagnostics::report(&app, error),
        }
    });
    if let Err(error) = started {
        app.global::<PlayerDialogState>().set_dialog_open(false);
        diagnostics::report(app, error.to_string());
    }
}

async fn pick(
    app: &PlayerWindow,
    host: Option<slint::Weak<NativeVideoHost>>,
    filename: &str,
) -> Result<Vec<String>, String> {
    let host = host.and_then(|host| host.upgrade());
    let window = host
        .as_ref()
        .map_or_else(|| app.window(), |host| host.window());
    let owner = window
        .winit_window()
        .await
        .map_err(|error| error.to_string())?;
    let mut dialog = rfd::AsyncFileDialog::new()
        .set_title("Add Subtitle")
        .set_parent(owner.as_ref())
        .add_filter("Subtitles", file_selection::SUPPORTED_SUBTITLE_EXTENSIONS);
    if let Some(path) = file_actions::local_media_path(filename)
        && let Some(directory) = path.parent().filter(|path| !path.as_os_str().is_empty())
    {
        dialog = dialog.set_directory(directory);
    }
    dialog
        .pick_files()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|selected| {
            selected
                .path()
                .to_str()
                .filter(|path| file_selection::is_supported_subtitle(path))
                .map(str::to_owned)
                .ok_or_else(|| "Choose a supported subtitle file".into())
        })
        .collect()
}
