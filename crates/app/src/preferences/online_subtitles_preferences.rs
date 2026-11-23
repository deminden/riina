use super::model::{PreferenceSection, SectionError};
use crate::{PlayerWindow, PreferencesWindow};
use riina_core::online_subtitles::{Preferences, Provider};
use riina_core::plugin::SubtitleProvider;
use riina_ui::MenuCatalogRow;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

impl PreferenceSection for Preferences {
    fn get(all: &riina_core::Preferences) -> Self {
        all.online_subtitles.clone()
    }

    fn put(self, all: &mut riina_core::Preferences) {
        all.online_subtitles = self;
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
    let player_preferences = app.global::<riina_ui::PlayerPreferences>();
    let preferences = Preferences {
        provider: provider(player_preferences.get_open_subtitles_provider_index()),
        plugin_provider: optional_key(player_preferences.get_open_subtitles_plugin_provider()),
        api_key: player_preferences.get_open_subtitles_api_key().to_string(),
        assrt_token: player_preferences.get_assrt_token().to_string(),
        auto_search: player_preferences.get_open_subtitles_auto_search(),
        auto_search_threshold_minutes: player_preferences
            .get_open_subtitles_threshold_text()
            .trim()
            .parse()
            .map_err(|_| {
                "Online subtitle threshold must be a whole number of minutes".to_owned()
            })?,
    };
    preferences.validate().map_err(|error| error.to_string())?;
    Ok(preferences)
}

pub(crate) fn current_window(window: &crate::PreferencesWindow) -> Result<Preferences, String> {
    let settings = window.global::<crate::SubtitleSettingsState>();
    let index = settings.get_open_subtitles_provider_index();
    let preferences = Preferences {
        provider: provider(index),
        plugin_provider: selected_plugin(&settings, index),
        api_key: window
            .global::<crate::SubtitleSettingsState>()
            .get_open_subtitles_api_key()
            .to_string(),
        assrt_token: window
            .global::<crate::SubtitleSettingsState>()
            .get_assrt_token()
            .to_string(),
        auto_search: window
            .global::<crate::SubtitleSettingsState>()
            .get_open_subtitles_auto_search(),
        auto_search_threshold_minutes: window
            .global::<crate::SubtitleSettingsState>()
            .get_open_subtitles_threshold_text()
            .trim()
            .parse()
            .map_err(|_| {
                "Online subtitle threshold must be a whole number of minutes".to_owned()
            })?,
    };
    preferences.validate().map_err(|error| error.to_string())?;
    Ok(preferences)
}

pub(crate) fn install_player(app: &crate::PlayerWindow, preferences: &Preferences) {
    let player_preferences = app.global::<riina_ui::PlayerPreferences>();
    player_preferences.set_open_subtitles_provider_index(provider_index(preferences.provider));
    player_preferences.set_open_subtitles_plugin_provider(
        preferences
            .plugin_provider
            .clone()
            .unwrap_or_default()
            .into(),
    );
    refresh_player_label(app);
    player_preferences.set_open_subtitles_api_key(preferences.api_key.clone().into());
    player_preferences.set_assrt_token(preferences.assrt_token.clone().into());
    player_preferences.set_open_subtitles_auto_search(preferences.auto_search);
    player_preferences.set_open_subtitles_threshold_text(
        preferences.auto_search_threshold_minutes.to_string().into(),
    );
}

pub(crate) fn install_window(window: &crate::PreferencesWindow, preferences: &Preferences) {
    install_provider_options(
        window,
        preferences.provider,
        preferences.plugin_provider.as_deref(),
    );
    let settings = window.global::<crate::SubtitleSettingsState>();
    settings.set_open_subtitles_api_key(preferences.api_key.clone().into());
    settings.set_assrt_token(preferences.assrt_token.clone().into());
    settings.set_open_subtitles_auto_search(preferences.auto_search);
    settings.set_open_subtitles_threshold_text(
        preferences.auto_search_threshold_minutes.to_string().into(),
    );
}

fn install_provider_options(window: &PreferencesWindow, provider: Provider, key: Option<&str>) {
    let settings = window.global::<crate::SubtitleSettingsState>();
    let mut rows = settings
        .get_open_subtitles_plugin_options()
        .iter()
        .collect::<Vec<_>>();
    let index = key.map_or_else(
        || provider_index(provider),
        |key| {
            let position = rows
                .iter()
                .position(|row| row.token.as_str() == key)
                .unwrap_or_else(|| {
                    let index = rows.len();
                    rows.push(MenuCatalogRow {
                        token: key.into(),
                        label: "Unavailable plugin provider".into(),
                        selected: false,
                    });
                    index
                });
            i32::try_from(position + 3).unwrap_or_default()
        },
    );
    let mut labels = ["OpenSubtitles", "Assrt.net", "Shooter.cn"]
        .map(SharedString::from)
        .to_vec();
    labels.extend(rows.iter().map(|row| row.label.clone()));
    settings.set_open_subtitles_plugin_options(ModelRc::new(VecModel::from(rows)));
    settings.set_open_subtitles_provider_options(ModelRc::new(VecModel::from(labels)));
    settings.set_open_subtitles_provider_index(index);
}

pub(crate) fn refresh_catalog(window: &PreferencesWindow, app: &PlayerWindow) {
    let settings = window.global::<crate::SubtitleSettingsState>();
    let index = settings.get_open_subtitles_provider_index();
    let key = selected_plugin(&settings, index);
    install_window_catalog(window, app);
    install_provider_options(window, provider(index), key.as_deref());
}

pub(crate) fn install_window_catalog(window: &PreferencesWindow, app: &PlayerWindow) {
    window
        .global::<crate::SubtitleSettingsState>()
        .set_open_subtitles_plugin_options(
            app.global::<riina_ui::PlayerPreferences>()
                .get_plugin_subtitle_providers(),
        );
}

pub(crate) fn replace_plugin_providers(
    app: &PlayerWindow,
    identifier: &str,
    providers: Vec<SubtitleProvider>,
) {
    let prefs = app.global::<riina_ui::PlayerPreferences>();
    let prefix = format!("plugin:{identifier}:");
    let mut rows = prefs
        .get_plugin_subtitle_providers()
        .iter()
        .filter(|row| !row.token.starts_with(&prefix))
        .collect::<Vec<_>>();
    rows.extend(providers.into_iter().map(|provider| MenuCatalogRow {
        token: format!("{prefix}{}", provider.id).into(),
        label: provider.name.into(),
        selected: false,
    }));
    prefs.set_plugin_subtitle_providers(ModelRc::new(VecModel::from(rows)));
    refresh_player_label(app);
    app.invoke_plugin_subtitle_catalog_changed();
}

pub(crate) fn plugin_label(app: &PlayerWindow, key: &str) -> Option<SharedString> {
    app.global::<riina_ui::PlayerPreferences>()
        .get_plugin_subtitle_providers()
        .iter()
        .find(|row| row.token.as_str() == key)
        .map(|row| row.label)
}

fn refresh_player_label(app: &PlayerWindow) {
    let prefs = app.global::<riina_ui::PlayerPreferences>();
    prefs.set_open_subtitles_plugin_provider_label(
        plugin_label(app, prefs.get_open_subtitles_plugin_provider().as_str()).unwrap_or_default(),
    );
}

fn optional_key(key: SharedString) -> Option<String> {
    (!key.is_empty()).then(|| key.to_string())
}

fn selected_plugin(settings: &crate::SubtitleSettingsState<'_>, index: i32) -> Option<String> {
    let index = usize::try_from(index.checked_sub(3)?).ok()?;
    optional_key(
        settings
            .get_open_subtitles_plugin_options()
            .row_data(index)?
            .token,
    )
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
