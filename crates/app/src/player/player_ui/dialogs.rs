use super::{PlayerContext, bind};
use crate::{
    NativeVideoHost, PlayerWindow,
    ui_dialogs::{self, Purpose},
};
use riina_core::{Command, Phase, Playback, osc_preferences::TimePrecision, sync::lock};
use riina_mpv::CommandSender;
use riina_ui::{PlayerDialogState, PlayerMenuActions};
use slint::ComponentHandle;
use std::{cell::Cell, rc::Rc};

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

pub(super) fn install_jump_time(app: &PlayerWindow, context: &PlayerContext) {
    let target = Rc::new(Cell::new(0));
    let opening = target.clone();
    bind!(
        context,
        app.global::<PlayerMenuActions>(),
        on_open_jump_time_dialog,
        |ctx| {
            let Some(app) = ctx.app() else { return };
            let dialog = app.global::<PlayerDialogState>();
            if dialog.get_dialog_open() || dialog.get_jump_time_dialog_visible() {
                return;
            }
            let state = lock(ctx.playback()).clone();
            if jump_is_available(&state) {
                opening.set(state.generation);
                dialog.set_jump_time_draft(jump_draft(&state).into());
                dialog.set_jump_time_dialog_visible(true);
            }
        }
    );
    bind!(
        context,
        app.global::<PlayerDialogState>(),
        on_jump_to_time,
        |ctx, input| {
            let Some(app) = ctx.app() else { return };
            let dialog = app.global::<PlayerDialogState>();
            if !dialog.get_jump_time_dialog_visible() {
                return;
            }
            let generation = target.get();
            let state = lock(ctx.playback()).clone();
            if state.generation != generation || !jump_is_available(&state) {
                dialog.invoke_close_jump_time();
                return;
            }
            let Some(seconds) = riina_core::time_input::parse_video_time(input.as_str()) else {
                dialog.set_jump_time_invalid(true);
                dialog.set_jump_time_error("".into());
                return;
            };
            ctx.activity();
            match ctx.commands().send_for_media(
                generation,
                Command::Seek {
                    seconds,
                    absolute: true,
                },
            ) {
                Ok(()) => dialog.invoke_close_jump_time(),
                Err(error) => {
                    dialog.set_jump_time_invalid(false);
                    dialog.set_jump_time_error(error.to_string().into());
                }
            }
        }
    );
}

fn jump_is_available(state: &Playback) -> bool {
    state.phase.has_loaded_file() && !state.idle_active && !state.engine_shutdown
}

fn jump_draft(state: &Playback) -> String {
    let duration = state
        .duration
        .filter(|time| time.is_finite() && *time >= 0.0);
    let position = if state.phase == Phase::Ended {
        duration.or(state.position)
    } else {
        state.position
    };
    let Some(position) = position.filter(|time| time.is_finite()) else {
        return String::new();
    };
    let position = position.clamp(0.0, duration.unwrap_or(f64::MAX));
    TimePrecision::Milliseconds.elapsed_label(Some(position))
}

pub(super) fn install_playlist_url(app: &PlayerWindow, command_sender: &CommandSender) {
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
                    crate::player::playlist_edits::submit(
                        &app,
                        &commands,
                        Command::AppendFiles(vec![url.to_owned()]),
                    )
                });
            if let Err(error) = result {
                app.global::<PlayerDialogState>()
                    .set_url_dialog_error(error.into());
                return false;
            }
            true
        });
}
