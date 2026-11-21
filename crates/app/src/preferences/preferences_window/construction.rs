//! Initialize the Settings window and register its page controllers.
use super::{changes, key_bindings};
use crate::preferences::model::PreferenceModel;
use crate::{PlaybackStore, PlayerWindow, PreferencesWindow, plugin_host::PluginHostHandle};
use riina_core::Preferences;
use riina_mpv::CommandSender;
use riina_ui::PlayerMenuActions;
use slint::ComponentHandle;
use std::path::Path;

pub(super) struct Inputs<'a> {
    pub(super) baseline: &'a Preferences,
    pub(super) path: &'a Path,
    pub(super) model: &'a PreferenceModel,
    pub(super) player: &'a CommandSender,
    pub(super) playback: &'a PlaybackStore,
    pub(super) plugin: Option<&'a PluginHostHandle>,
}

pub(super) fn create(
    app: &PlayerWindow,
    inputs: Inputs<'_>,
    clear_history: impl Fn() -> Result<usize, String> + 'static,
) -> Option<PreferencesWindow> {
    let Inputs {
        baseline,
        path,
        model,
        player: decoder_player,
        playback: latest_playback,
        plugin: plugin_host,
    } = inputs;
    let window = match crate::PreferencesWindow::new() {
        Ok(window) => window,
        Err(error) => {
            app.set_status(error.to_string().into());
            return None;
        }
    };
    crate::appearance_preferences::install_system_typography(&window);
    crate::platform::enable_preferences_mica(&window);
    crate::settings_search::connect(&window);
    connect_assrt_instructions(app, &window);
    crate::language_token_preferences::connect(&window);
    crate::osc_preferences::connect_preferences(&window);
    crate::utilities_preferences::connect(&window, path, clear_history);
    crate::plugin_preferences::connect(
        &window,
        app,
        path,
        plugin_host.cloned(),
        decoder_player.clone(),
    );
    crate::advanced_preferences::install_window(&window, &baseline.advanced);
    crate::advanced_preferences::connect_actions(app, &window, path.to_path_buf());
    let available_devices = latest_playback
        .lock()
        .map(|playback| playback.audio_devices.clone())
        .unwrap_or_default();
    crate::audio_preferences::install_window(&window, &baseline.audio, &available_devices);
    crate::tone_mapping_preferences::install_window(&window, &baseline.hdr_tone_mapping);
    window.set_load_icc_profile(baseline.load_icc_profile);
    window.set_default_hdr_mode_enabled(baseline.hdr_enabled);
    window.set_video_threads_text(baseline.video_threads.to_string().into());
    crate::control_preferences::install_window(&window, &baseline.controls);
    if let Err(error) =
        crate::key_binding_preferences::install_window(&window, &baseline.key_bindings)
    {
        app.set_status(error.into());
        return None;
    }
    crate::network_connection_preferences::install_window(&window, &baseline.network);
    crate::network_ytdl_preferences::install_window(&window, &baseline.network_online_videos);
    crate::pip_preferences::install_preferences(&window, baseline.picture_in_picture);
    crate::thumbnail_preferences::install_window(&window, &baseline.thumbnail_preview);
    crate::online_subtitles_preferences::install_window(&window, &baseline.online_subtitles);
    crate::window_resize_preferences::install_preferences(&window, baseline.window_resize);
    window
        .global::<riina_ui::GeneralSettingsState>()
        .set_allow_multiple_windows(baseline.allow_multiple_windows);
    window
        .global::<riina_ui::GeneralSettingsState>()
        .set_group_simultaneous_opens_in_playlist(baseline.group_simultaneous_opens_in_playlist);
    window
        .global::<riina_ui::GeneralSettingsState>()
        .set_allow_duplicate_players(baseline.allow_duplicate_players);
    window
        .global::<riina_ui::GeneralSettingsState>()
        .set_playlist_auto_add(baseline.playlist_auto_add);
    crate::platform::preferences::install_subtitle_color_panel(app, &window);
    key_bindings::connect(app, &window);
    connect_font_and_advanced(app, &window);
    changes::connect(
        app,
        &window,
        decoder_player.clone(),
        path.to_path_buf(),
        model.clone(),
        latest_playback.clone(),
    );
    super::folder::register(app, &window);
    Some(window)
}

fn connect_assrt_instructions(app: &PlayerWindow, window: &PreferencesWindow) {
    let assrt_token_page_parent = app.as_weak();
    window.on_open_assrt_token_page(move || {
        if let Err(error) = crate::platform::open_http_url_in_browser("https://2.assrt.net/api/doc")
            && let Some(app) = assrt_token_page_parent.upgrade()
        {
            app.set_status(format!("Could not open Assrt API instructions: {error}").into());
        }
    });
}

fn connect_font_and_advanced(app: &PlayerWindow, window: &PreferencesWindow) {
    let selected_font = app
        .global::<riina_ui::PlaybackSubtitleState>()
        .get_subtitle_font()
        .to_string();
    window
        .global::<crate::SubtitleSettingsState>()
        .set_subtitle_font(selected_font.into());
    let choose_parent = app.as_weak();
    let choose_window = window.as_weak();
    window.on_choose_subtitle_font(move || {
        if let (Some(app), Some(window)) = (choose_parent.upgrade(), choose_window.upgrade()) {
            window
                .global::<crate::SubtitleSettingsState>()
                .set_subtitle_font(
                    app.global::<riina_ui::PlaybackSubtitleState>()
                        .get_subtitle_font(),
                );
            app.global::<PlayerMenuActions>()
                .invoke_choose_subtitle_font();
        }
    });
    let advanced_settings_window = window.as_weak();
    window.on_advanced_settings_changed(move || {
        if let Some(window) = advanced_settings_window.upgrade() {
            window.invoke_settings_changed();
        }
    });
}
