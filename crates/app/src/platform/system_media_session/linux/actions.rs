//! Dispatch Linux media-controller operations through the active player.
use super::interface::Operation;
use crate::{PlaybackStore, PlayerWindow};
use riina_core::{Command, sync::lock};
use riina_mpv::CommandSender;
use riina_ui::{PlayerControlActions, PlayerMenuActions, PlayerPreferences};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};

pub(super) fn apply(
    app: &PlayerWindow,
    latest: &PlaybackStore,
    sender: &CommandSender,
    generation: u64,
    operation: Operation,
) {
    let controls = app.global::<PlayerControlActions>();
    let menu = app.global::<PlayerMenuActions>();
    if let Some(key) = transport_key(&operation)
        && super::super::key_actions::dispatch_system_media_key(app, key)
    {
        return;
    }
    match operation {
        Operation::Raise => {
            let _ = crate::platform::show_window(app);
            app.window().with_winit_window(|window| {
                window.set_minimized(false);
                window.focus_window();
            });
        }
        Operation::Quit => menu.invoke_quit_riina(),
        Operation::Fullscreen(active) => menu.invoke_request_fullscreen(active),
        Operation::Play => controls.invoke_set_paused(false),
        Operation::Pause => controls.invoke_set_paused(true),
        Operation::PlayPause => menu.invoke_toggle_pause(),
        Operation::Stop => menu.invoke_stop(),
        Operation::Next => menu.invoke_playlist_next(),
        Operation::Previous => menu.invoke_playlist_previous(),
        Operation::Seek(offset) => seek(app, latest, sender, generation, offset),
        Operation::Position(position) => seek_to(app, sender, generation, position as f64 / 1e6),
        Operation::SelectTrack(id) => {
            submit_for_media(app, sender, generation, Command::SelectPlaylistId(id))
        }
        Operation::RemoveTrack(id) => {
            submit_for_media(app, sender, generation, Command::RemovePlaylistId(id))
        }
        Operation::AddTrack {
            path,
            after_id,
            make_current,
        } => submit_for_media(
            app,
            sender,
            generation,
            Command::InsertPlaylistTrack {
                path,
                after_id,
                make_current,
            },
        ),
        Operation::Loop(mode) => submit(app, sender, Command::SetLoopMode(mode)),
        Operation::Rate(rate) => submit(app, sender, Command::Speed(rate)),
        Operation::Shuffle(value) => {
            submit_for_media(app, sender, generation, Command::SetPlaylistShuffle(value))
        }
        Operation::Volume(value) => {
            let boost = app.global::<PlayerPreferences>().get_volume_boost();
            let volume = (value * 100.0).min(if boost { 200.0 } else { 100.0 });
            controls.invoke_set_volume(volume as f32);
            if volume > 0.0 && lock(latest).muted {
                submit(app, sender, Command::SetMuted(false));
            }
        }
    }
}

fn transport_key(operation: &Operation) -> Option<&'static str> {
    match operation {
        Operation::Play => Some("PLAYONLY"),
        Operation::Pause => Some("PAUSEONLY"),
        Operation::PlayPause => Some("PLAYPAUSE"),
        Operation::Stop => Some("STOP"),
        Operation::Next => Some("NEXT"),
        Operation::Previous => Some("PREV"),
        _ => None,
    }
}

fn seek(
    app: &PlayerWindow,
    latest: &PlaybackStore,
    sender: &CommandSender,
    generation: u64,
    offset: i64,
) {
    let (target, beyond_end) = {
        let state = lock(latest);
        if !state.seekable || state.generation != generation {
            return;
        }
        let target = state.position.unwrap_or(0.0) + offset as f64 / 1e6;
        (
            target,
            state.duration.is_some_and(|duration| target > duration),
        )
    };
    if beyond_end {
        submit_for_media(app, sender, generation, Command::PlaylistNext);
    } else {
        seek_to(app, sender, generation, target.max(0.0));
    }
}

fn seek_to(app: &PlayerWindow, sender: &CommandSender, generation: u64, seconds: f64) {
    app.global::<PlayerControlActions>().invoke_activity();
    submit_for_media(
        app,
        sender,
        generation,
        Command::Seek {
            seconds,
            absolute: true,
        },
    );
}

fn submit_for_media(app: &PlayerWindow, sender: &CommandSender, generation: u64, command: Command) {
    if let Err(error) = sender.send_for_media(generation, command) {
        crate::diagnostics::report(app, error);
    }
}

fn submit(app: &PlayerWindow, sender: &CommandSender, command: Command) {
    if let Err(error) = sender.send(command) {
        crate::diagnostics::report(app, error);
    }
}
