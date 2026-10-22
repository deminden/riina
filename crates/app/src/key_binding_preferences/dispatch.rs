//! Install and execute typed profile actions without retaining playback owners.
use super::{
    bindings::Bindings, builtins::mappings_for, keys::recorded_key, menus::menu_shortcut_rows,
};
use crate::PlayerWindow;
use riina_core::key_bindings::{ActionKind, Mapping, Settings};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::sync::Arc;

pub(crate) fn install_player(
    app: &PlayerWindow,
    settings: &Settings,
    latest_playback: Arc<std::sync::Mutex<riina_core::Playback>>,
) {
    app.set_key_binding_current_profile(settings.current_profile.clone().into());
    app.set_key_binding_use_system_media_controls(settings.use_system_media_controls);
    app.set_key_binding_display_raw_values(settings.display_raw_values);
    let mappings = match mappings_for(&settings.current_profile) {
        Ok(mappings) => mappings,
        Err(error) => {
            app.set_status(format!("Could not load input profile: {error}").into());
            Vec::new()
        }
    };
    let mappings = Bindings::new(mappings);
    app.set_menu_shortcuts(ModelRc::new(VecModel::from(menu_shortcut_rows(&mappings))));
    let player = app.as_weak();
    let observed_playback = latest_playback;
    app.on_dispatch_key_binding(move |text, control, alt, shift, meta| {
        let Some(key) = recorded_key(text.as_str(), control, alt, shift, meta) else {
            return false;
        };
        let Some(mapping) = mappings.get(&key) else {
            return false;
        };
        let Some(app) = player.upgrade() else {
            return true;
        };
        if is_system_media_key(&key) && !app.get_key_binding_use_system_media_controls() {
            return false;
        }
        dispatch_mapping(&app, mapping, &observed_playback);
        true
    });
}

pub(super) enum RuntimeAction {
    TogglePause,
    SetPaused(bool),
    Quit,
    Stop,
    ShowProgress,
    Seek(f32),
    Volume(f32),
    Mute,
    SubtitleVisibility,
    Fullscreen(Option<bool>),
    CycleAudio,
    CycleSubtitle,
    CycleVideo,
    FrameStep,
    FrameBackStep,
    SubtitlePosition(f32),
    OpenPreferences,
    SettingsPane(i32),
    PlaylistNext,
    PlaylistPrevious,
    PlaylistShuffle,
    TogglePlaylistPane(i32),
    ToggleMusicMode,
    TogglePictureInPicture,
    FitToScreen,
    ResizeWindow(i32),
    TogglePluginsPanel,
    FindOnlineSubs,
    RevealCurrentMedia,
    ToggleLiveText,
    AbLoop,
    Screenshot,
    MpvInputAction(String),
    Ignore,
}

pub(super) fn is_system_media_key(key: &str) -> bool {
    matches!(
        key,
        "PLAY"
            | "PAUSE"
            | "PLAYPAUSE"
            | "PLAYONLY"
            | "PAUSEONLY"
            | "STOP"
            | "FORWARD"
            | "REWIND"
            | "NEXT"
            | "PREV"
            | "VOLUME_UP"
            | "VOLUME_DOWN"
            | "MUTE"
            | "POWER"
            | "CLOSE_WIN"
    )
}

pub(super) fn runtime_action(mapping: &Mapping) -> Option<RuntimeAction> {
    let action = mapping.action.trim();
    if mapping.kind == ActionKind::Riina {
        return match action {
            "video-panel" => Some(RuntimeAction::SettingsPane(6)),
            "audio-panel" => Some(RuntimeAction::SettingsPane(1)),
            "sub-panel" => Some(RuntimeAction::SettingsPane(2)),
            "preferences" => Some(RuntimeAction::OpenPreferences),
            "playlist-panel" => Some(RuntimeAction::TogglePlaylistPane(4)),
            "chapter-panel" => Some(RuntimeAction::TogglePlaylistPane(3)),
            "fit-to-screen" => Some(RuntimeAction::FitToScreen),
            "bigger-window" => Some(RuntimeAction::ResizeWindow(4)),
            "smaller-window" => Some(RuntimeAction::ResizeWindow(5)),
            "toggle-pip" => Some(RuntimeAction::TogglePictureInPicture),
            "toggle-music-mode" => Some(RuntimeAction::ToggleMusicMode),
            "find-online-subs" => Some(RuntimeAction::FindOnlineSubs),
            "show-current-file-in-finder" => Some(RuntimeAction::RevealCurrentMedia),
            "live-text" => Some(RuntimeAction::ToggleLiveText),
            "plugin-panel" => Some(RuntimeAction::TogglePluginsPanel),
            _ => None,
        };
    }
    let words = action.split_whitespace().collect::<Vec<_>>();
    match words.as_slice() {
        ["ignore"] => Some(RuntimeAction::Ignore),
        ["cycle", "pause"] => Some(RuntimeAction::TogglePause),
        ["set", "pause", "yes"] => Some(RuntimeAction::SetPaused(true)),
        ["set", "pause", "no"] => Some(RuntimeAction::SetPaused(false)),
        ["quit"] => Some(RuntimeAction::Quit),
        ["stop"] => Some(RuntimeAction::Stop),
        ["show-progress"] => Some(RuntimeAction::ShowProgress),
        ["cycle", "mute"] => Some(RuntimeAction::Mute),
        ["cycle", "sub-visibility"] => Some(RuntimeAction::SubtitleVisibility),
        ["cycle", "fullscreen"] => Some(RuntimeAction::Fullscreen(None)),
        ["set", "fullscreen", "yes"] => Some(RuntimeAction::Fullscreen(Some(true))),
        ["set", "fullscreen", "no"] => Some(RuntimeAction::Fullscreen(Some(false))),
        ["cycle", "audio"] => Some(RuntimeAction::CycleAudio),
        ["cycle", "sub"] => Some(RuntimeAction::CycleSubtitle),
        ["cycle", "video"] => Some(RuntimeAction::CycleVideo),
        ["frame-step"] => Some(RuntimeAction::FrameStep),
        ["frame-back-step"] => Some(RuntimeAction::FrameBackStep),
        ["playlist-next"] => Some(RuntimeAction::PlaylistNext),
        ["playlist-prev"] => Some(RuntimeAction::PlaylistPrevious),
        ["playlist-shuffle"] => Some(RuntimeAction::PlaylistShuffle),
        ["ab-loop"] => Some(RuntimeAction::AbLoop),
        ["screenshot"] => Some(RuntimeAction::Screenshot),
        ["set", "window-scale", scale] => match *scale {
            "0.5" => Some(RuntimeAction::ResizeWindow(0)),
            "1" | "1.0" => Some(RuntimeAction::ResizeWindow(1)),
            "2" | "2.0" => Some(RuntimeAction::ResizeWindow(2)),
            _ => None,
        },
        ["seek", seconds] => seconds
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite())
            .map(RuntimeAction::Seek),
        ["add", "volume", amount] => amount
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite())
            .map(RuntimeAction::Volume),
        ["add", "sub-pos", amount] => amount
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite())
            .map(RuntimeAction::SubtitlePosition),
        _ if riina_core::key_bindings::mpv_action_commands(action).is_ok() => {
            Some(RuntimeAction::MpvInputAction(action.to_owned()))
        }
        _ => None,
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn dispatch_system_media_key(
    app: &PlayerWindow,
    key: &str,
    latest_playback: &Arc<std::sync::Mutex<riina_core::Playback>>,
) -> bool {
    if !app.get_key_binding_use_system_media_controls() {
        return false;
    }
    let Ok(mappings) = mappings_for(&app.get_key_binding_current_profile()) else {
        return false;
    };
    let mappings = Bindings::new(mappings);
    let Some(mapping) = mappings.get(key) else {
        return false;
    };
    dispatch_mapping(app, mapping, latest_playback);
    true
}

#[cfg(any(target_os = "macos", test))]
pub(crate) fn preferred_system_skip_interval(mapping: Option<&Mapping>) -> Option<f64> {
    let Some(mapping) = mapping else {
        return Some(15.0);
    };
    if mapping.kind != ActionKind::Mpv {
        return None;
    }
    let commands = riina_core::key_bindings::mpv_action_commands(&mapping.action).ok()?;
    if commands.len() != 1 || commands[0].first().map(String::as_str) != Some("seek") {
        return None;
    }
    let command = &commands[0];
    let target = command.get(1)?.parse::<f64>().ok()?;
    if !target.is_finite() {
        return None;
    }
    if command.get(2).is_some_and(|flags| {
        !flags
            .split('+')
            .all(|flag| matches!(flag, "exact" | "keyframes" | "relative"))
    }) {
        return None;
    }
    Some(target.abs())
}

#[cfg(target_os = "macos")]
pub(crate) fn system_skip_intervals_for_profile(profile: &str) -> (Option<f64>, Option<f64>) {
    let Ok(mappings) = mappings_for(profile) else {
        return (None, None);
    };
    let mappings = Bindings::new(mappings);
    let interval_for = |key| preferred_system_skip_interval(mappings.get(key));
    (interval_for("FORWARD"), interval_for("REWIND"))
}

fn dispatch_mapping(
    app: &PlayerWindow,
    mapping: &Mapping,
    latest_playback: &Arc<std::sync::Mutex<riina_core::Playback>>,
) {
    let Some(action) = runtime_action(mapping) else {
        app.set_status(format!("Input action is not supported yet: {}", mapping.action).into());
        return;
    };
    match action {
        RuntimeAction::TogglePause => app.invoke_toggle_pause(),
        RuntimeAction::SetPaused(paused) => app.invoke_set_paused(paused),
        RuntimeAction::Quit => app.invoke_quit_riina(),
        RuntimeAction::Stop => app.invoke_stop(),
        RuntimeAction::ShowProgress => {
            let message = latest_playback.lock().ok().and_then(|playback| {
                crate::video_osd::progress_message(playback.position, playback.duration)
            });
            if let Some(message) = message {
                app.set_video_osd_request_feedback(false);
                app.set_video_osd_text(message.text.into());
                app.set_video_osd_progress(message.progress.unwrap_or(-1.0));
                app.invoke_show_video_osd();
            }
        }
        RuntimeAction::Seek(seconds) => app.invoke_seek_relative(seconds),
        RuntimeAction::Volume(amount) => app.invoke_adjust_volume(amount),
        RuntimeAction::Mute => app.invoke_toggle_mute(),
        RuntimeAction::SubtitleVisibility => app.invoke_toggle_subtitle_visibility(),
        RuntimeAction::Fullscreen(desired) => {
            if desired.is_none_or(|desired| desired != app.get_fullscreen()) {
                app.invoke_toggle_fullscreen();
            }
        }
        RuntimeAction::CycleAudio => app.invoke_cycle_audio_track(),
        RuntimeAction::CycleSubtitle => app.invoke_cycle_subtitle_track(),
        RuntimeAction::CycleVideo => app.invoke_cycle_video_track(),
        RuntimeAction::FrameStep => app.invoke_frame_step(),
        RuntimeAction::FrameBackStep => app.invoke_frame_back_step(),
        RuntimeAction::SubtitlePosition(amount) => app.invoke_adjust_subtitle_position(amount),
        RuntimeAction::OpenPreferences => app.invoke_open_preferences(),
        RuntimeAction::SettingsPane(index) => app.invoke_toggle_settings_pane(index),
        RuntimeAction::PlaylistNext => app.invoke_playlist_next(),
        RuntimeAction::PlaylistPrevious => app.invoke_playlist_previous(),
        RuntimeAction::PlaylistShuffle => app.invoke_toggle_playlist_shuffle(),
        RuntimeAction::TogglePlaylistPane(kind) => app.invoke_toggle_playlist_pane(kind),
        RuntimeAction::ToggleMusicMode => app.invoke_toggle_music_mode(),
        RuntimeAction::TogglePictureInPicture => app.invoke_toggle_picture_in_picture(),
        RuntimeAction::FitToScreen => app.invoke_resize_window_command(3),
        RuntimeAction::ResizeWindow(command) => app.invoke_resize_window_command(command),
        RuntimeAction::TogglePluginsPanel => app.invoke_toggle_plugins_pane(),
        RuntimeAction::FindOnlineSubs => app.invoke_find_online_subtitles(),
        RuntimeAction::ToggleLiveText => app.invoke_toggle_live_text(),
        RuntimeAction::RevealCurrentMedia => reveal_current_media(app, latest_playback),
        RuntimeAction::AbLoop => app.invoke_cycle_ab_loop(),
        RuntimeAction::Screenshot => app.invoke_take_screenshot(),
        RuntimeAction::MpvInputAction(action) => app.invoke_execute_mpv_input_action(action.into()),
        RuntimeAction::Ignore => {}
    }
}

pub(crate) enum CurrentMediaLocation {
    NoMedia,
    NonLocal,
    Local(std::path::PathBuf),
}

pub(crate) fn reveal_current_media(
    app: &PlayerWindow,
    latest_playback: &Arc<std::sync::Mutex<riina_core::Playback>>,
) {
    let location = latest_playback
        .lock()
        .ok()
        .map_or(CurrentMediaLocation::NoMedia, |playback| {
            current_media_location(&playback)
        });
    let path = match location {
        CurrentMediaLocation::NoMedia => {
            app.set_status("There is no current media file to reveal".into());
            return;
        }
        CurrentMediaLocation::NonLocal => {
            app.set_status("Only local media files can be revealed".into());
            return;
        }
        CurrentMediaLocation::Local(path) => path,
    };
    if !path.is_file() {
        app.set_status("The current media file is not available in the file manager".into());
        return;
    }
    if let Err(error) = crate::platform::reveal_file(&path) {
        app.set_status(format!("Could not reveal the current media file: {error}").into());
    }
}

pub(crate) fn current_media_location(playback: &riina_core::Playback) -> CurrentMediaLocation {
    let Some(filename) = playback
        .playlist
        .iter()
        .find(|entry| entry.playing)
        .or_else(|| playback.playlist.iter().find(|entry| entry.current))
        .map(|entry| entry.filename.clone())
    else {
        return CurrentMediaLocation::NoMedia;
    };
    let filename = if filename
        .split_once(':')
        .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case("file"))
    {
        riina_core::playlist::clipboard_references(&filename)
            .into_iter()
            .next()
    } else if filename.contains("://") {
        return CurrentMediaLocation::NonLocal;
    } else {
        Some(filename)
    };
    filename.map_or(CurrentMediaLocation::NonLocal, |filename| {
        CurrentMediaLocation::Local(std::path::PathBuf::from(filename))
    })
}
