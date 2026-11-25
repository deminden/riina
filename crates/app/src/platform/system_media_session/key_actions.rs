//! Project the active key profile into desktop media actions.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use crate::{
    PlayerWindow,
    key_binding_preferences::{Bindings, dispatch_mapping, mappings_for},
};
#[cfg(any(target_os = "macos", target_os = "windows", test))]
use riina_core::key_bindings::{ActionKind, Mapping};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use slint::ComponentHandle;

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub(in crate::platform) fn dispatch_system_media_key(
    app: &PlayerWindow,
    key: &str,
    latest_playback: &crate::PlaybackStore,
) -> bool {
    if !app
        .global::<riina_ui::PlayerPreferences>()
        .get_key_binding_use_system_media_controls()
    {
        return false;
    }
    let Ok(mappings) = mappings_for(
        &app.global::<riina_ui::PlayerPreferences>()
            .get_key_binding_current_profile(),
    ) else {
        return false;
    };
    let mappings = Bindings::new(mappings);
    let Some(mapping) = mappings.get(key) else {
        return false;
    };
    dispatch_mapping(app, mapping, latest_playback);
    true
}

#[cfg(any(target_os = "macos", target_os = "windows", test))]
pub(crate) fn preferred_system_skip_interval(mapping: Option<&Mapping>) -> Option<f64> {
    let Some(mapping) = mapping else {
        return Some(15.0);
    };
    if mapping.kind != ActionKind::Mpv {
        return None;
    }
    let commands = riina_core::key_bindings::mpv_action_commands(&mapping.action).ok()?;
    if commands.len() != 1 || commands[0].first().map(String::as_str) != Some("seek") {
        return None;
    }
    let command = &commands[0];
    let target = command.get(1)?.parse::<f64>().ok()?;
    if !target.is_finite() {
        return None;
    }
    if command.get(2).is_some_and(|flags| {
        !flags
            .split('+')
            .all(|flag| matches!(flag, "exact" | "keyframes" | "relative"))
    }) {
        return None;
    }
    Some(target.abs())
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) fn system_skip_intervals_for_profile(profile: &str) -> (Option<f64>, Option<f64>) {
    let Ok(mappings) = mappings_for(profile) else {
        return (None, None);
    };
    let mappings = Bindings::new(mappings);
    let interval_for = |key| preferred_system_skip_interval(mappings.get(key));
    (interval_for("FORWARD"), interval_for("REWIND"))
}
