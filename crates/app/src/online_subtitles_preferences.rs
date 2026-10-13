use riina_core::online_subtitles::{Preferences, Provider};

fn provider(index: i32) -> Provider {
    match index {
        1 => Provider::Assrt,
        2 => Provider::Shooter,
        _ => Provider::OpenSubtitles,
    }
}

fn provider_index(provider: Provider) -> i32 {
    match provider {
        Provider::OpenSubtitles => 0,
        Provider::Assrt => 1,
        Provider::Shooter => 2,
    }
}

pub(crate) fn current_player(app: &crate::PlayerWindow) -> Result<Preferences, String> {
    let preferences = Preferences {
        provider: provider(app.get_open_subtitles_provider_index()),
        api_key: app.get_open_subtitles_api_key().to_string(),
        assrt_token: app.get_assrt_token().to_string(),
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
        provider: provider(window.get_open_subtitles_provider_index()),
        api_key: window.get_open_subtitles_api_key().to_string(),
        assrt_token: window.get_assrt_token().to_string(),
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
    app.set_open_subtitles_provider_index(provider_index(preferences.provider));
    app.set_open_subtitles_api_key(preferences.api_key.clone().into());
    app.set_assrt_token(preferences.assrt_token.clone().into());
    app.set_open_subtitles_auto_search(preferences.auto_search);
    app.set_open_subtitles_threshold_text(
        preferences.auto_search_threshold_minutes.to_string().into(),
    );
}

pub(crate) fn install_window(window: &crate::PreferencesWindow, preferences: &Preferences) {
    window.set_open_subtitles_provider_index(provider_index(preferences.provider));
    window.set_open_subtitles_api_key(preferences.api_key.clone().into());
    window.set_assrt_token(preferences.assrt_token.clone().into());
    window.set_open_subtitles_auto_search(preferences.auto_search);
    window.set_open_subtitles_threshold_text(
        preferences.auto_search_threshold_minutes.to_string().into(),
    );
}

#[cfg(test)]
mod tests {
    use super::{provider, provider_index};
    use riina_core::online_subtitles::Provider;

    #[test]
    fn provider_preference_index_round_trips_all_supported_services() {
        for (index, selected_provider) in
            [Provider::OpenSubtitles, Provider::Assrt, Provider::Shooter]
                .into_iter()
                .enumerate()
        {
            assert_eq!(provider_index(selected_provider), index as i32);
            assert_eq!(provider(index as i32), selected_provider);
        }
        assert_eq!(provider(-1), Provider::OpenSubtitles);
    }
}
