//! Translate the focused player's system media commands into existing actions.
use crate::{PlaybackStore, PlayerWindow, platform::RemoteMediaAction as Action};
use riina_core::Command;
use riina_mpv::CommandSender;
use riina_ui::{PlayerControlActions, PlayerMenuActions, PlayerSidebarActions};
use slint::ComponentHandle;

pub(super) fn apply(
    app: &PlayerWindow,
    action: Action,
    latest_playback: &PlaybackStore,
    player: &CommandSender,
) {
    match action {
        Action::Play => {
            if !crate::key_binding_preferences::dispatch_system_media_key(
                app,
                "PLAYONLY",
                latest_playback,
            ) {
                app.global::<PlayerControlActions>()
                    .invoke_set_paused(false);
            }
        }
        Action::Pause => {
            if !crate::key_binding_preferences::dispatch_system_media_key(
                app,
                "PAUSEONLY",
                latest_playback,
            ) {
                app.global::<PlayerControlActions>().invoke_set_paused(true);
            }
        }
        Action::Toggle => {
            if !crate::key_binding_preferences::dispatch_system_media_key(
                app,
                "PLAYPAUSE",
                latest_playback,
            ) {
                app.global::<PlayerMenuActions>().invoke_toggle_pause();
            }
        }
        Action::Stop => {
            if !crate::key_binding_preferences::dispatch_system_media_key(
                app,
                "STOP",
                latest_playback,
            ) {
                app.global::<PlayerMenuActions>().invoke_stop();
            }
        }
        Action::Next => {
            if !crate::key_binding_preferences::dispatch_system_media_key(
                app,
                "NEXT",
                latest_playback,
            ) {
                app.global::<PlayerMenuActions>().invoke_playlist_next();
            }
        }
        Action::Previous => {
            if !crate::key_binding_preferences::dispatch_system_media_key(
                app,
                "PREV",
                latest_playback,
            ) {
                app.global::<PlayerMenuActions>().invoke_playlist_previous();
            }
        }
        Action::Forward(seconds) => {
            if !crate::key_binding_preferences::dispatch_system_media_key(
                app,
                "FORWARD",
                latest_playback,
            ) && seconds.is_finite()
                && seconds > 0.0
            {
                app.global::<PlayerControlActions>()
                    .invoke_execute_mpv_input_action(format!("seek {seconds} exact").into());
            }
        }
        Action::Backward(seconds) => {
            if !crate::key_binding_preferences::dispatch_system_media_key(
                app,
                "REWIND",
                latest_playback,
            ) && seconds.is_finite()
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
