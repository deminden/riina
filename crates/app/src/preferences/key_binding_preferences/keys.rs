//! Convert platform key events and profile spellings to one shortcut identity.
use slint::platform::Key;

// Key events carry Slint's encoded scalar, not the human-readable enum name.
// Keep the conversion at the event boundary; profiles and menus use mpv names.
const EVENT_KEYS: &[(Key, &str)] = &[
    (Key::Space, "SPACE"),
    (Key::LeftArrow, "LEFT"),
    (Key::RightArrow, "RIGHT"),
    (Key::UpArrow, "UP"),
    (Key::DownArrow, "DOWN"),
    (Key::Return, "ENTER"),
    (Key::Escape, "ESC"),
    (Key::Tab, "TAB"),
    (Key::Backtab, "TAB"),
    (Key::Backspace, "BS"),
    (Key::Delete, "DEL"),
    (Key::Insert, "INS"),
    (Key::Home, "HOME"),
    (Key::End, "END"),
    (Key::PageUp, "PGUP"),
    (Key::PageDown, "PGDWN"),
    (Key::F1, "F1"),
    (Key::F2, "F2"),
    (Key::F3, "F3"),
    (Key::F4, "F4"),
    (Key::F5, "F5"),
    (Key::F6, "F6"),
    (Key::F7, "F7"),
    (Key::F8, "F8"),
    (Key::F9, "F9"),
    (Key::F10, "F10"),
    (Key::F11, "F11"),
    (Key::F12, "F12"),
    (Key::F13, "F13"),
    (Key::F14, "F14"),
    (Key::F15, "F15"),
    (Key::F16, "F16"),
    (Key::F17, "F17"),
    (Key::F18, "F18"),
    (Key::F19, "F19"),
    (Key::F20, "F20"),
    (Key::F21, "F21"),
    (Key::F22, "F22"),
    (Key::F23, "F23"),
    (Key::F24, "F24"),
];

fn event_key(text: &str) -> Option<String> {
    let mut chars = text.chars();
    let first = chars.next()?;
    if chars.next().is_none() {
        if [
            Key::Control,
            Key::ControlR,
            Key::Shift,
            Key::ShiftR,
            Key::Alt,
            Key::AltGr,
            Key::Meta,
            Key::MetaR,
        ]
        .into_iter()
        .any(|key| char::from(key) == first)
        {
            return None;
        }
        if let Some((_, name)) = EVENT_KEYS.iter().find(|(key, _)| char::from(*key) == first) {
            return Some((*name).into());
        }
        if first.is_control() {
            return None;
        }
    }
    let key = match text {
        " " | "Space" => "SPACE".to_owned(),
        "←" | "LeftArrow" | "ArrowLeft" => "LEFT".to_owned(),
        "→" | "RightArrow" | "ArrowRight" => "RIGHT".to_owned(),
        "↑" | "UpArrow" | "ArrowUp" => "UP".to_owned(),
        "↓" | "DownArrow" | "ArrowDown" => "DOWN".to_owned(),
        "Return" | "Enter" => "ENTER".to_owned(),
        "Escape" | "Esc" => "ESC".to_owned(),
        "Tab" | "Backtab" => "TAB".to_owned(),
        "Backspace" => "BS".to_owned(),
        "Delete" => "DEL".to_owned(),
        "Insert" => "INS".to_owned(),
        "Home" => "HOME".to_owned(),
        "End" => "END".to_owned(),
        "PageUp" => "PGUP".to_owned(),
        "PageDown" => "PGDWN".to_owned(),
        "Control" | "ControlR" | "Ctrl" | "Alt" | "AltGr" | "Shift" | "ShiftR" | "Meta"
        | "MetaR" | "Super" => return None,
        value if value.chars().count() == 1 && first.is_alphabetic() => value.to_lowercase(),
        value => value.to_owned(),
    };
    Some(key)
}

pub(crate) fn recorded_key(
    text: &str,
    control: bool,
    alt: bool,
    shift: bool,
    meta: bool,
) -> Option<String> {
    let key = event_key(text)?;
    let shift =
        shift || text.chars().eq(std::iter::once(char::from(Key::Backtab))) || text == "Backtab";
    Some(format_modifiers(key, control, alt, shift, meta))
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

fn profile_key_parts(value: &str) -> Vec<&str> {
    if value == "+" {
        return vec!["+"];
    }
    if let Some(modifiers) = value.strip_suffix("++") {
        return modifiers.split('+').chain(std::iter::once("+")).collect();
    }
    value.split('+').collect()
}

pub(super) fn normalize_profile_key(value: &str) -> String {
    let mut control = false;
    let mut alt = false;
    let mut shift = false;
    let mut meta = false;
    let parts = profile_key_parts(value);
    let Some((key, modifiers)) = parts.split_last() else {
        return String::new();
    };
    let key = *key;
    for part in modifiers {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => control = true,
            "alt" | "option" => alt = true,
            "shift" => shift = true,
            "meta" | "super" | "command" | "cmd" => {
                meta = true;
            }
            _ => return String::new(),
        }
    }
    // IINA's Meta modifier is the platform's primary shortcut; its additional
    // Control modifier maps to Alt on Windows and Linux, matching the UI layer.
    if !cfg!(target_os = "macos") && meta {
        meta = false;
        if control {
            alt = true;
        }
        control = true;
    }
    let key = match key.to_ascii_uppercase().as_str() {
        "SPACE" => " ".to_owned(),
        "LEFT" | "LEFTARROW" => "←".to_owned(),
        "RIGHT" | "RIGHTARROW" => "→".to_owned(),
        "UP" | "UPARROW" => "↑".to_owned(),
        "DOWN" | "DOWNARROW" => "↓".to_owned(),
        "ENTER" | "RETURN" => "Return".to_owned(),
        "ESC" | "ESCAPE" => "Escape".to_owned(),
        "PGUP" | "PAGEUP" => "PageUp".to_owned(),
        "PGDWN" | "PAGEDOWN" => "PageDown".to_owned(),
        _ if key.chars().count() == 1 && key.chars().next().is_some_and(char::is_uppercase) => {
            shift = true;
            key.to_lowercase()
        }
        _ => key.to_owned(),
    };
    recorded_key(&key, control, alt, shift, meta).unwrap_or_default()
}

pub(crate) fn shortcut_keys(key: &str) -> Option<slint::Keys> {
    let normalized = normalize_profile_key(key);
    if normalized.is_empty() {
        return None;
    }
    let profile_parts = profile_key_parts(&normalized);
    let explicit_shift = profile_parts.contains(&"Shift");
    let parts = profile_parts
        .into_iter()
        .map(|part| match part {
            // Named punctuation implies Slint's Shift? policy and rejects an
            // explicit Shift modifier. Literal characters preserve the profile's
            // actual modifiers, including IINA's chapter/audio-delay bindings.
            value
                if explicit_shift
                    && value.len() == 1
                    && value.as_bytes()[0].is_ascii_punctuation() =>
            {
                value.to_owned()
            }
            "Ctrl" => "Control".to_owned(),
            " " | "SPACE" => "Space".to_owned(),
            "←" | "LEFT" => "LeftArrow".to_owned(),
            "→" | "RIGHT" => "RightArrow".to_owned(),
            "↑" | "UP" => "UpArrow".to_owned(),
            "↓" | "DOWN" => "DownArrow".to_owned(),
            "ENTER" => "Return".to_owned(),
            "ESC" => "Escape".to_owned(),
            "PGUP" => "PageUp".to_owned(),
            "PGDWN" => "PageDown".to_owned(),
            "INS" => "Insert".to_owned(),
            "HOME" => "Home".to_owned(),
            "END" => "End".to_owned(),
            "BS" => "Backspace".to_owned(),
            "DEL" => "Delete".to_owned(),
            "-" => "HyphenMinus".to_owned(),
            "=" => "Equals".to_owned(),
            "[" => "OpenBracket".to_owned(),
            "]" => "CloseBracket".to_owned(),
            "\\" => "BackSlash".to_owned(),
            "`" => "BackQuote".to_owned(),
            "," => "Comma".to_owned(),
            "." => "Period".to_owned(),
            "/" => "Slash".to_owned(),
            "<" => "LessThan".to_owned(),
            ">" => "GreaterThan".to_owned(),
            "(" => "OpenParen".to_owned(),
            ")" => "CloseParen".to_owned(),
            "+" => "Plus".to_owned(),
            "_" => "Underscore".to_owned(),
            value => value.to_owned(),
        })
        .collect::<Vec<_>>();
    slint::Keys::from_parts(parts.iter().map(String::as_str)).ok()
}
