//! Captured default profile data and user-profile loading.
use super::keys::normalize_profile_key;
use riina_core::key_bindings::{ActionKind, Mapping};

mod alternate;
mod mpv;
mod platform_keys;

pub(super) use platform_keys::equivalent as platform_equivalent;
type Row = (&'static str, &'static str, ActionKind);

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

pub(super) const SYSTEM_MEDIA_ROWS: [(&str, &str); 15] = [
    ("POWER", "quit"),
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
    ("CLOSE_WIN", "quit"),
];

pub(super) fn builtin_mappings(name: &str) -> Vec<Mapping> {
    let rows = match name {
        "IINA Default" => IINA_DEFAULT_ROWS.as_slice(),
        "mpv Default" => mpv::BEFORE_MEDIA,
        "VLC Default" => alternate::VLC,
        "Movist Default" => alternate::MOVIST,
        "Movist v2 Default" => alternate::MOVIST_V2,
        _ => return Vec::new(),
    };
    let mut mappings: Vec<_> = rows
        .iter()
        .map(|&(key, action, kind)| mapping(key, action, kind))
        .collect();
    let alternate = name != "IINA Default";
    mappings.extend(SYSTEM_MEDIA_ROWS.map(|(key, default)| {
        let action = match (alternate, key) {
            (true, "STOP") => "quit",
            (true, "FORWARD") => "seek 60",
            (true, "REWIND") => "seek -60",
            _ => default,
        };
        mapping(key, action, ActionKind::Mpv)
    }));
    if name == "mpv Default" {
        mappings.extend(
            mpv::AFTER_MEDIA
                .iter()
                .map(|&(key, action, kind)| mapping(key, action, kind)),
        );
    }
    mappings
}

fn mapping(key: &str, action: &str, kind: ActionKind) -> Mapping {
    Mapping {
        key: key.into(),
        action: action.into(),
        kind,
        comment: None,
    }
}

pub(crate) fn mappings_for(name: &str) -> Result<Vec<Mapping>, String> {
    let mut mappings = editable_mappings_for(name)?;
    if BUILTIN_PROFILES.contains(&name) {
        return Ok(mappings);
    }
    for (key, action) in SYSTEM_MEDIA_ROWS {
        if !mappings
            .iter()
            .any(|mapping| normalize_profile_key(&mapping.key) == key)
        {
            mappings.push(mapping(key, action, ActionKind::Mpv));
        }
    }
    Ok(mappings)
}

pub(crate) fn editable_mappings_for(name: &str) -> Result<Vec<Mapping>, String> {
    let mut rows = stored_mappings_for(name)?;
    for row in &mut rows {
        if let Some(chord) = platform_equivalent(name, &row.key) {
            row.key = chord.into();
        }
    }
    Ok(rows)
}

pub(super) fn stored_mappings_for(name: &str) -> Result<Vec<Mapping>, String> {
    if BUILTIN_PROFILES.contains(&name) {
        Ok(builtin_mappings(name))
    } else {
        crate::key_binding_profiles::ProfileStore::for_current_user()
            .and_then(|store| store.load(name))
            .map_err(|error| error.to_string())
    }
}
