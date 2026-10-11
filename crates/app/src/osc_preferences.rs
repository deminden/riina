//! Translate editable controller settings into the shared validated policy.
use crate::{PlayerWindow, PreferencesWindow};
use riina_core::osc_preferences::Settings;

pub(crate) fn current_player(window: &PlayerWindow) -> Settings {
    Settings {
        auto_hide_enabled: window.get_control_auto_hide_enabled(),
        auto_hide_timeout_secs: f64::from(window.get_control_auto_hide_timeout_secs()),
    }
}

pub(crate) fn current_preferences(window: &PreferencesWindow) -> Result<Settings, String> {
    let timeout_secs = parse_timeout(window.get_control_auto_hide_timeout_text().as_str())?;
    Settings {
        auto_hide_enabled: window.get_control_auto_hide_enabled(),
        auto_hide_timeout_secs: timeout_secs,
    }
    .validate()
    .map_err(str::to_owned)
}

pub(crate) fn install_player(window: &PlayerWindow, settings: Settings) {
    window.set_control_auto_hide_enabled(settings.auto_hide_enabled);
    window.set_control_auto_hide_timeout_secs(settings.auto_hide_timeout_secs as f32);
}

pub(crate) fn install_preferences(window: &PreferencesWindow, settings: Settings) {
    window.set_control_auto_hide_enabled(settings.auto_hide_enabled);
    window
        .set_control_auto_hide_timeout_text(format_timeout(settings.auto_hide_timeout_secs).into());
}

fn format_timeout(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

fn parse_timeout(text: &str) -> Result<f64, String> {
    text.trim()
        .parse::<f64>()
        .map_err(|_| "Enter a numeric control-bar auto-hide time".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{format_timeout, parse_timeout};
    use riina_core::osc_preferences::Settings;

    #[test]
    fn timeout_editor_preserves_fractional_seconds_and_rejects_bad_text() {
        assert_eq!(parse_timeout(" 2.5 ").unwrap(), 2.5);
        assert_eq!(format_timeout(3.0), "3");
        assert_eq!(format_timeout(2.5), "2.5");
        for input in ["", "two"] {
            assert!(parse_timeout(input).is_err());
        }
        for input in ["-1", "NaN", "infinity"] {
            let timeout = parse_timeout(input).unwrap();
            assert!(
                Settings {
                    auto_hide_timeout_secs: timeout,
                    ..Settings::default()
                }
                .validate()
                .is_err()
            );
        }
    }
}
