//! Preferences rows and edits use one loaded profile per operation.
use super::builtins::{BUILTIN_PROFILES, platform_equivalent, stored_mappings_for};
use super::keys::shortcut_keys;
use crate::{KeyBindingRow, PlayerWindow, PreferencesWindow, key_binding_profiles::ProfileStore};
use riina_core::key_bindings::{ActionKind, Mapping, Settings};
use slint::ComponentHandle;
use slint::{ModelRc, SharedString, VecModel};

pub(super) fn display_row(mapping: &Mapping, raw: bool) -> KeyBindingRow {
    let key = if raw {
        mapping.key.clone()
    } else if cfg!(target_os = "macos") {
        match mapping.key.as_str() {
            "Shift+Meta+v" => "⇧⌘V".into(),
            "Shift+Meta+a" => "⇧⌘A".into(),
            "Shift+Meta+s" => "⇧⌘S".into(),
            "Ctrl+Meta+v" => "⌃⌘V".into(),
            "Ctrl+Meta+s" => "⌃⌘S".into(),
            "Ctrl+Meta+a" => "⌃⌘A".into(),
            "Meta+." => "⌘.".into(),
            "Meta+/" => "⌘/".into(),
            "Meta+D" => "⇧⌘D".into(),
            "SPACE" => "Space".into(),
            "RIGHT" => "→".into(),
            "LEFT" => "←".into(),
            "UP" => "↑".into(),
            "DOWN" => "↓".into(),
            _ => mapping.key.clone(),
        }
    } else {
        shortcut_keys(&mapping.key)
            .map(|keys| keys.to_string())
            .unwrap_or_else(|| mapping.key.clone())
    };
    let action = if raw {
        mapping.action.clone()
    } else {
        match mapping.action.as_str() {
            "cycle pause" => "Cycle Pause".into(),
            "seek 5" => "Seek forward 5 seconds".into(),
            "seek -5" => "Seek backward 5 seconds".into(),
            "add volume 5" => "Increase volume".into(),
            "add volume -5" => "Decrease volume".into(),
            "cycle mute" => "Toggle mute".into(),
            "cycle sub-visibility" => "Toggle subtitles".into(),
            "cycle fullscreen" => "Toggle full screen".into(),
            "video-panel" => "Video panel".into(),
            "audio-panel" => "Audio panel".into(),
            "sub-panel" => "Subtitle panel".into(),
            "playlist-panel" => "Playlist panel".into(),
            "chapter-panel" => "Chapter panel".into(),
            "fit-to-screen" => "Fit window to video".into(),
            "toggle-pip" => "Toggle Picture-in-Picture".into(),
            "toggle-music-mode" => "Toggle Music Mode".into(),
            "show-current-file-in-finder" => "Show current file in file manager".into(),
            "open-file" => "Open file".into(),
            "open-url" => "Open URL".into(),
            "save-playlist" => "Save current playlist".into(),
            "save-downloaded-sub" => "Save downloaded subtitle".into(),
            "delete-current-file" => format!(
                "Move current file to the {}",
                crate::platform::system_trash_name()
            ),
            "delete-current-file-hard" => "Permanently delete current file".into(),
            "toggle-flip" => "Flip vertically".into(),
            "toggle-mirror" => "Mirror horizontally".into(),
            "live-text" => "Toggle Live Text".into(),
            "cycle video" => "Cycle Video track".into(),
            "cycle sub" => "Cycle Subtitle track".into(),
            "cycle audio" => "Cycle Audio track".into(),
            "stop" => "Stop".into(),
            "add sub-pos -1" => "Move subtitles up".into(),
            "add sub-pos 1" => "Move subtitles down".into(),
            "frame-step" => "Next frame".into(),
            "frame-back-step" => "Previous frame".into(),
            _ => mapping.action.clone(),
        }
    };
    KeyBindingRow {
        key: key.into(),
        action: action.into(),
        comment: mapping.comment.clone().unwrap_or_default().into(),
    }
}

pub(super) fn matches_query(mapping: &Mapping, raw: bool, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let row = display_row(mapping, raw);
    row_matches_query(mapping, &row, query)
}

fn row_matches_query(mapping: &Mapping, row: &KeyBindingRow, query: &str) -> bool {
    [
        row.key.as_str(),
        row.action.as_str(),
        mapping.key.as_str(),
        mapping.action.as_str(),
        mapping.comment.as_deref().unwrap_or_default(),
    ]
    .into_iter()
    .any(|part| part.to_lowercase().contains(query))
}

fn set_rows(
    window: &PreferencesWindow,
    profile: &str,
    mappings: &[Mapping],
    raw: bool,
    query: &str,
) {
    let query = query.trim().to_lowercase();
    window
        .global::<riina_ui::KeyBindingSettingsState>()
        .set_key_binding_rows(ModelRc::new(VecModel::from(
            mappings
                .iter()
                .filter_map(|mapping| {
                    let mut row = display_row(mapping, raw);
                    if !raw
                        && let Some(chord) = platform_equivalent(profile, &mapping.key)
                        && let Some(keys) = shortcut_keys(chord)
                    {
                        row.key = keys.to_string().into();
                    }
                    (query.is_empty() || row_matches_query(mapping, &row, &query)).then_some(row)
                })
                .collect::<Vec<_>>(),
        )));
}

pub(super) fn visible_source_indices(mappings: &[Mapping], raw: bool, query: &str) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    mappings
        .iter()
        .enumerate()
        .filter_map(|(index, mapping)| matches_query(mapping, raw, &query).then_some(index))
        .collect()
}

fn is_builtin_profile(name: &str) -> bool {
    BUILTIN_PROFILES.contains(&name)
}

fn selected_mapping_index(
    window: &PreferencesWindow,
    mappings: &[Mapping],
    row: i32,
) -> Result<usize, String> {
    let visible = visible_source_indices(
        mappings,
        window
            .global::<riina_ui::KeyBindingSettingsState>()
            .get_key_binding_display_raw_values(),
        &window
            .global::<riina_ui::KeyBindingSettingsState>()
            .get_key_binding_search_query(),
    );
    usize::try_from(row)
        .ok()
        .and_then(|row| visible.get(row).copied())
        .ok_or_else(|| "Select a key binding first".to_owned())
}

fn profile_name(window: &PreferencesWindow) -> String {
    window
        .global::<riina_ui::KeyBindingSettingsState>()
        .get_key_binding_current_profile()
        .to_string()
}

pub(crate) struct MappingDraft {
    profile: String,
    file: Vec<Mapping>,
    index: Option<usize>,
}

impl MappingDraft {
    pub(crate) fn open(window: &PreferencesWindow, row: Option<i32>) -> Result<Self, String> {
        let profile = profile_name(window);
        if is_builtin_profile(&profile) {
            return Err("Built-in key binding sets are read-only".into());
        }
        let file = stored_mappings_for(&profile)?;
        let index = row
            .map(|row| selected_mapping_index(window, &file, row))
            .transpose()?;
        Ok(Self {
            profile,
            file,
            index,
        })
    }

    pub(crate) fn original(&self) -> Option<&Mapping> {
        self.index.and_then(|index| self.file.get(index))
    }

    pub(crate) fn save(
        &self,
        window: &PreferencesWindow,
        key: &str,
        action: &str,
        comment: &str,
        kind_index: i32,
    ) -> Result<(), String> {
        let key = key.trim();
        let action = action.trim();
        if key.is_empty() || action.is_empty() {
            return Err("Enter both a key and an action".into());
        }
        let kind = match kind_index {
            0 => ActionKind::Mpv,
            1 => ActionKind::Riina,
            _ => return Err("Choose a valid action type".into()),
        };
        let mapping = Mapping {
            key: key.to_owned(),
            action: action.to_owned(),
            kind,
            comment: (!comment.trim().is_empty()).then(|| comment.trim().to_owned()),
        };
        self.commit(window, |file| {
            if let Some(index) = self.index {
                file[index] = mapping;
            } else {
                file.push(mapping);
            }
        })
    }

    fn commit(
        &self,
        window: &PreferencesWindow,
        edit: impl FnOnce(&mut Vec<Mapping>),
    ) -> Result<(), String> {
        if profile_name(window) != self.profile {
            return Err(
                "The active key binding set changed. Reopen the editor to continue.".into(),
            );
        }
        let mut updated = self.file.clone();
        edit(&mut updated);
        ProfileStore::for_current_user()
            .and_then(|store| store.replace_mappings(&self.profile, &self.file, &updated))
            .map_err(|error| error.to_string())?;
        project_rows(window, &self.profile, &updated);
        window
            .global::<riina_ui::KeyBindingSettingsState>()
            .invoke_profile_file_changed(self.profile.as_str().into());
        Ok(())
    }
}

pub(crate) fn remove_mapping(window: &PreferencesWindow, row: i32) -> Result<(), String> {
    let draft = MappingDraft::open(window, Some(row))?;
    let Some(index) = draft.index else {
        return Err("Select a key binding first".into());
    };
    draft.commit(window, |file| {
        file.remove(index);
    })
}

fn project_rows(window: &PreferencesWindow, profile: &str, mappings: &[Mapping]) {
    let state = window.global::<riina_ui::KeyBindingSettingsState>();
    set_rows(
        window,
        profile,
        mappings,
        state.get_key_binding_display_raw_values(),
        &state.get_key_binding_search_query(),
    );
    state.set_key_binding_selected_row(-1);
}

fn profile_names(settings: &Settings) -> Result<Vec<String>, String> {
    let mut names = BUILTIN_PROFILES.map(str::to_owned).to_vec();
    let profiles = crate::key_binding_profiles::ProfileStore::for_current_user()
        .map_err(|error| error.to_string())?
        .list()
        .map_err(|error| error.to_string())?;
    names.extend(profiles.into_iter().map(|profile| profile.name));
    if !names.contains(&settings.current_profile) {
        names.push(settings.current_profile.clone());
    }
    Ok(names)
}

pub(crate) fn current_player(app: &PlayerWindow) -> Result<Settings, String> {
    let player_preferences = app.global::<riina_ui::PlayerPreferences>();
    let settings = Settings {
        current_profile: player_preferences
            .get_key_binding_current_profile()
            .to_string(),
        use_system_media_controls: player_preferences.get_key_binding_use_system_media_controls(),
        display_raw_values: player_preferences.get_key_binding_display_raw_values(),
    };
    settings.validate().map_err(|error| error.to_string())?;
    Ok(settings)
}

pub(crate) fn install_window(
    window: &PreferencesWindow,
    settings: &Settings,
) -> Result<(), String> {
    let names = profile_names(settings)?;
    window
        .global::<riina_ui::KeyBindingSettingsState>()
        .set_key_binding_profiles(ModelRc::new(VecModel::from(
            names
                .iter()
                .cloned()
                .map(SharedString::from)
                .collect::<Vec<_>>(),
        )));
    window
        .global::<riina_ui::KeyBindingSettingsState>()
        .set_key_binding_profile_index(
            names
                .iter()
                .position(|name| name == &settings.current_profile)
                .unwrap_or(0) as i32,
        );
    window
        .global::<riina_ui::KeyBindingSettingsState>()
        .set_key_binding_current_profile(settings.current_profile.clone().into());
    window
        .global::<riina_ui::KeyBindingSettingsState>()
        .set_key_binding_use_system_media_controls(settings.use_system_media_controls);
    window
        .global::<riina_ui::KeyBindingSettingsState>()
        .set_key_binding_display_raw_values(settings.display_raw_values);
    let mappings = stored_mappings_for(&settings.current_profile)?;
    set_rows(
        window,
        &settings.current_profile,
        &mappings,
        settings.display_raw_values,
        &window
            .global::<riina_ui::KeyBindingSettingsState>()
            .get_key_binding_search_query(),
    );
    Ok(())
}

pub(crate) fn current_window(window: &PreferencesWindow) -> Result<Settings, String> {
    let settings = Settings {
        current_profile: window
            .global::<riina_ui::KeyBindingSettingsState>()
            .get_key_binding_current_profile()
            .to_string(),
        use_system_media_controls: window
            .global::<riina_ui::KeyBindingSettingsState>()
            .get_key_binding_use_system_media_controls(),
        display_raw_values: window
            .global::<riina_ui::KeyBindingSettingsState>()
            .get_key_binding_display_raw_values(),
    };
    settings.validate().map_err(|error| error.to_string())?;
    Ok(settings)
}

pub(crate) fn select_profile(window: &PreferencesWindow, name: &str) -> Result<(), String> {
    project_rows(window, name, &stored_mappings_for(name)?);
    Ok(())
}

pub(crate) fn search(window: &PreferencesWindow, query: &str) -> Result<(), String> {
    let profile = profile_name(window);
    let mappings = stored_mappings_for(&profile)?;
    let form = window.global::<riina_ui::KeyBindingSettingsState>();
    set_rows(
        window,
        &profile,
        &mappings,
        form.get_key_binding_display_raw_values(),
        query,
    );
    form.set_key_binding_selected_row(-1);
    Ok(())
}

pub(crate) fn refresh(window: &PreferencesWindow, settings: &Settings) -> Result<(), String> {
    install_window(window, settings)
}

pub(crate) fn activate_profile(window: &PreferencesWindow, name: &str) -> Result<(), String> {
    let mut settings = current_window(window)?;
    settings.current_profile = name.to_owned();
    refresh(window, &settings)?;
    window.invoke_settings_changed();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_draft_does_not_follow_the_form_to_a_different_profile() {
        i_slint_backend_testing::init_no_event_loop();
        let window = PreferencesWindow::new().unwrap();
        let state = window.global::<riina_ui::KeyBindingSettingsState>();
        state.set_key_binding_current_profile("Another set".into());
        let draft = MappingDraft {
            profile: "Original set".into(),
            file: Vec::new(),
            index: None,
        };
        let result = draft.save(&window, "F8", "open-url", "", 1);
        assert!(result.unwrap_err().contains("set changed"));
        assert_eq!(state.get_key_binding_current_profile(), "Another set");
    }
}
