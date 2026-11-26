//! Profile shortcuts for the two player menu surfaces.
use super::bindings::Bindings;
use super::keys::shortcut_keys;
use crate::MenuShortcutRow;
use riina_core::key_bindings::ActionKind;

// This order is shared with the `menu-shortcut(index)` calls in the two player
// menu surfaces. Keep it stable so profile changes update the same actions in
// both the shared-renderer and native-video windows.
pub(super) const MENU_SHORTCUT_ACTIONS: [(ActionKind, &str); 61] = [
    (ActionKind::Mpv, "cycle pause"),
    (ActionKind::Mpv, "stop"),
    (ActionKind::Mpv, "seek 5"),
    (ActionKind::Mpv, "seek -5"),
    (ActionKind::Mpv, "frame-step"),
    (ActionKind::Mpv, "frame-back-step"),
    (ActionKind::Mpv, "set speed 1.0"),
    (ActionKind::Mpv, "multiply speed 2.0"),
    (ActionKind::Mpv, "multiply speed 0.5"),
    (ActionKind::Mpv, "multiply speed 1.1"),
    (ActionKind::Mpv, "multiply speed 1/1.1"),
    (ActionKind::Mpv, "screenshot"),
    (ActionKind::Riina, "playlist-panel"),
    (ActionKind::Mpv, "playlist-next"),
    (ActionKind::Mpv, "playlist-prev"),
    (ActionKind::Riina, "chapter-panel"),
    (ActionKind::Mpv, "add chapter 1"),
    (ActionKind::Mpv, "add chapter -1"),
    (ActionKind::Riina, "video-panel"),
    (ActionKind::Mpv, "cycle video"),
    (ActionKind::Mpv, "set window-scale 0.5"),
    (ActionKind::Mpv, "set window-scale 1"),
    (ActionKind::Mpv, "set window-scale 2"),
    (ActionKind::Riina, "fit-to-screen"),
    (ActionKind::Riina, "bigger-window"),
    (ActionKind::Riina, "smaller-window"),
    (ActionKind::Riina, "toggle-pip"),
    (ActionKind::Mpv, "cycle fullscreen"),
    (ActionKind::Mpv, "cycle ontop"),
    (ActionKind::Riina, "live-text"),
    (ActionKind::Riina, "toggle-music-mode"),
    (ActionKind::Riina, "audio-panel"),
    (ActionKind::Mpv, "cycle audio"),
    (ActionKind::Mpv, "add volume 5"),
    (ActionKind::Mpv, "add volume -5"),
    (ActionKind::Mpv, "add volume 1"),
    (ActionKind::Mpv, "add volume -1"),
    (ActionKind::Mpv, "cycle mute"),
    (ActionKind::Mpv, "add audio-delay 0.5"),
    (ActionKind::Mpv, "add audio-delay -0.5"),
    (ActionKind::Mpv, "add audio-delay 0.1"),
    (ActionKind::Mpv, "add audio-delay -0.1"),
    (ActionKind::Riina, "sub-panel"),
    (ActionKind::Mpv, "cycle sub"),
    (ActionKind::Mpv, "cycle sub-visibility"),
    (ActionKind::Riina, "plugin-panel"),
    (ActionKind::Mpv, "ab-loop"),
    (ActionKind::Mpv, "cycle-values loop \"inf\" \"no\""),
    (ActionKind::Mpv, "set audio-delay 0"),
    (ActionKind::Mpv, "add sub-delay 0.5"),
    (ActionKind::Mpv, "add sub-delay -0.5"),
    (ActionKind::Mpv, "set sub-delay 0"),
    (ActionKind::Mpv, "add sub-delay 0.1"),
    (ActionKind::Mpv, "add sub-delay -0.1"),
    (ActionKind::Mpv, "cycle secondary-sub-visibility"),
    (ActionKind::Riina, "find-online-subs"),
    (ActionKind::Riina, "show-current-file-in-finder"),
    (ActionKind::Riina, "save-playlist"),
    (ActionKind::Riina, "toggle-mirror"),
    (ActionKind::Riina, "toggle-flip"),
    (ActionKind::Riina, "save-downloaded-sub"),
];

pub(crate) fn menu_shortcut_rows(mappings: &Bindings) -> Vec<MenuShortcutRow> {
    MENU_SHORTCUT_ACTIONS
        .iter()
        .map(|(kind, action)| {
            // Prefer the first effective alias, after duplicate-key precedence.
            // An overridden entry must not install a conflicting menu accelerator.
            let shortcut = mappings
                .effective()
                .filter(|mapping| {
                    mapping.kind == *kind && menu_action_matches(action, mapping.action.trim())
                })
                .find_map(|mapping| shortcut_keys(&mapping.key))
                .unwrap_or_default();
            MenuShortcutRow { shortcut }
        })
        .collect()
}

fn menu_action_matches(expected: &str, candidate: &str) -> bool {
    if expected == candidate {
        return true;
    }
    if matches!(expected, "cycle audio" | "cycle sub" | "cycle video") {
        return candidate.strip_suffix(" up") == Some(expected);
    }
    if expected == "cycle-values loop \"inf\" \"no\"" {
        return matches!(
            candidate,
            "cycle-values loop-file \"inf\" \"no\"" | "cycle-values loop inf no"
        );
    }
    let Some((expected_command, expected_value)) = expected.rsplit_once(' ') else {
        return false;
    };
    candidate
        .strip_prefix(expected_command)
        .and_then(|suffix| suffix.strip_prefix(' '))
        .and_then(|value| value.parse::<f64>().ok())
        .is_some_and(|value| expected_value.parse::<f64>() == Ok(value))
}
