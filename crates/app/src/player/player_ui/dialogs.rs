use crate::{
    NativeVideoHost, PlayerWindow,
    ui_dialogs::{self, Purpose},
};
use riina_core::Command;
use riina_mpv::CommandSender;
use riina_ui::{PlayerDialogState, PlayerMenuActions};
use slint::ComponentHandle;

pub(super) fn install_browse(
    app: &PlayerWindow,
    command_sender: &CommandSender,
    host: Option<slint::Weak<NativeVideoHost>>,
) {
    let browse_player = command_sender.clone();
    let weak = app.as_weak();
    let browse_host = host;
    app.global::<PlayerMenuActions>()
        .on_browse_files(move |category| {
            let purpose = match category {
                1 => Purpose::Audio,
                2 => Purpose::Subtitle,
                4 => Purpose::Playlist,
                5 => Purpose::PlaylistFolders,
                _ => return,
            };
            if let Some(app) = weak.upgrade() {
                ui_dialogs::open(
                    &app,
                    browse_host.clone(),
                    purpose,
                    browse_player.clone(),
                    None,
                );
            }
        });
}

pub(super) fn install_time_and_url(app: &PlayerWindow, command_sender: &CommandSender) {
    install_url_dialogs(app, command_sender);
    let weak = app.as_weak();
    app.global::<PlayerMenuActions>()
        .on_open_jump_time_dialog(move || {
            if let Some(app) = weak.upgrade() {
                app.global::<riina_ui::PlayerDialogState>()
                    .set_jump_time_draft(
                        app.global::<riina_ui::PlaybackTimelineState>()
                            .get_position_label(),
                    );
                app.global::<riina_ui::PlayerDialogState>()
                    .set_jump_time_error("".into());
                app.global::<riina_ui::PlayerDialogState>()
                    .set_jump_time_dialog_visible(true);
            }
        });
    let jump_player = command_sender.clone();
    let weak = app.as_weak();
    app.global::<riina_ui::PlayerDialogState>()
        .on_jump_to_time(move |input| {
            let Some(app) = weak.upgrade() else { return };
            let Some(seconds) = riina_core::time_input::parse_video_time(input.as_str()) else {
                app.global::<riina_ui::PlayerDialogState>()
                    .set_jump_time_error("Enter a valid time such as 20:35 or 1:02:03.500.".into());
                return;
            };
            app.global::<riina_ui::PlayerControlActions>()
                .invoke_activity();
            match jump_player.send(Command::Seek {
                seconds,
                absolute: true,
            }) {
                Ok(()) => {
                    app.global::<riina_ui::PlayerDialogState>()
                        .set_jump_time_error("".into());
                    app.global::<riina_ui::PlayerDialogState>()
                        .set_jump_time_dialog_visible(false);
                }
                Err(error) => app
                    .global::<riina_ui::PlayerDialogState>()
                    .set_jump_time_error(error.to_string().into()),
            }
        });
}

fn install_url_dialogs(app: &PlayerWindow, command_sender: &CommandSender) {
    let weak = app.as_weak();
    app.global::<PlayerMenuActions>()
        .on_open_playlist_url_dialog(move || {
            if let Some(app) = weak.upgrade() {
                let dialog = app.global::<PlayerDialogState>();
                dialog.set_url_dialog_draft("".into());
                dialog.set_url_dialog_error("".into());
                dialog.set_url_dialog_visible(true);
            }
        });
    let weak = app.as_weak();
    let commands = command_sender.clone();
    app.global::<PlayerDialogState>()
        .on_append_playlist_url(move |input| {
            let Some(app) = weak.upgrade() else {
                return false;
            };
            let result = crate::player::media_open::validate_url(input.as_str())
                .map_err(|error| error.to_string())
                .and_then(|url| {
                    commands
                        .send_playlist_edit(Command::AppendFiles(vec![url.to_owned()]))
                        .map_err(String::from)
                });
            if let Err(error) = result {
                app.global::<PlayerDialogState>()
                    .set_url_dialog_error(error.into());
                return false;
            }
            true
        });
}
