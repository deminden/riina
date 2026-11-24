//! Install and execute typed profile actions without retaining playback owners.
use super::{
    bindings::Bindings,
    builtins::mappings_for,
    keys::{normalize_profile_key, recorded_key},
    menus::menu_shortcut_rows,
};
use crate::{PlayerWindow, preferences::model::PreferenceSection};
use riina_core::key_bindings::{ActionKind, Mapping, Settings, normalize_mpv_key};
use riina_ui::PlayerMenuActions;
use slint::{ComponentHandle, ModelRc, VecModel};
use std::rc::Rc;

pub(crate) fn install_player(
    app: &PlayerWindow,
    settings: &Settings,
    latest_playback: crate::PlaybackStore,
) {
    settings.write_player(app);
    let mappings = match mappings_for(&settings.current_profile) {
        Ok(mappings) => mappings,
        Err(error) => {
            crate::diagnostics::report(app, format!("Could not load input profile: {error}"));
            Vec::new()
        }
    };
    let mappings = Rc::new(Bindings::new(mappings));
    crate::platform::player_touch_bar::bind_escape(app, || {
        matches!(
            mappings.get("ESC").and_then(runtime_action),
            Some(RuntimeAction::Fullscreen(Some(false)))
        )
    });
    let input_bindings = slint::SharedString::from(plugin_input_bindings(&mappings));
    app.set_plugin_input_bindings(input_bindings.clone());
    // Player entries can start before Slint delivers property change callbacks.
    app.invoke_plugin_input_bindings_changed(input_bindings);
    app.set_menu_shortcuts(ModelRc::new(VecModel::from(menu_shortcut_rows(&mappings))));
    let profile = mappings.clone();
    app.on_profile_has_shortcut(move |key| {
        profile.get(&normalize_profile_key(key.as_str())).is_some()
    });
    crate::player_ui::refresh_plugin_menu_shortcuts(app);
    let player = app.as_weak();
    let observed_playback = latest_playback;
    app.global::<riina_ui::PlayerControlActions>()
        .on_dispatch_key_binding(move |text, control, alt, shift, meta| {
            let Some(key) = recorded_key(text.as_str(), control, alt, shift, meta) else {
                return false;
            };
            let Some(mapping) = mappings.get(&key) else {
                return false;
            };
            let Some(app) = player.upgrade() else {
                return true;
            };
            if is_system_media_key(&key)
                && !app
                    .global::<riina_ui::PlayerPreferences>()
                    .get_key_binding_use_system_media_controls()
            {
                return false;
            }
            dispatch_mapping(&app, mapping, &observed_playback);
            true
        });
}

fn plugin_input_bindings(mappings: &Bindings) -> String {
    let bindings: serde_json::Map<_, _> = mappings
        .effective()
        .map(|mapping| {
            let key = normalize_mpv_key(&mapping.key);
            let binding = serde_json::json!({
                "key": key,
                "action": mapping.action,
                "isIINACommand": mapping.kind == ActionKind::Riina,
            });
            (key, binding)
        })
        .collect();
    serde_json::Value::Object(bindings).to_string()
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

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub(crate) fn dispatch_system_media_key(
    app: &PlayerWindow,
    key: &str,
    latest_playback: &crate::PlaybackStore,
) -> bool {
    if !app
        .global::<riina_ui::PlayerPreferences>()
        .get_key_binding_use_system_media_controls()
    {
        return false;
    }
    let Ok(mappings) = mappings_for(
        &app.global::<riina_ui::PlayerPreferences>()
            .get_key_binding_current_profile(),
    ) else {
        return false;
    };
    let mappings = Bindings::new(mappings);
    let Some(mapping) = mappings.get(key) else {
        return false;
    };
    dispatch_mapping(app, mapping, latest_playback);
    true
}

#[cfg(any(target_os = "macos", target_os = "windows", test))]
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

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(crate) fn system_skip_intervals_for_profile(profile: &str) -> (Option<f64>, Option<f64>) {
    let Ok(mappings) = mappings_for(profile) else {
        return (None, None);
    };
    let mappings = Bindings::new(mappings);
    let interval_for = |key| preferred_system_skip_interval(mappings.get(key));
    (interval_for("FORWARD"), interval_for("REWIND"))
}

fn dispatch_mapping(app: &PlayerWindow, mapping: &Mapping, latest_playback: &crate::PlaybackStore) {
    let Some(action) = runtime_action(mapping) else {
        crate::diagnostics::report(
            app,
            format!("Input action is not supported yet: {}", mapping.action),
        );
        return;
    };
    match action {
        RuntimeAction::TogglePause => app.global::<PlayerMenuActions>().invoke_toggle_pause(),
        RuntimeAction::SetPaused(paused) => app
            .global::<riina_ui::PlayerControlActions>()
            .invoke_set_paused(paused),
        RuntimeAction::Quit => app.global::<PlayerMenuActions>().invoke_quit_riina(),
        RuntimeAction::Stop => app.global::<PlayerMenuActions>().invoke_stop(),
        RuntimeAction::ShowProgress => {
            let message = {
                let playback = riina_core::sync::lock(latest_playback);
                crate::video_osd::progress_message(playback.position, playback.duration)
            };
            if let Some(message) = message {
                app.set_video_osd_request_feedback(false);
                app.global::<riina_ui::PlayerOsdState>()
                    .set_video_osd_text(message.text.into());
                app.global::<riina_ui::PlayerOsdState>()
                    .set_video_osd_progress(message.progress.unwrap_or(-1.0));
                app.global::<riina_ui::PlayerControlActions>()
                    .invoke_show_video_osd();
            }
        }
        RuntimeAction::Seek(seconds) => app
            .global::<PlayerMenuActions>()
            .invoke_seek_relative(seconds),
        RuntimeAction::Volume(amount) => app
            .global::<PlayerMenuActions>()
            .invoke_adjust_volume(amount),
        RuntimeAction::Mute => app.global::<PlayerMenuActions>().invoke_toggle_mute(),
        RuntimeAction::SubtitleVisibility => app
            .global::<PlayerMenuActions>()
            .invoke_toggle_subtitle_visibility(),
        RuntimeAction::Fullscreen(desired) => {
            if desired.is_none_or(|desired| desired != app.get_fullscreen()) {
                app.global::<PlayerMenuActions>().invoke_toggle_fullscreen();
            }
        }
        RuntimeAction::CycleAudio => app.global::<PlayerMenuActions>().invoke_cycle_audio_track(),
        RuntimeAction::CycleSubtitle => app
            .global::<PlayerMenuActions>()
            .invoke_cycle_subtitle_track(),
        RuntimeAction::CycleVideo => app.global::<PlayerMenuActions>().invoke_cycle_video_track(),
        RuntimeAction::FrameStep => app.global::<PlayerMenuActions>().invoke_frame_step(),
        RuntimeAction::FrameBackStep => app.global::<PlayerMenuActions>().invoke_frame_back_step(),
        RuntimeAction::SubtitlePosition(amount) => app
            .global::<riina_ui::PlayerControlActions>()
            .invoke_adjust_subtitle_position(amount),
        RuntimeAction::OpenPreferences => {
            app.global::<PlayerMenuActions>().invoke_open_preferences()
        }
        RuntimeAction::SettingsPane(index) => app
            .global::<PlayerMenuActions>()
            .invoke_toggle_settings_pane(index),
        RuntimeAction::PlaylistNext => app.global::<PlayerMenuActions>().invoke_playlist_next(),
        RuntimeAction::PlaylistPrevious => {
            app.global::<PlayerMenuActions>().invoke_playlist_previous()
        }
        RuntimeAction::PlaylistShuffle => app
            .global::<riina_ui::PlayerSidebarActions>()
            .invoke_toggle_playlist_shuffle(),
        RuntimeAction::TogglePlaylistPane(kind) => app
            .global::<PlayerMenuActions>()
            .invoke_toggle_playlist_pane(kind),
        RuntimeAction::ToggleMusicMode => {
            app.global::<PlayerMenuActions>().invoke_toggle_music_mode()
        }
        RuntimeAction::TogglePictureInPicture => app
            .global::<PlayerMenuActions>()
            .invoke_toggle_picture_in_picture(),
        RuntimeAction::FitToScreen => app
            .global::<PlayerMenuActions>()
            .invoke_resize_window_command(3),
        RuntimeAction::ResizeWindow(command) => app
            .global::<PlayerMenuActions>()
            .invoke_resize_window_command(command),
        RuntimeAction::TogglePluginsPanel => app
            .global::<PlayerMenuActions>()
            .invoke_toggle_plugins_pane(),
        RuntimeAction::FindOnlineSubs => app
            .global::<PlayerMenuActions>()
            .invoke_find_online_subtitles(),
        RuntimeAction::ToggleLiveText => {
            app.global::<PlayerMenuActions>().invoke_toggle_live_text()
        }
        RuntimeAction::RevealCurrentMedia => reveal_current_media(app, latest_playback),
        RuntimeAction::AbLoop => app.global::<PlayerMenuActions>().invoke_cycle_ab_loop(),
        RuntimeAction::Screenshot => app.global::<PlayerMenuActions>().invoke_take_screenshot(),
        RuntimeAction::MpvInputAction(action) => app
            .global::<riina_ui::PlayerControlActions>()
            .invoke_execute_mpv_input_action(action.into()),
        RuntimeAction::Ignore => {}
    }
}

pub(crate) enum CurrentMediaLocation {
    NoMedia,
    NonLocal,
    Local(std::path::PathBuf),
}

pub(crate) fn reveal_current_media(app: &PlayerWindow, latest_playback: &crate::PlaybackStore) {
    let location = current_media_location(&riina_core::sync::lock(latest_playback));
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
        crate::diagnostics::report(
            app,
            format!("Could not reveal the current media file: {error}"),
        );
    }
}

pub(crate) fn current_media_location(playback: &riina_core::Playback) -> CurrentMediaLocation {
    let Some(filename) = playback
        .playlist
        .iter()
        .find(|entry| entry.playing)
        .or_else(|| playback.playlist.iter().find(|entry| entry.current))
        .map(|entry| entry.filename.as_str())
    else {
        return CurrentMediaLocation::NoMedia;
    };
    crate::file_actions::local_media_path(filename)
        .map_or(CurrentMediaLocation::NonLocal, CurrentMediaLocation::Local)
}
