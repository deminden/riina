//! Keep the Subtitle > Other controls and native subtitle policy in sync.
use slint::ComponentHandle;

use super::model::{PreferenceSection, SectionError};
use crate::{PlayerWindow, PreferencesWindow};
use riina_core::Preferences;
use riina_core::subtitle_controls::{OtherPreferences, SUBTITLE_ENCODING_CHOICES};
use slint::{ModelRc, SharedString, VecModel};

impl PreferenceSection for OtherPreferences {
    fn get(all: &Preferences) -> Self {
        all.subtitle_other.clone()
    }

    fn put(self, all: &mut Preferences) {
        all.subtitle_other = self;
    }

    fn read_window(window: &PreferencesWindow) -> Result<Self, SectionError> {
        current_window(window).map_err(SectionError::from)
    }

    fn write_window(&self, window: &PreferencesWindow) {
        if let Err(error) = install_window(window, self) {
            crate::diagnostics::warn!("Could not refresh the subtitle language settings: {error}");
        }
    }

    fn write_player(&self, app: &PlayerWindow) {
        install_player(app, self);
    }
}

pub(crate) fn encoding_options() -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        SUBTITLE_ENCODING_CHOICES
            .iter()
            .map(|(label, _)| SharedString::from(*label))
            .collect::<Vec<_>>(),
    ))
}

fn encoding_code(index: i32) -> Option<&'static str> {
    usize::try_from(index)
        .ok()
        .and_then(|index| SUBTITLE_ENCODING_CHOICES.get(index))
        .map(|(_, code)| *code)
}

pub(crate) fn current_player(app: &crate::PlayerWindow) -> Result<OtherPreferences, String> {
    let preferences = OtherPreferences {
        preferred_languages: app
            .global::<riina_ui::PlayerPreferences>()
            .get_subtitle_preferred_languages()
            .to_string(),
        default_encoding: app
            .global::<riina_ui::PlayerPreferences>()
            .get_subtitle_default_encoding()
            .to_string(),
    };
    preferences.validate().map_err(|error| error.to_string())?;
    Ok(preferences)
}

pub(crate) fn current_window(
    window: &crate::PreferencesWindow,
) -> Result<OtherPreferences, String> {
    let encoding = encoding_code(
        window
            .global::<crate::SubtitleSettingsState>()
            .get_subtitle_encoding_index(),
    )
    .ok_or_else(|| "Invalid subtitle encoding selection".to_owned())?;
    let preferences = OtherPreferences {
        preferred_languages: window
            .global::<crate::SubtitleSettingsState>()
            .get_subtitle_preferred_languages_text()
            .to_string(),
        default_encoding: encoding.into(),
    };
    preferences.validate().map_err(|error| error.to_string())?;
    Ok(preferences)
}

pub(crate) fn install_player(app: &crate::PlayerWindow, preferences: &OtherPreferences) {
    app.global::<riina_ui::PlayerPreferences>()
        .set_subtitle_preferred_languages(preferences.preferred_languages.clone().into());
    app.global::<riina_ui::PlayerPreferences>()
        .set_subtitle_default_encoding(preferences.default_encoding.clone().into());
    app.global::<riina_ui::PlaybackSubtitleState>()
        .set_subtitle_encoding_options(ModelRc::new(VecModel::from(
            SUBTITLE_ENCODING_CHOICES
                .iter()
                .map(|(label, code)| crate::CatalogRow {
                    token: (*code).into(),
                    label: (*label).into(),
                    detail: String::new().into(),
                    path: String::new().into(),
                    language: String::new().into(),
                    selected: *code == preferences.default_encoding,
                    playing: false,
                    network: false,
                })
                .collect::<Vec<_>>(),
        )));
}

pub(crate) fn install_window(
    window: &crate::PreferencesWindow,
    preferences: &OtherPreferences,
) -> Result<(), String> {
    window
        .global::<crate::SubtitleSettingsState>()
        .set_subtitle_preferred_languages_text(preferences.preferred_languages.clone().into());
    crate::language_token_preferences::install_subtitle(window, &preferences.preferred_languages);
    let index = preferences
        .encoding_index()
        .ok_or_else(|| "Invalid subtitle encoding selection".to_owned())?;
    window
        .global::<crate::SubtitleSettingsState>()
        .set_subtitle_encoding_index(index);
    Ok(())
}

pub(crate) fn apply(
    player: &riina_mpv::CommandSender,
    previous: &OtherPreferences,
    selected: &OtherPreferences,
) -> Result<(), String> {
    selected.validate().map_err(|error| error.to_string())?;
    previous.validate().map_err(|error| error.to_string())?;
    let languages_changed = selected.preferred_languages != previous.preferred_languages;
    if languages_changed
        && let Err(error) = player.send(riina_core::Command::SubtitlePreferredLanguages(
            selected.preferred_languages.clone(),
        ))
    {
        return Err(error.to_string());
    }
    if selected.default_encoding != previous.default_encoding
        && let Err(error) = player.send(riina_core::Command::SubtitleEncoding(
            selected.default_encoding.clone(),
        ))
    {
        if languages_changed {
            crate::preferences::restore_playback(
                player,
                riina_core::Command::SubtitlePreferredLanguages(
                    previous.preferred_languages.clone(),
                ),
            );
        }
        return Err(error.to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{encoding_code, encoding_options};
    use riina_core::subtitle_controls::SUBTITLE_ENCODING_CHOICES;
    use slint::Model;

    #[test]
    fn encoding_menu_matches_the_pinned_order_and_round_trips_every_item() {
        let options = encoding_options();
        assert_eq!(options.row_count(), SUBTITLE_ENCODING_CHOICES.len());
        for (index, (title, code)) in SUBTITLE_ENCODING_CHOICES.iter().enumerate() {
            assert_eq!(options.row_data(index), Some((*title).into()));
            assert_eq!(encoding_code(index as i32), Some(*code));
        }
        assert_eq!(encoding_code(-1), None);
        assert_eq!(encoding_code(SUBTITLE_ENCODING_CHOICES.len() as i32), None);
    }
}
