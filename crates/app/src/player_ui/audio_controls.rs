//! Equalizer callbacks share one settings owner across all controls.
use crate::PlayerWindow;
use riina_core::Command;
use riina_mpv::CommandSender;
use slint::{ComponentHandle, Model};
use std::sync::{Arc, Mutex};

pub(super) fn install(
    app: &PlayerWindow,
    player: &CommandSender,
    audio_equalizer: Arc<Mutex<riina_core::audio_equalizer::Settings>>,
) {
    let audio_eq_settings = audio_equalizer.clone();
    let audio_eq_player = player.clone();
    let weak = app.as_weak();
    app.on_set_audio_equalizer(move |index, value| {
        let Some(index) = usize::try_from(index).ok().filter(|index| *index < 10) else {
            return;
        };
        if !value.is_finite() || !(-12.0..=12.0).contains(&value) {
            return;
        }
        let Some(app) = weak.upgrade() else { return };
        let settings = {
            let Ok(mut settings) = audio_eq_settings.lock() else {
                app.set_status("Audio equalizer settings are unavailable".into());
                return;
            };
            let values = app.get_audio_equalizer_values();
            if values.row_count() == settings.gains_db.len() {
                for (index, gain) in settings.gains_db.iter_mut().enumerate() {
                    if let Some(value) = values.row_data(index) {
                        *gain = f64::from(value);
                    }
                }
            }
            settings.gains_db[index] = f64::from(value);
            settings.selected_profile = Some("custom".into());
            settings.selected_profile_is_user = false;
            settings.clone()
        };
        if let Err(error) = audio_eq_player.send(Command::AudioEqualizer(settings.gains_db)) {
            app.set_status(error.into());
            return;
        }
        set_audio_equalizer_controls(&app, &settings);
        app.invoke_activity();
    });
    let audio_eq_settings_for_profile = audio_equalizer.clone();
    let audio_eq_player_for_profile = player.clone();
    let weak = app.as_weak();
    app.on_select_audio_equalizer_profile(move |index| {
        let Some(app) = weak.upgrade() else { return };
        let Ok(mut settings) = audio_eq_settings_for_profile.lock() else {
            app.set_status("Audio equalizer settings are unavailable".into());
            return;
        };
        let Some(index) = usize::try_from(index).ok() else {
            app.set_status("Unknown audio equalizer profile".into());
            return;
        };
        if let Err(error) = settings.select_profile_at(index) {
            app.set_status(error.into());
            return;
        }
        let snapshot = settings.clone();
        drop(settings);
        if let Err(error) =
            audio_eq_player_for_profile.send(Command::AudioEqualizer(snapshot.gains_db))
        {
            app.set_status(error.into());
            return;
        }
        set_audio_equalizer_controls(&app, &snapshot);
    });
    let audio_eq_settings_for_validation = audio_equalizer.clone();
    let weak = app.as_weak();
    app.on_validate_audio_equalizer_profile_name(move |name, mode| {
        let Some(app) = weak.upgrade() else {
            return "Audio equalizer settings are unavailable".into();
        };
        let visible_names = audio_equalizer_visible_profile_names(&app);
        if audio_equalizer_name_is_display_duplicate(name.as_str(), &visible_names) {
            return "An audio equalizer profile already has that name".into();
        }
        let Ok(settings) = audio_eq_settings_for_validation.lock() else {
            return "Audio equalizer settings are unavailable".into();
        };
        let validation = match mode {
            1 => settings.validate_new_profile_name(name.as_str()),
            2 => {
                let current = settings.selected_profile.as_deref().unwrap_or_default();
                settings.validate_renamed_profile_name(current, name.as_str())
            }
            _ => return "".into(),
        };
        validation.err().unwrap_or("").into()
    });
    let audio_eq_settings_for_save = audio_equalizer.clone();
    let weak = app.as_weak();
    app.on_save_audio_equalizer_profile(move |name| {
        let Some(app) = weak.upgrade() else {
            return false;
        };
        let visible_names = audio_equalizer_visible_profile_names(&app);
        if audio_equalizer_name_is_display_duplicate(name.as_str(), &visible_names) {
            app.set_status("An audio equalizer profile already has that name".into());
            return false;
        }
        let Ok(mut settings) = audio_eq_settings_for_save.lock() else {
            app.set_status("Audio equalizer settings are unavailable".into());
            return false;
        };
        if let Err(error) = settings.save_profile(name.as_str()) {
            app.set_status(error.into());
            return false;
        }
        set_audio_equalizer_controls(&app, &settings);
        true
    });
    let audio_eq_settings_for_rename = audio_equalizer.clone();
    let weak = app.as_weak();
    app.on_rename_audio_equalizer_profile(move |replacement| {
        let Some(app) = weak.upgrade() else {
            return false;
        };
        let visible_names = audio_equalizer_visible_profile_names(&app);
        if audio_equalizer_name_is_display_duplicate(replacement.as_str(), &visible_names) {
            app.set_status("An audio equalizer profile already has that name".into());
            return false;
        }
        let Ok(mut settings) = audio_eq_settings_for_rename.lock() else {
            app.set_status("Audio equalizer settings are unavailable".into());
            return false;
        };
        let current = settings.selected_profile.clone().unwrap_or_default();
        if let Err(error) = settings.rename_profile(&current, replacement.as_str()) {
            app.set_status(error.into());
            return false;
        }
        set_audio_equalizer_controls(&app, &settings);
        true
    });
    let audio_eq_settings_for_delete = audio_equalizer.clone();
    let weak = app.as_weak();
    app.on_delete_audio_equalizer_profile(move || {
        let Some(app) = weak.upgrade() else { return };
        let Ok(mut settings) = audio_eq_settings_for_delete.lock() else {
            app.set_status("Audio equalizer settings are unavailable".into());
            return;
        };
        let selected = settings.selected_profile.clone().unwrap_or_default();
        if let Err(error) = settings.remove_profile(&selected) {
            app.set_status(error.into());
            return;
        }
        set_audio_equalizer_controls(&app, &settings);
    });
}

pub(super) fn set_audio_equalizer_controls(
    app: &PlayerWindow,
    settings: &riina_core::audio_equalizer::Settings,
) {
    let profile_ids = settings.profile_ids();
    app.set_audio_equalizer_values(slint::ModelRc::new(slint::VecModel::from(
        settings.gains_db.map(|gain| gain as f32).to_vec(),
    )));
    app.set_audio_equalizer_profile_names(slint::ModelRc::new(slint::VecModel::from(
        profile_ids
            .iter()
            .map(|id| match id {
                riina_core::audio_equalizer::ProfileId::Manual => "Manual".to_owned(),
                riina_core::audio_equalizer::ProfileId::Preset(name) => name
                    .replace('_', " ")
                    .split_whitespace()
                    .map(|part| {
                        let mut chars = part.chars();
                        chars
                            .next()
                            .map(|first| first.to_uppercase().to_string() + chars.as_str())
                            .unwrap_or_default()
                    })
                    .collect::<Vec<_>>()
                    .join(" "),
                riina_core::audio_equalizer::ProfileId::User(name) => name.clone(),
            })
            .map(slint::SharedString::from)
            .collect::<Vec<_>>(),
    )));
    app.set_audio_equalizer_profile_index(settings.selected_profile_index().unwrap_or(0) as i32);
    app.set_audio_equalizer_user_profile_start_index(
        profile_ids
            .iter()
            .position(|id| matches!(id, riina_core::audio_equalizer::ProfileId::User(_)))
            .unwrap_or(profile_ids.len()) as i32,
    );
    app.set_audio_equalizer_can_delete_profile(settings.selected_profile_is_user());
    app.set_audio_equalizer_can_edit_profile(settings.selected_profile_is_user());
    app.set_audio_equalizer_can_save_profile(settings.is_manual_selected());
}

fn audio_equalizer_visible_profile_names(app: &PlayerWindow) -> Vec<String> {
    let names = app.get_audio_equalizer_profile_names();
    (1..names.row_count())
        .filter_map(|index| names.row_data(index))
        .map(|name| name.to_string())
        .collect()
}

fn audio_equalizer_name_is_display_duplicate(name: &str, visible_names: &[String]) -> bool {
    visible_names.iter().any(|visible| visible == name)
}

#[cfg(test)]
mod audio_equalizer_name_tests {
    use super::audio_equalizer_name_is_display_duplicate;

    #[test]
    fn duplicate_check_uses_the_visible_preset_and_user_profile_labels() {
        let visible_names = vec!["Flat".into(), "Rock".into(), "My mix".into()];
        assert!(audio_equalizer_name_is_display_duplicate(
            "Rock",
            &visible_names
        ));
        assert!(audio_equalizer_name_is_display_duplicate(
            "My mix",
            &visible_names
        ));
        assert!(!audio_equalizer_name_is_display_duplicate(
            "Manual",
            &visible_names
        ));
        assert!(!audio_equalizer_name_is_display_duplicate(
            "rock",
            &visible_names
        ));
    }
}
