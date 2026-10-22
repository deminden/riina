//! Captured default profile data and user-profile loading.
use super::keys::normalize_profile_key;
use riina_core::key_bindings::{ActionKind, Mapping};

pub(super) const BUILTIN_PROFILES: [&str; 5] = [
    "IINA Default",
    "mpv Default",
    "VLC Default",
    "Movist Default",
    "Movist v2 Default",
];

const IINA_DEFAULT_ROWS: [(&str, &str, ActionKind); 76] = [
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
    ("Alt+RIGHT", "frame-step", ActionKind::Mpv),
    ("Alt+LEFT", "frame-back-step", ActionKind::Mpv),
    ("Shift+LEFT", "sub-seek -1", ActionKind::Mpv),
    ("Shift+RIGHT", "sub-seek 1", ActionKind::Mpv),
    ("UP", "add volume 5", ActionKind::Mpv),
    ("DOWN", "add volume -5", ActionKind::Mpv),
    ("Alt+UP", "add volume 1", ActionKind::Mpv),
    ("Alt+DOWN", "add volume -1", ActionKind::Mpv),
    ("Meta+/", "cycle mute", ActionKind::Mpv),
    ("m", "cycle mute", ActionKind::Mpv),
    ("v", "cycle sub-visibility", ActionKind::Mpv),
    ("r", "add sub-pos -1", ActionKind::Mpv),
    ("R", "add sub-pos +1", ActionKind::Mpv),
    ("t", "add sub-pos +1", ActionKind::Mpv),
    (".", "frame-step", ActionKind::Mpv),
    (",", "frame-back-step", ActionKind::Mpv),
    ("Shift+Meta+p", "playlist-panel", ActionKind::Riina),
    ("Shift+Meta+c", "chapter-panel", ActionKind::Riina),
    ("Meta+3", "fit-to-screen", ActionKind::Riina),
    ("Ctrl+Meta+p", "toggle-pip", ActionKind::Riina),
    ("Alt+Meta+m", "toggle-music-mode", ActionKind::Riina),
    ("Meta+D", "find-online-subs", ActionKind::Riina),
    (
        "Shift+Meta+r",
        "show-current-file-in-finder",
        ActionKind::Riina,
    ),
    ("Shift+Meta+t", "live-text", ActionKind::Riina),
    ("Meta+s", "screenshot", ActionKind::Mpv),
    ("Meta+RIGHT", "playlist-next", ActionKind::Mpv),
    ("Meta+LEFT", "playlist-prev", ActionKind::Mpv),
    ("Shift+Meta+>", "add chapter 1", ActionKind::Mpv),
    ("Shift+Meta+<", "add chapter -1", ActionKind::Mpv),
    ("Meta+[", "multiply speed 0.5", ActionKind::Mpv),
    ("Meta+]", "multiply speed 2.0", ActionKind::Mpv),
    ("Alt+Meta+[", "multiply speed 1/1.1", ActionKind::Mpv),
    ("Alt+Meta+]", "multiply speed 1.1", ActionKind::Mpv),
    ("Meta+\\", "set speed 1.0", ActionKind::Mpv),
    ("Meta+0", "set window-scale 0.5", ActionKind::Mpv),
    ("Meta+1", "set window-scale 1", ActionKind::Mpv),
    ("Meta+2", "set window-scale 2", ActionKind::Mpv),
    ("Meta+-", "smaller-window", ActionKind::Riina),
    ("Meta+=", "bigger-window", ActionKind::Riina),
    ("Ctrl+Meta+f", "cycle fullscreen", ActionKind::Mpv),
    ("f", "cycle fullscreen", ActionKind::Mpv),
    ("Ctrl+Meta+t", "cycle ontop", ActionKind::Mpv),
    ("Shift+(", "add audio-delay 0.5", ActionKind::Mpv),
    ("Shift+)", "add audio-delay -0.5", ActionKind::Mpv),
    ("Alt+Shift+(", "add audio-delay 0.1", ActionKind::Mpv),
    ("Alt+Shift+)", "add audio-delay -0.1", ActionKind::Mpv),
    ("Shift+_", "set audio-delay 0", ActionKind::Mpv),
    ("Z", "add sub-delay -0.5", ActionKind::Mpv),
    ("X", "add sub-delay 0.5", ActionKind::Mpv),
    ("Alt+Z", "add sub-delay -0.1", ActionKind::Mpv),
    ("Alt+X", "add sub-delay 0.1", ActionKind::Mpv),
    ("C", "set sub-delay 0", ActionKind::Mpv),
    ("ESC", "set fullscreen no", ActionKind::Mpv),
    ("ENTER", "set fullscreen yes", ActionKind::Mpv),
    ("Meta+l", "ab-loop", ActionKind::Mpv),
    (
        "Meta+L",
        "cycle-values loop \"inf\" \"no\"",
        ActionKind::Mpv,
    ),
    ("q", "quit", ActionKind::Mpv),
    ("p", "cycle pause", ActionKind::Mpv),
    ("o", "show-progress", ActionKind::Mpv),
    ("P", "show-progress", ActionKind::Mpv),
    ("Shift+PGUP", "seek 600", ActionKind::Mpv),
    ("Shift+PGDWN", "seek -600", ActionKind::Mpv),
    ("G", "add sub-scale +0.1", ActionKind::Mpv),
    ("F", "add sub-scale -0.1", ActionKind::Mpv),
    ("Alt+v", "cycle secondary-sub-visibility", ActionKind::Mpv),
    ("E", "cycle edition", ActionKind::Mpv),
];

const MPV_DEFAULT_ROWS: [(&str, &str, ActionKind); 27] = [
    ("RIGHT", "seek 5", ActionKind::Mpv),
    ("LEFT", "seek -5", ActionKind::Mpv),
    ("UP", "seek 60", ActionKind::Mpv),
    ("DOWN", "seek -60", ActionKind::Mpv),
    ("Shift+RIGHT", "seek 1 exact", ActionKind::Mpv),
    ("Shift+LEFT", "seek -1 exact", ActionKind::Mpv),
    ("Shift+UP", "seek 5 exact", ActionKind::Mpv),
    ("Shift+DOWN", "seek -5 exact", ActionKind::Mpv),
    ("Ctrl+LEFT", "sub-seek -1", ActionKind::Mpv),
    ("Ctrl+RIGHT", "sub-seek 1", ActionKind::Mpv),
    ("SPACE", "cycle pause", ActionKind::Mpv),
    ("p", "cycle pause", ActionKind::Mpv),
    ("f", "cycle fullscreen", ActionKind::Mpv),
    ("m", "cycle mute", ActionKind::Mpv),
    ("v", "cycle sub-visibility", ActionKind::Mpv),
    ("j", "cycle sub", ActionKind::Mpv),
    ("Alt+RIGHT", "frame-step", ActionKind::Mpv),
    ("Alt+LEFT", "frame-back-step", ActionKind::Mpv),
    (".", "frame-step", ActionKind::Mpv),
    (",", "frame-back-step", ActionKind::Mpv),
    ("9", "add volume -2", ActionKind::Mpv),
    ("0", "add volume 2", ActionKind::Mpv),
    ("[", "multiply speed 0.9091", ActionKind::Mpv),
    ("]", "multiply speed 1.1", ActionKind::Mpv),
    ("{", "multiply speed 0.5", ActionKind::Mpv),
    ("}", "multiply speed 2", ActionKind::Mpv),
    ("l", "ab-loop", ActionKind::Mpv),
];

const VLC_DEFAULT_ROWS: [(&str, &str, ActionKind); 19] = [
    ("Meta+f", "cycle fullscreen", ActionKind::Mpv),
    ("ESC", "set fullscreen no", ActionKind::Mpv),
    ("SPACE", "cycle pause", ActionKind::Mpv),
    ("Meta+=", "multiply speed 1.5", ActionKind::Mpv),
    ("Meta+-", "multiply speed 0.6667", ActionKind::Mpv),
    ("Meta+RIGHT", "playlist-next", ActionKind::Mpv),
    ("Meta+LEFT", "playlist-prev", ActionKind::Mpv),
    ("Meta+.", "stop", ActionKind::Mpv),
    ("Ctrl+Meta+LEFT", "seek -3", ActionKind::Mpv),
    ("Ctrl+Meta+RIGHT", "seek 3", ActionKind::Mpv),
    ("Alt+Meta+LEFT", "seek -10", ActionKind::Mpv),
    ("Alt+Meta+RIGHT", "seek 10", ActionKind::Mpv),
    ("Shift+Meta+LEFT", "seek -60", ActionKind::Mpv),
    ("Shift+Meta+RIGHT", "seek 60", ActionKind::Mpv),
    ("Shift+Alt+Meta+LEFT", "seek -300", ActionKind::Mpv),
    ("Shift+Alt+Meta+RIGHT", "seek 300", ActionKind::Mpv),
    ("e", "frame-step", ActionKind::Mpv),
    ("Meta+l", "ab-loop", ActionKind::Mpv),
    ("m", "cycle mute", ActionKind::Mpv),
];

const MOVIST_DEFAULT_ROWS: [(&str, &str, ActionKind); 21] = [
    ("Alt+Meta+RIGHT", "playlist-next", ActionKind::Mpv),
    ("Alt+Meta+LEFT", "playlist-prev", ActionKind::Mpv),
    ("Alt+Meta+s", "playlist-shuffle", ActionKind::Mpv),
    ("SPACE", "cycle pause", ActionKind::Mpv),
    ("LEFT", "seek -10", ActionKind::Mpv),
    ("Alt+LEFT", "seek -60", ActionKind::Mpv),
    ("Shift+Alt+LEFT", "seek -300", ActionKind::Mpv),
    ("RIGHT", "seek 10", ActionKind::Mpv),
    ("Alt+RIGHT", "seek 60", ActionKind::Mpv),
    ("Shift+Alt+RIGHT", "seek 300", ActionKind::Mpv),
    ("Ctrl+Alt+LEFT", "cycle sub down", ActionKind::Mpv),
    ("Ctrl+Alt+RIGHT", "cycle sub up", ActionKind::Mpv),
    ("Meta+|", "set speed 1", ActionKind::Mpv),
    ("Shift+Meta+LEFT", "multiply speed 0.9", ActionKind::Mpv),
    ("Shift+Meta+RIGHT", "multiply speed 1.1", ActionKind::Mpv),
    ("z", "set speed 1", ActionKind::Mpv),
    ("x", "multiply speed 0.9", ActionKind::Mpv),
    ("c", "multiply speed 1.1", ActionKind::Mpv),
    ("f", "cycle fullscreen", ActionKind::Mpv),
    ("m", "cycle mute", ActionKind::Mpv),
    ("Meta+.", "stop", ActionKind::Mpv),
];

const MOVIST_V2_DEFAULT_ROWS: [(&str, &str, ActionKind); 24] = [
    ("Meta+RIGHT", "playlist-next", ActionKind::Mpv),
    ("Meta+LEFT", "playlist-prev", ActionKind::Mpv),
    ("Alt+Meta+s", "playlist-shuffle", ActionKind::Mpv),
    ("Alt+Meta+r", "cycle-values loop inf no", ActionKind::Mpv),
    ("Ctrl+Meta+v", "cycle video up", ActionKind::Mpv),
    ("Ctrl+Meta+V", "cycle video down", ActionKind::Mpv),
    ("SPACE", "cycle pause", ActionKind::Mpv),
    ("LEFT", "seek -5", ActionKind::Mpv),
    ("Alt+LEFT", "seek -30", ActionKind::Mpv),
    ("Shift+Alt+LEFT", "seek -300", ActionKind::Mpv),
    ("RIGHT", "seek 5", ActionKind::Mpv),
    ("Alt+RIGHT", "seek 30", ActionKind::Mpv),
    ("Shift+Alt+RIGHT", "seek 300", ActionKind::Mpv),
    (",", "frame-back-step", ActionKind::Mpv),
    (".", "frame-step", ActionKind::Mpv),
    ("Alt+1", "set speed 1", ActionKind::Mpv),
    ("Alt+2", "set speed 2", ActionKind::Mpv),
    ("Alt+4", "set speed 4", ActionKind::Mpv),
    ("m", "cycle mute", ActionKind::Mpv),
    ("f", "cycle fullscreen", ActionKind::Mpv),
    ("v", "cycle sub-visibility", ActionKind::Mpv),
    ("j", "cycle sub", ActionKind::Mpv),
    ("Meta+.", "stop", ActionKind::Mpv),
    ("Meta+s", "screenshot", ActionKind::Mpv),
];

pub(super) const SYSTEM_MEDIA_ROWS: [(&str, &str); 15] = [
    ("PLAY", "cycle pause"),
    ("PAUSE", "cycle pause"),
    ("PLAYPAUSE", "cycle pause"),
    ("PLAYONLY", "set pause no"),
    ("PAUSEONLY", "set pause yes"),
    ("STOP", "stop"),
    ("FORWARD", "seek 15"),
    ("REWIND", "seek -15"),
    ("NEXT", "playlist-next"),
    ("PREV", "playlist-prev"),
    ("VOLUME_UP", "add volume 2"),
    ("VOLUME_DOWN", "add volume -2"),
    ("MUTE", "cycle mute"),
    ("POWER", "quit"),
    ("CLOSE_WIN", "quit"),
];

pub(super) fn builtin_mappings(name: &str) -> Vec<Mapping> {
    let rows = match name {
        "IINA Default" => &IINA_DEFAULT_ROWS[..],
        "mpv Default" => &MPV_DEFAULT_ROWS[..],
        "VLC Default" => &VLC_DEFAULT_ROWS[..],
        "Movist Default" => &MOVIST_DEFAULT_ROWS[..],
        "Movist v2 Default" => &MOVIST_V2_DEFAULT_ROWS[..],
        _ => return Vec::new(),
    };
    rows.iter()
        .map(|(key, action, kind)| Mapping {
            key: (*key).into(),
            action: (*action).into(),
            kind: *kind,
            comment: None,
        })
        .collect()
}

pub(crate) fn mappings_for(name: &str) -> Result<Vec<Mapping>, String> {
    let mut mappings = if BUILTIN_PROFILES.contains(&name) {
        builtin_mappings(name)
    } else {
        crate::key_binding_profiles::ProfileStore::for_current_user()
            .and_then(|store| store.load(name))
            .map_err(|error| error.to_string())?
    };
    for (key, action) in SYSTEM_MEDIA_ROWS {
        if !mappings
            .iter()
            .any(|mapping| normalize_profile_key(&mapping.key) == key)
        {
            mappings.push(Mapping {
                key: key.into(),
                action: action.into(),
                kind: ActionKind::Mpv,
                comment: None,
            });
        }
    }
    Ok(mappings)
}
