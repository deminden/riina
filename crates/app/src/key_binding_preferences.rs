use crate::{KeyBindingRow, PlayerWindow, PreferencesWindow};
use riina_core::key_bindings::{ActionKind, Mapping, Settings};
use slint::{ModelRc, SharedString, VecModel};

const BUILTIN_PROFILES: [&str; 5] = [
    "IINA Default",
    "mpv Default",
    "VLC Default",
    "Movist Default",
    "Movist v2 Default",
];

const IINA_DEFAULT_ROWS: [(&str, &str, ActionKind); 22] = [
    ("Shift+Meta+v", "video-panel", ActionKind::Riina),
    ("Shift+Meta+a", "audio-panel", ActionKind::Riina),
    ("Shift+Meta+s", "sub-panel", ActionKind::Riina),
    ("Ctrl+Meta+v", "cycle video", ActionKind::Mpv),
    ("Ctrl+Meta+s", "cycle sub", ActionKind::Mpv),
    ("Ctrl+Meta+a", "cycle audio", ActionKind::Mpv),
    ("SPACE", "cycle pause", ActionKind::Mpv),
    ("Meta+.", "stop", ActionKind::Mpv),
    ("RIGHT", "seek 5", ActionKind::Mpv),
    ("LEFT", "seek -5", ActionKind::Mpv),
    ("UP", "add volume 5", ActionKind::Mpv),
    ("DOWN", "add volume -5", ActionKind::Mpv),
    ("Alt+UP", "add volume 1", ActionKind::Mpv),
    ("Alt+DOWN", "add volume -1", ActionKind::Mpv),
    ("m", "cycle mute", ActionKind::Mpv),
    ("v", "cycle sub-visibility", ActionKind::Mpv),
    ("r", "add sub-pos -1", ActionKind::Mpv),
    ("R", "add sub-pos 1", ActionKind::Mpv),
    ("t", "add sub-pos 1", ActionKind::Mpv),
    (".", "frame-step", ActionKind::Mpv),
    (",", "frame-back-step", ActionKind::Mpv),
    ("f", "cycle fullscreen", ActionKind::Mpv),
];

fn builtin_mappings(name: &str) -> Vec<Mapping> {
    if name != "IINA Default" {
        return Vec::new();
    }
    IINA_DEFAULT_ROWS
        .iter()
        .map(|(key, action, kind)| Mapping {
            key: (*key).into(),
            action: (*action).into(),
            kind: *kind,
            comment: None,
        })
        .collect()
}

fn mappings_for(name: &str) -> Result<Vec<Mapping>, String> {
    if BUILTIN_PROFILES.contains(&name) {
        return Ok(builtin_mappings(name));
    }
    crate::key_binding_profiles::ProfileStore::for_current_user()
        .and_then(|store| store.load(name))
        .map_err(|error| error.to_string())
}

fn display_row(mapping: &Mapping, raw: bool) -> KeyBindingRow {
    let key = if raw {
        mapping.key.clone()
    } else {
        match mapping.key.as_str() {
            "Shift+Meta+v" => shortcut("⇧⌘V", "Ctrl⇧V"),
            "Shift+Meta+a" => shortcut("⇧⌘A", "Ctrl⇧A"),
            "Shift+Meta+s" => shortcut("⇧⌘S", "Ctrl⇧S"),
            "Ctrl+Meta+v" => shortcut("⌃⌘V", "Ctrl⌥V"),
            "Ctrl+Meta+s" => shortcut("⌃⌘S", "Ctrl⌥S"),
            "Ctrl+Meta+a" => shortcut("⌃⌘A", "Ctrl⌥A"),
            "Meta+." => shortcut("⌘.", "Ctrl."),
            "SPACE" => "Space".into(),
            "RIGHT" => "→".into(),
            "LEFT" => "←".into(),
            "UP" => "↑".into(),
            "DOWN" => "↓".into(),
            _ => mapping.key.clone(),
        }
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

fn shortcut(mac: &str, windows: &str) -> String {
    if cfg!(target_os = "macos") {
        mac.to_owned()
    } else {
        windows.to_owned()
    }
}

fn matches_query(mapping: &Mapping, raw: bool, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let row = display_row(mapping, raw);
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

fn set_rows(window: &PreferencesWindow, mappings: &[Mapping], raw: bool, query: &str) {
    let query = query.trim().to_lowercase();
    window.set_key_binding_rows(ModelRc::new(VecModel::from(
        mappings
            .iter()
            .filter(|mapping| matches_query(mapping, raw, &query))
            .map(|mapping| display_row(mapping, raw))
            .collect::<Vec<_>>(),
    )));
}

fn visible_source_indices(mappings: &[Mapping], raw: bool, query: &str) -> Vec<usize> {
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

fn selected_mapping_index(window: &PreferencesWindow, row: i32) -> Result<usize, String> {
    let profile = window.get_key_binding_current_profile().to_string();
    let mappings = mappings_for(&profile)?;
    let visible = visible_source_indices(
        &mappings,
        window.get_key_binding_display_raw_values(),
        &window.get_key_binding_search_query(),
    );
    usize::try_from(row)
        .ok()
        .and_then(|row| visible.get(row).copied())
        .ok_or_else(|| "Select a key binding first".to_owned())
}

pub(crate) fn mapping_for_row(window: &PreferencesWindow, row: i32) -> Result<Mapping, String> {
    let profile = window.get_key_binding_current_profile().to_string();
    if is_builtin_profile(&profile) {
        return Err("Built-in key binding sets are read-only".into());
    }
    let index = selected_mapping_index(window, row)?;
    mappings_for(&profile)?
        .get(index)
        .cloned()
        .ok_or_else(|| "The selected key binding no longer exists".into())
}

pub(crate) fn save_mapping(
    window: &PreferencesWindow,
    row: i32,
    key: &str,
    action: &str,
    comment: &str,
    kind_index: i32,
) -> Result<(), String> {
    let profile = window.get_key_binding_current_profile().to_string();
    if is_builtin_profile(&profile) {
        return Err("Built-in key binding sets are read-only".into());
    }
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
    let mut mappings = mappings_for(&profile)?;
    let mapping = Mapping {
        key: key.to_owned(),
        action: action.to_owned(),
        kind,
        comment: (!comment.trim().is_empty()).then(|| comment.trim().to_owned()),
    };
    if row < 0 {
        mappings.push(mapping);
    } else {
        let index = selected_mapping_index(window, row)?;
        let target = mappings
            .get_mut(index)
            .ok_or_else(|| "The selected key binding no longer exists".to_owned())?;
        *target = mapping;
    }
    crate::key_binding_profiles::ProfileStore::for_current_user()
        .and_then(|store| store.save(&profile, &mappings))
        .map_err(|error| error.to_string())?;
    set_rows(
        window,
        &mappings,
        window.get_key_binding_display_raw_values(),
        &window.get_key_binding_search_query(),
    );
    window.set_key_binding_selected_row(-1);
    Ok(())
}

pub(crate) fn remove_mapping(window: &PreferencesWindow, row: i32) -> Result<(), String> {
    let profile = window.get_key_binding_current_profile().to_string();
    if is_builtin_profile(&profile) {
        return Err("Built-in key binding sets are read-only".into());
    }
    let index = selected_mapping_index(window, row)?;
    let mut mappings = mappings_for(&profile)?;
    if index >= mappings.len() {
        return Err("The selected key binding no longer exists".into());
    }
    mappings.remove(index);
    crate::key_binding_profiles::ProfileStore::for_current_user()
        .and_then(|store| store.save(&profile, &mappings))
        .map_err(|error| error.to_string())?;
    set_rows(
        window,
        &mappings,
        window.get_key_binding_display_raw_values(),
        &window.get_key_binding_search_query(),
    );
    window.set_key_binding_selected_row(-1);
    Ok(())
}

pub(crate) fn recorded_key(
    text: &str,
    control: bool,
    alt: bool,
    shift: bool,
    meta: bool,
) -> Option<String> {
    let key = match text {
        " " | "Space" => "SPACE".to_owned(),
        "←" | "LeftArrow" | "ArrowLeft" => "LEFT".to_owned(),
        "→" | "RightArrow" | "ArrowRight" => "RIGHT".to_owned(),
        "↑" | "UpArrow" | "ArrowUp" => "UP".to_owned(),
        "↓" | "DownArrow" | "ArrowDown" => "DOWN".to_owned(),
        "Return" | "Enter" => "ENTER".to_owned(),
        "Escape" | "Esc" => "ESC".to_owned(),
        "Tab" => "TAB".to_owned(),
        "Backspace" => "BS".to_owned(),
        "Delete" => "DEL".to_owned(),
        "Control" | "Ctrl" | "Alt" | "Shift" | "Meta" | "Super" => return None,
        value
            if value.chars().count() == 1
                && value.chars().next().is_some_and(char::is_alphabetic) =>
        {
            value.to_lowercase()
        }
        value => value.to_owned(),
    };
    Some(format_modifiers(key.to_owned(), control, alt, shift, meta))
}

fn format_modifiers(key: String, control: bool, alt: bool, shift: bool, meta: bool) -> String {
    let mut output = String::new();
    for (active, modifier) in [
        (control, "Ctrl+"),
        (alt, "Alt+"),
        (shift, "Shift+"),
        (meta, "Meta+"),
    ] {
        if active {
            output.push_str(modifier);
        }
    }
    output.push_str(&key);
    output
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

pub(crate) fn install_player(app: &PlayerWindow, settings: &Settings) {
    app.set_key_binding_current_profile(settings.current_profile.clone().into());
    app.set_key_binding_use_system_media_controls(settings.use_system_media_controls);
    app.set_key_binding_display_raw_values(settings.display_raw_values);
}

pub(crate) fn current_player(app: &PlayerWindow) -> Result<Settings, String> {
    let settings = Settings {
        current_profile: app.get_key_binding_current_profile().to_string(),
        use_system_media_controls: app.get_key_binding_use_system_media_controls(),
        display_raw_values: app.get_key_binding_display_raw_values(),
    };
    settings.validate().map_err(str::to_owned)?;
    Ok(settings)
}

pub(crate) fn install_window(
    window: &PreferencesWindow,
    settings: &Settings,
) -> Result<(), String> {
    let names = profile_names(settings)?;
    window.set_key_binding_profiles(ModelRc::new(VecModel::from(
        names
            .iter()
            .cloned()
            .map(SharedString::from)
            .collect::<Vec<_>>(),
    )));
    window.set_key_binding_profile_index(
        names
            .iter()
            .position(|name| name == &settings.current_profile)
            .unwrap_or(0) as i32,
    );
    window.set_key_binding_current_profile(settings.current_profile.clone().into());
    window.set_key_binding_use_system_media_controls(settings.use_system_media_controls);
    window.set_key_binding_display_raw_values(settings.display_raw_values);
    let mappings = mappings_for(&settings.current_profile)?;
    set_rows(
        window,
        &mappings,
        settings.display_raw_values,
        &window.get_key_binding_search_query(),
    );
    Ok(())
}

pub(crate) fn current_window(window: &PreferencesWindow) -> Result<Settings, String> {
    let settings = Settings {
        current_profile: window.get_key_binding_current_profile().to_string(),
        use_system_media_controls: window.get_key_binding_use_system_media_controls(),
        display_raw_values: window.get_key_binding_display_raw_values(),
    };
    settings.validate().map_err(str::to_owned)?;
    Ok(settings)
}

pub(crate) fn select_profile(window: &PreferencesWindow, name: &str) -> Result<(), String> {
    let mappings = mappings_for(name)?;
    set_rows(
        window,
        &mappings,
        window.get_key_binding_display_raw_values(),
        &window.get_key_binding_search_query(),
    );
    window.set_key_binding_selected_row(-1);
    Ok(())
}

pub(crate) fn search(window: &PreferencesWindow, query: &str) -> Result<(), String> {
    let mappings = mappings_for(&window.get_key_binding_current_profile())?;
    set_rows(
        window,
        &mappings,
        window.get_key_binding_display_raw_values(),
        query,
    );
    window.set_key_binding_selected_row(-1);
    Ok(())
}

pub(crate) fn refresh(window: &PreferencesWindow, settings: &Settings) -> Result<(), String> {
    install_window(window, settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_iina_defaults_begin_with_the_panel_and_track_actions() {
        let mappings = builtin_mappings("IINA Default");
        let visible: Vec<_> = mappings
            .iter()
            .take(7)
            .map(|mapping| (mapping.key.as_str(), mapping.action.as_str(), mapping.kind))
            .collect();
        assert_eq!(
            visible,
            [
                ("Shift+Meta+v", "video-panel", ActionKind::Riina),
                ("Shift+Meta+a", "audio-panel", ActionKind::Riina),
                ("Shift+Meta+s", "sub-panel", ActionKind::Riina),
                ("Ctrl+Meta+v", "cycle video", ActionKind::Mpv),
                ("Ctrl+Meta+s", "cycle sub", ActionKind::Mpv),
                ("Ctrl+Meta+a", "cycle audio", ActionKind::Mpv),
                ("SPACE", "cycle pause", ActionKind::Mpv),
            ]
        );
    }

    #[test]
    fn key_binding_search_matches_visible_and_raw_values_case_insensitively() {
        let mapping = &builtin_mappings("IINA Default")[0];
        assert!(matches_query(mapping, false, "video panel"));
        assert!(matches_query(mapping, true, "meta+v"));
        assert!(!matches_query(mapping, false, "subtitle"));
    }

    #[test]
    fn filtered_rows_keep_their_source_mapping_identity() {
        let mappings = builtin_mappings("IINA Default");
        assert_eq!(visible_source_indices(&mappings, false, "cycle audio"), [5]);
        assert_eq!(visible_source_indices(&mappings, true, "META+S"), [2, 4]);
        assert_eq!(
            visible_source_indices(&mappings, false, "missing"),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn recorded_shortcuts_use_mpv_names_and_preserve_modifier_order() {
        assert_eq!(
            recorded_key(" ", false, false, false, false).as_deref(),
            Some("SPACE")
        );
        assert_eq!(
            recorded_key("ArrowLeft", true, false, true, false).as_deref(),
            Some("Ctrl+Shift+LEFT")
        );
        assert_eq!(
            recorded_key("V", false, false, true, true).as_deref(),
            Some("Shift+Meta+v")
        );
        assert_eq!(recorded_key("Control", true, false, false, false), None);
    }
}
