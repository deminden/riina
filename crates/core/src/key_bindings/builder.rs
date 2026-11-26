//! Compile one command from the shortcut editor's selected controls.
use super::mpv_action_commands;

pub fn build_action(command: &str, selection: &str, modifier: &str, value: &str) -> Option<String> {
    let value = value.trim();
    if selection.is_empty()
        || !selection
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || matches!(byte, b'-' | b'+'))
        || value.contains(['\0', '\r', '\n', '#'])
    {
        return None;
    }
    let action = match command {
        "action" => return Some(selection.to_owned()),
        "seek" => {
            let amount = finite(value)?.abs();
            let amount = if modifier == "backward" {
                -amount
            } else {
                amount
            };
            if selection == "relative" {
                format!("seek {amount}")
            } else {
                format!("seek {amount} {selection}")
            }
        }
        "set"
            if matches!(
                selection,
                "pause" | "mute" | "fullscreen" | "ontop" | "deinterlace" | "sub-visibility"
            ) =>
        {
            if !matches!(modifier, "yes" | "no") {
                return None;
            }
            format!("set {selection} {modifier}")
        }
        "set" if selection == "video-aspect-override" => {
            if value.is_empty() {
                return None;
            }
            let quoted = value.replace('\\', "\\\\").replace('"', "\\\"");
            format!("set {selection} \"{quoted}\"")
        }
        "set" | "add" | "multiply" => {
            format!("{command} {selection} {}", finite(value)?)
        }
        "cycle" => format!("cycle {selection}"),
        "cycle-values" if !value.is_empty() => format!("cycle-values {selection} {value}"),
        _ => return None,
    };
    let parsed = mpv_action_commands(&action).ok()?;
    if parsed.len() != 1 || (command == "cycle-values" && parsed[0].len() < 4) {
        return None;
    }
    Some(action)
}

fn finite(value: &str) -> Option<f64> {
    value
        .parse::<f64>()
        .ok()
        .filter(|amount| amount.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guided_actions_generate_a_single_reusable_command() {
        for (verb, selection, modifier, value, expected) in [
            ("seek", "relative", "", "5", "seek 5"),
            (
                "seek",
                "relative-percent+exact",
                "backward",
                "12.5",
                "seek -12.5 relative-percent+exact",
            ),
            ("set", "pause", "yes", "", "set pause yes"),
            (
                "set",
                "video-aspect-override",
                "",
                "2.35:1",
                "set video-aspect-override \"2.35:1\"",
            ),
            ("add", "volume", "", "-5", "add volume -5"),
            (
                "cycle-values",
                "speed",
                "",
                "1 1.5 2",
                "cycle-values speed 1 1.5 2",
            ),
            ("action", "save-playlist", "", "", "save-playlist"),
        ] {
            assert_eq!(
                build_action(verb, selection, modifier, value).as_deref(),
                Some(expected)
            );
        }
        for value in ["", "NaN", "Infinity", "10; stop", "10 # trailing comment"] {
            assert!(build_action("seek", "absolute", "", value).is_none());
        }
        assert!(build_action("cycle-values", "speed", "", "1; stop").is_none());
        assert!(build_action("cycle-values", "speed", "", "1").is_none());
        assert!(build_action("action", "stop;quit", "", "").is_none());
    }
}
