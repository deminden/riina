//! Bind IINA's optional online-video extraction settings to the shared UI state.

use riina_core::network_preferences::OnlineVideos;

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
    validate(OnlineVideos {
        enabled: window.get_network_ytdl_enabled(),
        search_path: window.get_network_ytdl_search_path().to_string(),
        raw_options: window.get_network_ytdl_raw_options().to_string(),
    })
}

pub(crate) fn install_player(app: &crate::PlayerWindow, preferences: &OnlineVideos) {
    app.set_network_ytdl_enabled(preferences.enabled);
    app.set_network_ytdl_search_path(preferences.search_path.clone().into());
    app.set_network_ytdl_raw_options(preferences.raw_options.clone().into());
}

pub(crate) fn install_window(window: &crate::PreferencesWindow, preferences: &OnlineVideos) {
    window.set_network_ytdl_enabled(preferences.enabled);
    window.set_network_ytdl_search_path(preferences.search_path.clone().into());
    window.set_network_ytdl_raw_options(preferences.raw_options.clone().into());
}

pub(crate) fn apply(
    player: &riina_mpv::CommandSender,
    app: &crate::PlayerWindow,
    previous: &OnlineVideos,
    selected: &OnlineVideos,
) -> Result<(), String> {
    if previous != selected {
        player
            .send(riina_core::Command::NetworkOnlineVideos(selected.clone()))
            .map_err(|error| error.to_string())?;
        install_player(app, selected);
    }
    Ok(())
}

pub(crate) fn restore(
    player: &riina_mpv::CommandSender,
    app: &crate::PlayerWindow,
    previous: &OnlineVideos,
    selected: &OnlineVideos,
) {
    if previous != selected {
        let _ = player.send(riina_core::Command::NetworkOnlineVideos(previous.clone()));
        install_player(app, previous);
    }
}

pub(crate) fn refresh_window(window: &crate::PreferencesWindow, app: &crate::PlayerWindow) {
    if let Ok(preferences) = current_player(app) {
        install_window(window, &preferences);
    }
}
