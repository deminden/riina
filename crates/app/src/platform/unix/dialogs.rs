use crate::{
    NativeVideoHost, PlayerWindow, WelcomeWindow,
    ui_dialogs::{Purpose, dispatch_selection},
};
use riina_mpv::CommandSender;
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{future::Future, path::PathBuf};

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
        move |dialog| async move {
            let dialog = dialog.set_title(purpose.title());
            let files = if purpose.allows_multiple() {
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
        |dialog| async {
            Ok(dialog
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
    pick: impl FnOnce(rfd::AsyncFileDialog) -> F + 'static,
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
            pick(rfd::AsyncFileDialog::new().set_parent(owner.as_ref())).await
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
