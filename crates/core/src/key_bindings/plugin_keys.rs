//! Canonical keys for the IINA plugin API, independent of platform remapping.

pub fn normalize_mpv_key(value: &str) -> String {
    if value == "default-bindings" || value.bytes().filter(|&byte| byte == b'-').count() > 1 {
        return value.to_owned();
    }
    let (modifiers, key) = if value == "+" {
        ("", "+")
    } else if let Some(modifiers) = value.strip_suffix("++") {
        (modifiers, "+")
    } else {
        value.rsplit_once('+').unwrap_or(("", value))
    };
    let mut mask = [false; 4];
    let names = ["Ctrl", "Alt", "Shift", "Meta"];
    for part in modifiers.split('+') {
        if let Some(index) = names
            .iter()
            .position(|name| part.eq_ignore_ascii_case(name))
        {
            mask[index] = true;
        }
    }
    let mut key = canonical_name(key);
    if mask[2]
        && let Some(shifted) = shifted_key(&key)
    {
        key = shifted;
        mask[2] = false;
    }
    let mut result = String::new();
    for (active, modifier) in mask.into_iter().zip(names) {
        if active {
            result.push_str(modifier);
            result.push('+');
        }
    }
    result.push_str(&key);
    result
}

fn canonical_name(key: &str) -> String {
    match key {
        "#" => "SHARP".into(),
        "+" => "PLUS".into(),
        key if key.chars().count() == 1 => key.into(),
        key => key.to_uppercase(),
    }
}

fn shifted_key(key: &str) -> Option<String> {
    const BASE: &str = "`1234567890-=[]\\;',./";
    const SHIFTED: &str = "~!@#$%^&*()_+{}|:\"<>?";
    if key.len() == 1 && key.as_bytes()[0].is_ascii_alphabetic() {
        return Some(key.to_ascii_uppercase());
    }
    if let Some(index) = BASE.find(key).filter(|_| key.len() == 1) {
        return Some(canonical_name(&SHIFTED[index..=index]));
    }
    (matches!(key, "SHARP" | "PLUS") || (key.len() == 1 && SHIFTED.contains(key)))
        .then(|| key.to_owned())
}

#[cfg(test)]
mod tests {
    use super::normalize_mpv_key;

    #[test]
    fn plugin_normalization_preserves_punctuation_named_keys_and_sequences() {
        for (raw, expected) in [
            ("metA+shift+ctrl+k", "Ctrl+Meta+K"),
            ("Shift+-", "_"),
            ("Shift+=", "PLUS"),
            ("Ctrl++", "Ctrl+PLUS"),
            ("#", "SHARP"),
            ("Shift+esc", "Shift+ESC"),
            ("Meta+Alt+X", "Alt+Meta+X"),
            ("ctrl+shift+shift+x", "Ctrl+X"),
            ("a-b-c", "a-b-c"),
            ("default-bindings", "default-bindings"),
        ] {
            assert_eq!(normalize_mpv_key(raw), expected);
        }
    }
}
