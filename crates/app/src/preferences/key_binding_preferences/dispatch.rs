//! Install and execute typed profile actions without retaining playback owners.
use super::{
    Bindings,
    keys::{normalize_profile_key, recorded_key},
    mappings_for, menu_shortcut_rows,
};
use crate::{PlayerWindow, preferences::model::PreferenceSection};
use riina_core::key_bindings::{
    ActionKind, InputCommand, Mapping, Settings, normalize_mpv_key, parse_mpv_input,
};
use riina_ui::PlayerMenuActions;
use slint::{ComponentHandle, ModelRc, VecModel};
use std::rc::Rc;

pub(crate) fn install_player(
    app: &PlayerWindow,
    settings: &Settings,
    latest_playback: crate::PlaybackStore,
) {
    let bindings = match load_bindings(&settings.current_profile) {
        Ok(bindings) => bindings,
        Err(error) => {
            crate::diagnostics::report(app, format!("Could not load input profile: {error}"));
            let defaults = Settings::default();
            load_bindings(&defaults.current_profile)
                .unwrap_or_else(|_| Rc::new(Bindings::new(Vec::new())))
        }
    };
    install_bindings(app, settings, bindings, latest_playback);
}

pub(crate) fn load_bindings(profile: &str) -> Result<Rc<Bindings>, String> {
    mappings_for(profile).map(|mappings| Rc::new(Bindings::new(mappings)))
}

pub(crate) fn install_bindings(
    app: &PlayerWindow,
    settings: &Settings,
    mappings: Rc<Bindings>,
    latest_playback: crate::PlaybackStore,
) {
    settings.write_player(app);
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
    install_dispatch(app, mappings, latest_playback);
}

pub(super) fn install_dispatch(
    app: &PlayerWindow,
    mappings: Rc<Bindings>,
    observed_playback: crate::PlaybackStore,
) {
    let player = app.as_weak();
    app.global::<riina_ui::PlayerControlActions>()
        .on_dispatch_key_binding(move |text, control, alt, shift, meta, repeat| {
            let Some(key) = recorded_key(text.as_str(), control, alt, shift, meta) else {
                return false;
            };
            let mapping = mappings.get(&key).or_else(|| {
                // Shift can be needed to produce a punctuation symbol on the current layout.
                // An explicit Shift binding still wins over a binding to the symbol alone.
                let text = text.as_str();
                if !shift || text.len() != 1 || !"!@#$%^&*()_+{}|:\"<>?~".contains(text) {
                    return None;
                }
                mappings.get(&recorded_key(text, control, alt, false, meta)?)
            });
            let Some(mapping) = mapping else {
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
            let (action, repeat_override) = resolve_action(mapping);
            if !repeat || allows_repeat(action.as_ref(), repeat_override) {
                dispatch_action(&app, mapping, action, &observed_playback);
            }
            true
        });
}

fn allows_repeat(action: Option<&RuntimeAction>, override_value: Option<bool>) -> bool {
    match action {
        Some(RuntimeAction::TrashCurrentMedia | RuntimeAction::DeleteCurrentMediaPermanently)
        | None => false,
        Some(
            RuntimeAction::Screenshot | RuntimeAction::Quit { .. } | RuntimeAction::OpenMpvConsole,
        ) => override_value.unwrap_or(false),
        Some(_) => override_value.unwrap_or(true),
    }
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
    Quit { save_position: bool },
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
    ResizeProfileWindow(String),
    WindowLevel(Option<bool>),
    ToggleFileLoop,
    TogglePluginsPanel,
    FindOnlineSubs,
    OpenFile,
    OpenUrl,
    SaveCurrentPlaylist,
    SaveDownloadedSubtitle,
    RevealCurrentMedia,
    TrashCurrentMedia,
    DeleteCurrentMediaPermanently,
    ToggleVideoReflection { horizontal: bool },
    ToggleLiveText,
    AbLoop,
    Screenshot,
    OpenMpvConsole,
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
    resolve_action(mapping).0
}

fn resolve_action(mapping: &Mapping) -> (Option<RuntimeAction>, Option<bool>) {
    let action = mapping.action.trim();
    if mapping.kind == ActionKind::Riina {
        let action = match action {
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
            "open-file" => Some(RuntimeAction::OpenFile),
            "open-url" => Some(RuntimeAction::OpenUrl),
            "save-playlist" => Some(RuntimeAction::SaveCurrentPlaylist),
            "save-downloaded-sub" => Some(RuntimeAction::SaveDownloadedSubtitle),
            "show-current-file-in-finder" => Some(RuntimeAction::RevealCurrentMedia),
            "delete-current-file" => Some(RuntimeAction::TrashCurrentMedia),
            "delete-current-file-hard" => Some(RuntimeAction::DeleteCurrentMediaPermanently),
            "toggle-mirror" => Some(RuntimeAction::ToggleVideoReflection { horizontal: true }),
            "toggle-flip" => Some(RuntimeAction::ToggleVideoReflection { horizontal: false }),
            "live-text" => Some(RuntimeAction::ToggleLiveText),
            "plugin-panel" => Some(RuntimeAction::TogglePluginsPanel),
            _ => None,
        };
        return (action, None);
    }
    let Ok(commands) = parse_mpv_input(action) else {
        return (None, None);
    };
    let repeat = commands.first().and_then(InputCommand::repeat_override);
    let command = match commands.as_slice() {
        [command] => mpv_runtime_action(action, command),
        _ => Some(RuntimeAction::MpvInputAction(action.to_owned())),
    };
    (command, repeat)
}

fn mpv_runtime_action(action: &str, command: &InputCommand) -> Option<RuntimeAction> {
    let words: Vec<_> = command.arguments().iter().map(String::as_str).collect();
    if words == ["script-binding", "console/enable"] {
        return Some(RuntimeAction::OpenMpvConsole);
    }
    if !command.prefixes().is_empty()
        && !matches!(
            words.as_slice(),
            ["screenshot" | "quit" | "quit-watch-later", ..]
                | ["cycle" | "set", "fullscreen" | "ontop", ..]
        )
        && !words.contains(&"window-scale")
    {
        return Some(RuntimeAction::MpvInputAction(action.to_owned()));
    }
    match words.as_slice() {
        ["ignore"] => Some(RuntimeAction::Ignore),
        ["cycle", "pause"] => Some(RuntimeAction::TogglePause),
        ["set", "pause", "yes"] => Some(RuntimeAction::SetPaused(true)),
        ["set", "pause", "no"] => Some(RuntimeAction::SetPaused(false)),
        ["quit"] => Some(RuntimeAction::Quit {
            save_position: false,
        }),
        ["quit-watch-later"] => Some(RuntimeAction::Quit {
            save_position: true,
        }),
        [command @ ("quit" | "quit-watch-later"), code] if code.parse::<i32>().is_ok() => {
            Some(RuntimeAction::Quit {
                save_position: *command == "quit-watch-later",
            })
        }
        ["stop"] => Some(RuntimeAction::Stop),
        ["show-progress"] => Some(RuntimeAction::ShowProgress),
        ["cycle", "mute"] => Some(RuntimeAction::Mute),
        ["cycle", "ontop"] => Some(RuntimeAction::WindowLevel(None)),
        ["set", "ontop", "yes"] => Some(RuntimeAction::WindowLevel(Some(true))),
        ["set", "ontop", "no"] => Some(RuntimeAction::WindowLevel(Some(false))),
        ["cycle", "sub-visibility"] => Some(RuntimeAction::SubtitleVisibility),
        ["cycle", "fullscreen"] => Some(RuntimeAction::Fullscreen(None)),
        ["set", "fullscreen", "yes"] => Some(RuntimeAction::Fullscreen(Some(true))),
        ["set", "fullscreen", "no"] => Some(RuntimeAction::Fullscreen(Some(false))),
        ["cycle", "audio"] | ["cycle", "audio", "up"] => Some(RuntimeAction::CycleAudio),
        ["cycle", "sub"] | ["cycle", "sub", "up"] => Some(RuntimeAction::CycleSubtitle),
        ["cycle", "video"] | ["cycle", "video", "up"] => Some(RuntimeAction::CycleVideo),
        ["frame-step"] => Some(RuntimeAction::FrameStep),
        ["frame-back-step"] => Some(RuntimeAction::FrameBackStep),
        ["playlist-next"] => Some(RuntimeAction::PlaylistNext),
        ["playlist-prev"] => Some(RuntimeAction::PlaylistPrevious),
        ["playlist-shuffle"] => Some(RuntimeAction::PlaylistShuffle),
        ["ab-loop"] => Some(RuntimeAction::AbLoop),
        ["screenshot", ..] => Some(RuntimeAction::Screenshot),
        ["set", "window-scale", scale] => match *scale {
            "0.5" => Some(RuntimeAction::ResizeWindow(0)),
            "1" | "1.0" => Some(RuntimeAction::ResizeWindow(1)),
            "2" | "2.0" => Some(RuntimeAction::ResizeWindow(2)),
            _ => Some(RuntimeAction::ResizeProfileWindow(action.to_owned())),
        },
        ["multiply", "window-scale", _]
        | ["cycle-values", "window-scale", ..]
        | ["cycle-values", "!reverse", "window-scale", ..] => {
            Some(RuntimeAction::ResizeProfileWindow(action.to_owned()))
        }
        ["cycle-values", "loop" | "loop-file", enabled, disabled]
            if *enabled == "inf" && *disabled == "no" =>
        {
            Some(RuntimeAction::ToggleFileLoop)
        }
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
        _ => Some(RuntimeAction::MpvInputAction(action.to_owned())),
    }
}

fn dispatch_action(
    app: &PlayerWindow,
    mapping: &Mapping,
    action: Option<RuntimeAction>,
    latest_playback: &crate::PlaybackStore,
) {
    let Some(action) = action else {
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
        RuntimeAction::Quit { save_position } => {
            if save_position {
                app.global::<riina_ui::PlayerControlActions>()
                    .invoke_execute_mpv_input_action("write-watch-later-config".into());
            }
            app.global::<PlayerMenuActions>().invoke_quit_riina();
        }
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
        RuntimeAction::ResizeProfileWindow(action) => {
            app.invoke_profile_window_scale_requested(action.into())
        }
        RuntimeAction::WindowLevel(value) => {
            let layout = app.global::<riina_ui::PlayerLayoutState>();
            layout.set_always_float_on_top(
                value.unwrap_or_else(|| !layout.get_always_float_on_top()),
            );
        }
        RuntimeAction::ToggleFileLoop => {
            app.global::<PlayerMenuActions>().invoke_toggle_file_loop()
        }
        RuntimeAction::TogglePluginsPanel => app
            .global::<PlayerMenuActions>()
            .invoke_toggle_plugins_pane(),
        RuntimeAction::FindOnlineSubs => app
            .global::<PlayerMenuActions>()
            .invoke_find_online_subtitles(),
        RuntimeAction::ToggleLiveText => {
            app.global::<PlayerMenuActions>().invoke_toggle_live_text()
        }
        RuntimeAction::OpenFile => app.global::<PlayerMenuActions>().invoke_open_file(),
        RuntimeAction::OpenUrl => app.global::<PlayerMenuActions>().invoke_open_url_dialog(),
        RuntimeAction::SaveCurrentPlaylist => app
            .global::<PlayerMenuActions>()
            .invoke_save_current_playlist(),
        RuntimeAction::SaveDownloadedSubtitle => app
            .global::<PlayerMenuActions>()
            .invoke_save_downloaded_subtitle(),
        RuntimeAction::RevealCurrentMedia => app
            .global::<PlayerMenuActions>()
            .invoke_reveal_current_media(),
        RuntimeAction::TrashCurrentMedia => app
            .global::<PlayerMenuActions>()
            .invoke_delete_current_media(),
        RuntimeAction::DeleteCurrentMediaPermanently => app
            .global::<PlayerMenuActions>()
            .invoke_delete_current_media_permanently(),
        RuntimeAction::ToggleVideoReflection { horizontal } => app
            .global::<PlayerMenuActions>()
            .invoke_toggle_video_reflection(horizontal),
        RuntimeAction::AbLoop => app.global::<PlayerMenuActions>().invoke_cycle_ab_loop(),
        RuntimeAction::Screenshot if mapping.action.trim() == "screenshot" => {
            app.global::<PlayerMenuActions>().invoke_take_screenshot()
        }
        RuntimeAction::Screenshot => app
            .global::<riina_ui::PlayerControlActions>()
            .invoke_take_key_screenshot(mapping.action.as_str().into()),
        RuntimeAction::OpenMpvConsole => app
            .global::<riina_ui::PlayerControlActions>()
            .invoke_open_mpv_console(),
        RuntimeAction::MpvInputAction(action) => app
            .global::<riina_ui::PlayerControlActions>()
            .invoke_execute_mpv_input_action(action.into()),
        RuntimeAction::Ignore => {}
    }
}
