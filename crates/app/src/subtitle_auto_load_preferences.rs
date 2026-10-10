//! Keep the Auto Load controls aligned with the persisted shared preference.

use riina_core::subtitle_controls::{AutoLoadMode, AutoLoadPreferences, SUBTITLE_AUTO_LOAD_MODES};
use slint::{ModelRc, SharedString, VecModel};

pub(crate) fn mode_options() -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        SUBTITLE_AUTO_LOAD_MODES
            .iter()
            .map(|label| SharedString::from(*label))
            .collect::<Vec<_>>(),
    ))
}

pub(crate) fn current_player(app: &crate::PlayerWindow) -> Result<AutoLoadPreferences, String> {
    let mode = AutoLoadMode::from_index(app.get_subtitle_auto_load_mode_index())
        .ok_or_else(|| "Invalid subtitle auto-load mode".to_owned())?;
    let preferences = AutoLoadPreferences {
        mode,
        priority_keywords: app.get_subtitle_auto_load_priority().to_string(),
        search_paths: app.get_subtitle_search_paths().to_string(),
    };
    preferences.validate().map_err(str::to_owned)?;
    Ok(preferences)
}

pub(crate) fn current_window(
    window: &crate::PreferencesWindow,
) -> Result<AutoLoadPreferences, String> {
    let mode = AutoLoadMode::from_index(window.get_subtitle_auto_load_mode_index())
        .ok_or_else(|| "Invalid subtitle auto-load mode".to_owned())?;
    let preferences = AutoLoadPreferences {
        mode,
        priority_keywords: window.get_subtitle_auto_load_priority_text().to_string(),
        search_paths: window.get_subtitle_search_paths_text().to_string(),
    };
    preferences.validate().map_err(str::to_owned)?;
    Ok(preferences)
}

pub(crate) fn install_player(app: &crate::PlayerWindow, preferences: &AutoLoadPreferences) {
    app.set_subtitle_auto_load_mode_index(preferences.mode.index());
    app.set_subtitle_auto_load_priority(preferences.priority_keywords.clone().into());
    app.set_subtitle_search_paths(preferences.search_paths.clone().into());
}

pub(crate) fn install_window(window: &crate::PreferencesWindow, preferences: &AutoLoadPreferences) {
    window.set_subtitle_auto_load_mode_index(preferences.mode.index());
    window.set_subtitle_auto_load_priority_text(preferences.priority_keywords.clone().into());
    window.set_subtitle_search_paths_text(preferences.search_paths.clone().into());
}

#[cfg(test)]
mod tests {
    use super::mode_options;
    use riina_core::subtitle_controls::SUBTITLE_AUTO_LOAD_MODES;
    use slint::Model;

    #[test]
    fn mode_menu_matches_the_reference_order() {
        let options = mode_options();
        assert_eq!(options.row_count(), SUBTITLE_AUTO_LOAD_MODES.len());
        for (index, label) in SUBTITLE_AUTO_LOAD_MODES.iter().enumerate() {
            assert_eq!(options.row_data(index), Some((*label).into()));
        }
    }
}
