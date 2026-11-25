//! Initialize the Settings window and register its page controllers.
use super::{changes, profile_window};
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
    pub(super) replay_gain: &'a crate::player_ui::ReplayGainBinding,
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
        replay_gain,
    } = inputs;
    let available_devices = latest_playback
        .lock()
        .map(|playback| playback.audio_devices.clone())
        .unwrap_or_default();
    let window = match profile_window::create(path, baseline, &available_devices, clear_history) {
        Ok(window) => window,
        Err(error) => {
            app.set_status(error.into());
            return None;
        }
    };
    crate::plugin_preferences::connect(
        &window,
        path,
        plugin_host.map(|host| host.global.clone()),
        Some((app, decoder_player.clone())),
    );
    let log_owner = app.as_weak();
    profile_window::connect_actions(&window, Some(app), path, move || {
        if let Some(app) = log_owner.upgrade() {
            app.global::<PlayerMenuActions>().invoke_show_log_viewer();
        }
    });
    crate::online_subtitles_preferences::install_window_catalog(&window, app);
    connect_subtitle_font(app, &window);
    replay_gain.attach(&window);
    let replay_gain = replay_gain.clone();
    changes::connect(
        app,
        &window,
        decoder_player.clone(),
        model.clone(),
        latest_playback.clone(),
        move |options| replay_gain.publish(options),
    );
    Some(window)
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
