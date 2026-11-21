use crate::{
    NativeVideoHost, PlayerWindow,
    ui_dialogs::{self, Purpose},
};
use riina_core::Command;
use riina_mpv::CommandSender;
use slint::ComponentHandle;

pub(super) fn install_browse(
    app: &PlayerWindow,
    command_sender: &CommandSender,
    host: Option<slint::Weak<NativeVideoHost>>,
) {
    let browse_player = command_sender.clone();
    let weak = app.as_weak();
    let browse_host = host;
    app.on_browse_files(move |category| {
        let purpose = match category {
            1 => Purpose::Audio,
            2 => Purpose::Subtitle,
            4 => Purpose::Playlist,
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
    let weak = app.as_weak();
    app.on_open_jump_time_dialog(move || {
        if let Some(app) = weak.upgrade() {
            app.set_jump_time_draft(app.get_position_label());
            app.set_jump_time_error("".into());
            app.set_jump_time_dialog_visible(true);
        }
    });
    let weak = app.as_weak();
    app.on_open_url_dialog(move || {
        if let Some(app) = weak.upgrade() {
            app.set_url_dialog_draft("".into());
            app.set_url_dialog_visible(true);
        }
    });
    let jump_player = command_sender.clone();
    let weak = app.as_weak();
    app.on_jump_to_time(move |input| {
        let Some(app) = weak.upgrade() else { return };
        let Some(seconds) = riina_core::time_input::parse_video_time(input.as_str()) else {
            app.set_jump_time_error("Enter a valid time such as 20:35 or 1:02:03.500.".into());
            return;
        };
        app.invoke_activity();
        match jump_player.send(Command::Seek {
            seconds,
            absolute: true,
        }) {
            Ok(()) => {
                app.set_jump_time_error("".into());
                app.set_jump_time_dialog_visible(false);
            }
            Err(error) => app.set_jump_time_error(error.into()),
        }
    });
}
