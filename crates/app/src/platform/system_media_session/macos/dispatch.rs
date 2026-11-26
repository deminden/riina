//! Translate the focused player's system media commands into existing actions.
use crate::{PlayerWindow, platform::RemoteMediaAction as Action};
use riina_core::Command;
use riina_mpv::CommandSender;
use riina_ui::{PlayerControlActions, PlayerMenuActions, PlayerSidebarActions};
use slint::ComponentHandle;

pub(super) fn apply(app: &PlayerWindow, action: Action, player: &CommandSender) {
    match action {
        Action::Play => {
            if !super::super::key_actions::dispatch_system_media_key(app, "PLAYONLY") {
                app.global::<PlayerControlActions>()
                    .invoke_set_paused(false);
            }
        }
        Action::Pause => {
            if !super::super::key_actions::dispatch_system_media_key(app, "PAUSEONLY") {
                app.global::<PlayerControlActions>().invoke_set_paused(true);
            }
        }
        Action::Toggle => {
            if !super::super::key_actions::dispatch_system_media_key(app, "PLAYPAUSE") {
                app.global::<PlayerMenuActions>().invoke_toggle_pause();
            }
        }
        Action::Stop => {
            if !super::super::key_actions::dispatch_system_media_key(app, "STOP") {
                app.global::<PlayerMenuActions>().invoke_stop();
            }
        }
        Action::Next => {
            if !super::super::key_actions::dispatch_system_media_key(app, "NEXT") {
                app.global::<PlayerMenuActions>().invoke_playlist_next();
            }
        }
        Action::Previous => {
            if !super::super::key_actions::dispatch_system_media_key(app, "PREV") {
                app.global::<PlayerMenuActions>().invoke_playlist_previous();
            }
        }
        Action::Forward(seconds) => {
            if !super::super::key_actions::dispatch_system_media_key(app, "FORWARD")
                && seconds.is_finite()
                && seconds > 0.0
            {
                app.global::<PlayerControlActions>()
                    .invoke_execute_mpv_input_action(format!("seek {seconds} exact").into());
            }
        }
        Action::Backward(seconds) => {
            if !super::super::key_actions::dispatch_system_media_key(app, "REWIND")
                && seconds.is_finite()
                && seconds > 0.0
            {
                app.global::<PlayerControlActions>()
                    .invoke_execute_mpv_input_action(format!("seek -{seconds} exact").into());
            }
        }
        Action::Position(seconds) if seconds.is_finite() && seconds >= 0.0 => {
            app.global::<PlayerMenuActions>()
                .invoke_seek(seconds as f32);
        }
        Action::Position(_) => {}
        Action::Rate(rate) if rate.is_finite() && rate > 0.0 => {
            let _ = app
                .global::<PlayerSidebarActions>()
                .invoke_set_speed_text(rate.to_string().into());
        }
        Action::Rate(_) => {}
        Action::Repeat => {
            if let Err(error) = player.send(Command::CycleLoop) {
                crate::diagnostics::report(app, error);
            }
        }
    }
}
