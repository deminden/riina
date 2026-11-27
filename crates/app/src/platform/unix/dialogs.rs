use crate::{
    NativeVideoHost, PlayerWindow, WelcomeWindow,
    ui_dialogs::{Purpose, SaveFile, SelectionMode, dispatch_selection},
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
    let picker = purpose.picker();
    start(
        app,
        welcome,
        move |owner| async move {
            #[cfg(target_os = "macos")]
            if matches!(picker.mode, SelectionMode::Media) {
                return crate::platform::choose_application_media(picker.title, Some(&owner)).await;
            }
            let mut dialog = rfd::AsyncFileDialog::new()
                .set_parent(owner.as_ref())
                .set_title(picker.title);
            let files = match picker.mode {
                SelectionMode::Track { kind, directory } => {
                    dialog = dialog
                        .add_filter(kind.filter_name(), &kind.extensions().collect::<Vec<_>>());
                    if let Some(directory) = directory {
                        dialog = dialog.set_directory(directory);
                    }
                    dialog.pick_file().await.into_iter().collect()
                }
                SelectionMode::Folders { multiple: true } => {
                    dialog.pick_folders().await.unwrap_or_default()
                }
                SelectionMode::Folders { multiple: false } => {
                    dialog.pick_folder().await.into_iter().collect()
                }
                SelectionMode::Media => dialog.pick_files().await.unwrap_or_default(),
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

pub(crate) fn save_file_dialog(
    app: &PlayerWindow,
    _host: Option<slint::Weak<NativeVideoHost>>,
    file: SaveFile,
    complete: impl FnOnce(&PlayerWindow, Result<Option<PathBuf>, String>) + 'static,
) {
    start(
        app,
        None,
        move |owner| async move {
            Ok(rfd::AsyncFileDialog::new()
                .set_parent(owner.as_ref())
                .set_title(file.title)
                .set_file_name(file.name)
                .add_filter(file.filter, &[file.extension])
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
