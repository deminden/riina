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
    let available_devices = latest_playback
        .lock()
        .map(|playback| playback.audio_devices.clone())
        .unwrap_or_default();
    let window =
        match super::profile_window::create(path, baseline, &available_devices, clear_history) {
            Ok(window) => window,
            Err(error) => {
                app.set_status(error.into());
                return None;
            }
        };
    connect_assrt_instructions(app, &window);
    crate::plugin_preferences::connect(
        &window,
        app,
        path,
        plugin_host.cloned(),
        decoder_player.clone(),
    );
    crate::advanced_preferences::connect_actions(app, &window, path.to_path_buf());
    crate::online_subtitles_preferences::install_window_catalog(&window, app);
    crate::platform::preferences::install_subtitle_color_panel(app, &window);
    key_bindings::connect(app, &window);
    connect_subtitle_font(app, &window);
    changes::connect(
        app,
        &window,
        decoder_player.clone(),
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
            crate::diagnostics::report(
                &app,
                format!("Could not open Assrt API instructions: {error}"),
            );
        }
    });
}

fn connect_subtitle_font(app: &PlayerWindow, window: &PreferencesWindow) {
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
}
