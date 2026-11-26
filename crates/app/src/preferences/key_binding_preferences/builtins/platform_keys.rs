//! Keep distinct macOS chords usable after mapping Command to Control.

#[cfg(not(target_os = "macos"))]
pub(in super::super) fn equivalent(profile: &str, key: &str) -> Option<&'static str> {
    // Use spare chords in each preset where two or three source chords would
    // otherwise collapse. Keep the native seek, playback and playlist keys.
    match (profile, key) {
        ("VLC Default", "Ctrl+Meta+LEFT") => Some("Alt+LEFT"),
        ("VLC Default", "Ctrl+Meta+RIGHT") => Some("Alt+RIGHT"),
        ("Movist Default", "Ctrl+Meta+v") => Some("Ctrl+Alt+Shift+v"),
        ("Movist Default", "Ctrl+Alt+Meta+s") => Some("Ctrl+Shift+a"),
        ("Movist Default", "Ctrl+Meta+s") => Some("Ctrl+Shift+s"),
        ("Movist Default", "Ctrl+Meta+S") => Some("Alt+Shift+s"),
        ("Movist Default", "Ctrl++") => Some("Ctrl+Alt++"),
        ("Movist Default", "Ctrl+Alt+LEFT") => Some("Ctrl+LEFT"),
        ("Movist Default", "Ctrl+Alt+RIGHT") => Some("Ctrl+RIGHT"),
        ("Movist Default", "Ctrl+Alt+Meta+LEFT") => Some("Ctrl+Alt+Shift+LEFT"),
        ("Movist Default", "Ctrl+Alt+Meta+RIGHT") => Some("Ctrl+Alt+Shift+RIGHT"),
        ("Movist Default", "Shift+Ctrl+LEFT") => Some("Ctrl+<"),
        ("Movist Default", "Shift+Ctrl+RIGHT") => Some("Ctrl+>"),
        ("Movist Default", "Ctrl+Meta+DOWN") => Some("Ctrl+Alt+Shift+DOWN"),
        ("Movist v2 Default", "Ctrl+Meta+s") => Some("Ctrl+s"),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
pub(in super::super) fn equivalent(_: &str, _: &str) -> Option<&'static str> {
    None
}
