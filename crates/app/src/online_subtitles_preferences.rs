use riina_core::online_subtitles::Preferences;

pub(crate) fn current_player(app: &crate::PlayerWindow) -> Result<Preferences, String> {
    let preferences = Preferences {
        api_key: app.get_open_subtitles_api_key().to_string(),
        auto_search: app.get_open_subtitles_auto_search(),
        auto_search_threshold_minutes: app
            .get_open_subtitles_threshold_text()
            .trim()
            .parse()
            .map_err(|_| {
                "Online subtitle threshold must be a whole number of minutes".to_owned()
            })?,
    };
    preferences.validate().map_err(str::to_owned)?;
    Ok(preferences)
}

pub(crate) fn current_window(window: &crate::PreferencesWindow) -> Result<Preferences, String> {
    let preferences = Preferences {
        api_key: window.get_open_subtitles_api_key().to_string(),
        auto_search: window.get_open_subtitles_auto_search(),
        auto_search_threshold_minutes: window
            .get_open_subtitles_threshold_text()
            .trim()
            .parse()
            .map_err(|_| {
                "Online subtitle threshold must be a whole number of minutes".to_owned()
            })?,
    };
    preferences.validate().map_err(str::to_owned)?;
    Ok(preferences)
}

pub(crate) fn install_player(app: &crate::PlayerWindow, preferences: &Preferences) {
    app.set_open_subtitles_api_key(preferences.api_key.clone().into());
    app.set_open_subtitles_auto_search(preferences.auto_search);
    app.set_open_subtitles_threshold_text(
        preferences.auto_search_threshold_minutes.to_string().into(),
    );
}

pub(crate) fn install_window(window: &crate::PreferencesWindow, preferences: &Preferences) {
    window.set_open_subtitles_api_key(preferences.api_key.clone().into());
    window.set_open_subtitles_auto_search(preferences.auto_search);
    window.set_open_subtitles_threshold_text(
        preferences.auto_search_threshold_minutes.to_string().into(),
    );
}
