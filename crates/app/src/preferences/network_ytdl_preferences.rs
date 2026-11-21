//! Bind IINA's optional online-video extraction settings to the shared UI state.
use slint::ComponentHandle;

use super::model::{PreferenceSection, SectionError};
use crate::{PlayerWindow, PreferencesWindow};
use riina_core::Preferences;
use riina_core::network_preferences::OnlineVideos;

impl PreferenceSection for OnlineVideos {
    fn get(all: &Preferences) -> Self {
        all.network_online_videos.clone()
    }

    fn put(self, all: &mut Preferences) {
        all.network_online_videos = self;
    }

    fn read_window(window: &PreferencesWindow) -> Result<Self, SectionError> {
        current_window(window).map_err(SectionError::from)
    }

    fn write_window(&self, window: &PreferencesWindow) {
        install_window(window, self);
    }

    fn write_player(&self, app: &PlayerWindow) {
        install_player(app, self);
    }
}

fn validate(preferences: OnlineVideos) -> Result<OnlineVideos, String> {
    preferences.validate().map_err(str::to_owned)?;
    Ok(preferences)
}

pub(crate) fn current_player(app: &crate::PlayerWindow) -> Result<OnlineVideos, String> {
    validate(OnlineVideos {
        enabled: app.get_network_ytdl_enabled(),
        search_path: app.get_network_ytdl_search_path().to_string(),
        raw_options: app.get_network_ytdl_raw_options().to_string(),
    })
}

pub(crate) fn current_window(window: &crate::PreferencesWindow) -> Result<OnlineVideos, String> {
    let form = window.global::<crate::NetworkSettingsState>();
    validate(OnlineVideos {
        enabled: form.get_network_ytdl_enabled(),
        search_path: form.get_network_ytdl_search_path().to_string(),
        raw_options: form.get_network_ytdl_raw_options().to_string(),
    })
}

pub(crate) fn install_player(app: &crate::PlayerWindow, preferences: &OnlineVideos) {
    app.set_network_ytdl_enabled(preferences.enabled);
    app.set_network_ytdl_search_path(preferences.search_path.clone().into());
    app.set_network_ytdl_raw_options(preferences.raw_options.clone().into());
}

pub(crate) fn install_window(window: &crate::PreferencesWindow, preferences: &OnlineVideos) {
    let form = window.global::<crate::NetworkSettingsState>();
    form.set_network_ytdl_enabled(preferences.enabled);
    form.set_network_ytdl_ui_enabled(preferences.enabled && !form.get_network_ytdl_plugin_active());
    form.set_network_ytdl_search_path(preferences.search_path.clone().into());
    form.set_network_ytdl_raw_options(preferences.raw_options.clone().into());
}

pub(crate) fn apply(
    player: &riina_mpv::CommandSender,
    app: &crate::PlayerWindow,
    previous: &OnlineVideos,
    selected: &OnlineVideos,
) -> Result<(), String> {
    if previous != selected {
        let mut effective = selected.clone();
        if app.get_network_ytdl_plugin_active() {
            effective.enabled = false;
        }
        player
            .send(riina_core::Command::NetworkOnlineVideos(effective))
            .map_err(|error| error.to_string())?;
        install_player(app, selected);
    }
    Ok(())
}

pub(crate) fn apply_plugin_override(
    player: &riina_mpv::CommandSender,
    app: &crate::PlayerWindow,
    plugin_active: bool,
) -> Result<(), String> {
    let mut effective = current_player(app)?;
    if plugin_active {
        effective.enabled = false;
    }
    player
        .send(riina_core::Command::NetworkOnlineVideos(effective))
        .map_err(|error| error.to_string())
}

pub(crate) fn restore(
    player: &riina_mpv::CommandSender,
    app: &crate::PlayerWindow,
    previous: &OnlineVideos,
    selected: &OnlineVideos,
) {
    if previous != selected {
        let mut effective = previous.clone();
        if app.get_network_ytdl_plugin_active() {
            effective.enabled = false;
        }
        let _ = player.send(riina_core::Command::NetworkOnlineVideos(effective));
        install_player(app, previous);
    }
}

pub(crate) fn refresh_window(window: &crate::PreferencesWindow, app: &crate::PlayerWindow) {
    if let Ok(preferences) = current_player(app) {
        install_window(window, &preferences);
    }
}
