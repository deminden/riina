use crate::{NativeVideoHost, PlayerWindow, WelcomeWindow, media_open, ui_dialogs::Purpose};
use riina_mpv::CommandSender;
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};

pub fn open(
    app: &PlayerWindow,
    _host: Option<slint::Weak<NativeVideoHost>>,
    purpose: Purpose,
    sender: CommandSender,
    welcome: Option<slint::Weak<WelcomeWindow>>,
) {
    if app.get_dialog_open() {
        return;
    }
    app.set_dialog_open(true);
    app.invoke_activity();
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
            let dialog = rfd::AsyncFileDialog::new()
                .set_title(purpose.title())
                .set_parent(owner.as_ref());
            let files = if matches!(purpose, Purpose::Media | Purpose::Playlist) {
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
            Ok::<_, String>(paths)
        }
        .await;
        app.set_dialog_open(false);
        app.invoke_activity();
        if result.as_ref().is_ok_and(Vec::is_empty)
            && let Some(welcome) = welcome.as_ref().and_then(slint::Weak::upgrade)
        {
            let _ = app.hide();
            let _ = welcome.show();
            return;
        }
        let result = result.and_then(|paths| {
            if matches!(purpose, Purpose::Media | Purpose::Playlist) {
                media_open::selection(&app, paths, matches!(purpose, Purpose::Playlist), sender);
                Ok(())
            } else {
                purpose
                    .command(paths)
                    .map_or(Ok(()), |command| sender.send(command))
            }
        });
        if let Err(error) = result {
            app.set_status(error.into());
        }
    });
    if let Err(error) = started
        && let Some(app) = failure.upgrade()
    {
        app.set_dialog_open(false);
        app.set_status(error.to_string().into());
    }
}
