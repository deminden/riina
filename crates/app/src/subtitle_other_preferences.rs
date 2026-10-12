//! Keep the Subtitle > Other controls and native subtitle policy in sync.

use riina_core::subtitle_controls::{OtherPreferences, SUBTITLE_ENCODING_CHOICES};
use slint::{ModelRc, SharedString, VecModel};

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
        preferred_languages: app.get_subtitle_preferred_languages().to_string(),
        default_encoding: app.get_subtitle_default_encoding().to_string(),
    };
    preferences.validate().map_err(str::to_owned)?;
    Ok(preferences)
}

pub(crate) fn current_window(
    window: &crate::PreferencesWindow,
) -> Result<OtherPreferences, String> {
    let encoding = encoding_code(window.get_subtitle_encoding_index())
        .ok_or_else(|| "Invalid subtitle encoding selection".to_owned())?;
    let preferences = OtherPreferences {
        preferred_languages: window.get_subtitle_preferred_languages_text().to_string(),
        default_encoding: encoding.into(),
    };
    preferences.validate().map_err(str::to_owned)?;
    Ok(preferences)
}

pub(crate) fn install_player(app: &crate::PlayerWindow, preferences: &OtherPreferences) {
    app.set_subtitle_preferred_languages(preferences.preferred_languages.clone().into());
    app.set_subtitle_default_encoding(preferences.default_encoding.clone().into());
    app.set_subtitle_encoding_options(ModelRc::new(VecModel::from(
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
    window.set_subtitle_preferred_languages_text(preferences.preferred_languages.clone().into());
    let index = preferences
        .encoding_index()
        .ok_or_else(|| "Invalid subtitle encoding selection".to_owned())?;
    window.set_subtitle_encoding_index(index);
    Ok(())
}

pub(crate) fn apply(
    player: &riina_mpv::CommandSender,
    previous: &OtherPreferences,
    selected: &OtherPreferences,
) -> Result<(), String> {
    selected.validate().map_err(str::to_owned)?;
    previous.validate().map_err(str::to_owned)?;
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
            let _ = player.send(riina_core::Command::SubtitlePreferredLanguages(
                previous.preferred_languages.clone(),
            ));
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
