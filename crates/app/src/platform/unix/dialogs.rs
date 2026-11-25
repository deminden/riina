use crate::{
    NativeVideoHost, PlayerWindow, WelcomeWindow,
    ui_dialogs::{Purpose, dispatch_selection},
};
use riina_mpv::CommandSender;
use slint::{
    ComponentHandle,
    winit_030::{WinitWindowAccessor, winit::window::Window},
};
use std::{future::Future, path::PathBuf, sync::Arc};

pub fn open(
    app: &PlayerWindow,
    _host: Option<slint::Weak<NativeVideoHost>>,
    purpose: Purpose,
    sender: CommandSender,
    welcome: Option<slint::Weak<WelcomeWindow>>,
) {
    start(
        app,
        welcome,
        move |owner| async move {
            #[cfg(target_os = "macos")]
            if matches!(
                purpose,
                Purpose::Media | Purpose::MediaAlternative | Purpose::Playlist
            ) {
                return crate::platform::choose_application_media(purpose.title(), Some(&owner))
                    .await;
            }
            let dialog = rfd::AsyncFileDialog::new()
                .set_parent(owner.as_ref())
                .set_title(purpose.title());
            let files = if matches!(purpose, Purpose::PlaylistFolders) {
                dialog.pick_folders().await.unwrap_or_default()
            } else if purpose.allows_multiple() {
                dialog.pick_files().await.unwrap_or_default()
            } else {
                dialog.pick_file().await.into_iter().collect()
            };
            let paths = files
                .into_iter()
                .map(|file| {
                    file.path()
                        .to_str()
                        .map(str::to_owned)
                        .ok_or_else(|| "The path contains invalid Unicode".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok((!paths.is_empty()).then_some(paths))
        },
        move |app, result| {
            let result = result.and_then(|paths| {
                dispatch_selection(app, purpose, paths.unwrap_or_default(), sender)
            });
            if let Err(error) = result {
                app.set_status(error.into());
            }
        },
    );
}

pub(crate) fn save_playlist_dialog(
    app: &PlayerWindow,
    _host: Option<slint::Weak<NativeVideoHost>>,
    complete: impl FnOnce(&PlayerWindow, Result<Option<PathBuf>, String>) + 'static,
) {
    start(
        app,
        None,
        |owner| async move {
            Ok(rfd::AsyncFileDialog::new()
                .set_parent(owner.as_ref())
                .set_title("Save to Playlist")
                .set_file_name("playlist.m3u8")
                .add_filter("M3U8 playlist", &["m3u8"])
                .save_file()
                .await
                .map(|file| file.path().to_path_buf()))
        },
        complete,
    );
}

fn start<T: 'static, F: Future<Output = Result<Option<T>, String>> + 'static>(
    app: &PlayerWindow,
    welcome: Option<slint::Weak<WelcomeWindow>>,
    pick: impl FnOnce(Arc<Window>) -> F + 'static,
    complete: impl FnOnce(&PlayerWindow, Result<Option<T>, String>) + 'static,
) {
    if app
        .global::<riina_ui::PlayerDialogState>()
        .get_dialog_open()
    {
        return;
    }
    app.global::<riina_ui::PlayerDialogState>()
        .set_dialog_open(true);
    app.global::<riina_ui::PlayerControlActions>()
        .invoke_activity();
    let weak = app.as_weak();
    let failure = weak.clone();
    let started = slint::spawn_local(async move {
        let Some(app) = weak.upgrade() else { return };
        let result = async {
            // Retain the real parent throughout the asynchronous native sheet or
            // portal request. Close requests remain gated by dialog_open.
            let owner = app
                .window()
                .winit_window()
                .await
                .map_err(|e| e.to_string())?;
            pick(owner).await
        }
        .await;
        app.global::<riina_ui::PlayerDialogState>()
            .set_dialog_open(false);
        app.global::<riina_ui::PlayerControlActions>()
            .invoke_activity();
        if result.as_ref().is_ok_and(Option::is_none)
            && let Some(welcome) = welcome.as_ref().and_then(slint::Weak::upgrade)
        {
            // Continue releasing session resources if the window is closing.
            let _ = app.hide();
            if let Err(error) = crate::platform::show_window(&welcome) {
                crate::diagnostics::warn!("Could not restore the welcome window: {error}");
            }
            return;
        }
        complete(&app, result);
    });
    if let Err(error) = started
        && let Some(app) = failure.upgrade()
    {
        app.global::<riina_ui::PlayerDialogState>()
            .set_dialog_open(false);
        app.set_status(error.to_string().into());
    }
}
