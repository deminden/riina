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
    operation: Operation,
) {
    let controls = app.global::<PlayerControlActions>();
    let menu = app.global::<PlayerMenuActions>();
    if let Some(key) = transport_key(&operation)
        && crate::key_binding_preferences::dispatch_system_media_key(app, key, latest)
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
        Operation::Seek(offset) => seek(app, latest, offset),
        Operation::Position(position) => menu.invoke_seek((position as f64 / 1e6) as f32),
        Operation::OpenUri(uri) => open(app, sender, &uri),
        Operation::Loop(mode) => submit(app, sender, Command::SetLoopMode(mode)),
        Operation::Rate(rate) => submit(app, sender, Command::Speed(rate)),
        Operation::Shuffle(value) => {
            if lock(latest).playlist_shuffled != value {
                submit(app, sender, Command::TogglePlaylistShuffle);
            }
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

fn seek(app: &PlayerWindow, latest: &PlaybackStore, offset: i64) {
    let target = {
        let state = lock(latest);
        if !state.seekable {
            return;
        }
        state.position.unwrap_or(0.0) + offset as f64 / 1e6
    };
    let menu = app.global::<PlayerMenuActions>();
    if lock(latest)
        .duration
        .is_some_and(|duration| target > duration)
    {
        menu.invoke_playlist_next();
    } else {
        menu.invoke_seek(target.max(0.0) as f32);
    }
}

fn open(app: &PlayerWindow, sender: &CommandSender, reference: &str) {
    if let Ok(uri) = url::Url::parse(reference)
        && uri.scheme() == "file"
        && let Ok(path) = uri.to_file_path()
        && let Some(path) = path.to_str()
    {
        crate::media_open::selection(app, vec![path.to_owned()], false, sender.clone());
    } else {
        crate::media_open::url(app, reference, sender.clone());
    }
}

fn submit(app: &PlayerWindow, sender: &CommandSender, command: Command) {
    if let Err(error) = sender.send(command) {
        crate::diagnostics::report(app, error);
    }
}
