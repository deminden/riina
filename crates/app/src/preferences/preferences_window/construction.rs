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
    pub(super) profile: &'a crate::player_ui::ProfilePlaybackBinding,
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
        profile,
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
    profile.attach(&window);
    let profile = profile.clone();
    changes::connect(
        app,
        &window,
        decoder_player.clone(),
        model.clone(),
        move |edit| {
            if let Some(options) = edit.replay_gain {
                profile.publish_replay_gain(options);
            }
            if let Some(font) = edit.subtitle_font {
                profile.publish_subtitle_font(font);
            }
            if let Some(settings) = edit.subtitle_auto_load {
                profile.publish_subtitle_auto_load(settings);
            }
            if let Some(settings) = edit.key_bindings {
                profile.publish_key_bindings(settings);
            }
        },
    );
    Some(window)
}
