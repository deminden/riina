//! Keep OSD fields parsed as one draft before any visible or saved state changes.
use crate::{PlayerWindow, PreferencesWindow};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Settings {
    pub enabled: bool,
    pub disable_file_start: bool,
    pub disable_pause_resume: bool,
    pub disable_speed: bool,
    pub disable_seek: bool,
    pub timeout_secs: f64,
    pub text_size: f64,
    pub display_time_and_battery_in_fullscreen: bool,
}

pub(crate) fn current_player(window: &PlayerWindow) -> Settings {
    Settings {
        enabled: window.get_osd_enabled(),
        disable_file_start: window.get_disable_osd_file_start(),
        disable_pause_resume: window.get_disable_osd_pause_resume(),
        disable_speed: window.get_disable_osd_speed(),
        disable_seek: window.get_disable_osd_seek(),
        timeout_secs: f64::from(window.get_osd_timeout_secs()),
        text_size: f64::from(window.get_osd_text_size()),
        display_time_and_battery_in_fullscreen: window.get_display_time_and_battery_in_fullscreen(),
    }
}

pub(crate) fn current_preferences(window: &PreferencesWindow) -> Result<Settings, String> {
    let settings = Settings {
        enabled: window.get_osd_enabled(),
        disable_file_start: window.get_disable_osd_file_start(),
        disable_pause_resume: window.get_disable_osd_pause_resume(),
        disable_speed: window.get_disable_osd_speed(),
        disable_seek: window.get_disable_osd_seek(),
        timeout_secs: parse(&window.get_osd_timeout_text(), "OSD auto-hide time")?,
        text_size: parse(&window.get_osd_text_size_text(), "OSD text size")?,
        display_time_and_battery_in_fullscreen: window.get_display_time_and_battery_in_fullscreen(),
    };
    settings.validate()?;
    Ok(settings)
}

impl Settings {
    pub(crate) fn validate(self) -> Result<Self, String> {
        std::time::Duration::try_from_secs_f64(self.timeout_secs)
            .map_err(|_| "Enter a finite OSD auto-hide time")?;
        if !self.text_size.is_finite() || !(5.0..=500.0).contains(&self.text_size) {
            return Err("OSD text size must be between 5 and 500 points".into());
        }
        Ok(self)
    }

    pub(crate) fn install_player(self, window: &PlayerWindow) {
        window.set_osd_enabled(self.enabled);
        window.set_disable_osd_file_start(self.disable_file_start);
        window.set_disable_osd_pause_resume(self.disable_pause_resume);
        window.set_disable_osd_speed(self.disable_speed);
        window.set_disable_osd_seek(self.disable_seek);
        window.set_osd_timeout_secs(self.timeout_secs as f32);
        window.set_osd_text_size(self.text_size as f32);
        window.set_display_time_and_battery_in_fullscreen(
            self.display_time_and_battery_in_fullscreen,
        );
    }

    pub(crate) fn install_preferences(self, window: &PreferencesWindow) {
        window.set_osd_enabled(self.enabled);
        window.set_disable_osd_file_start(self.disable_file_start);
        window.set_disable_osd_pause_resume(self.disable_pause_resume);
        window.set_disable_osd_speed(self.disable_speed);
        window.set_disable_osd_seek(self.disable_seek);
        window.set_osd_timeout_text(format_number(self.timeout_secs).into());
        window.set_osd_text_size_text(format_number(self.text_size).into());
        window.set_display_time_and_battery_in_fullscreen(
            self.display_time_and_battery_in_fullscreen,
        );
    }
}

fn parse(text: &str, name: &str) -> Result<f64, String> {
    text.trim()
        .parse::<f64>()
        .map_err(|_| format!("Enter a numeric {name}"))
}

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{Settings, format_number, parse};

    #[test]
    fn numeric_drafts_preserve_source_defaults_and_reject_invalid_text() {
        assert_eq!(parse(" 1 ", "timeout").unwrap(), 1.0);
        assert_eq!(parse("20.5", "size").unwrap(), 20.5);
        assert_eq!(format_number(20.0), "20");
        for input in ["", "NaN", "infinity", "-1"] {
            let parsed = parse(input, "size");
            if let Ok(value) = parsed {
                assert!(
                    Settings {
                        enabled: true,
                        disable_file_start: false,
                        disable_pause_resume: false,
                        disable_speed: false,
                        disable_seek: false,
                        timeout_secs: 1.0,
                        text_size: value,
                        display_time_and_battery_in_fullscreen: false,
                    }
                    .validate()
                    .is_err()
                );
            } else {
                assert!(parsed.is_err());
            }
        }
        for invalid in [0.0, 4.99, 500.01, f64::NAN, f64::INFINITY] {
            assert!(
                Settings {
                    enabled: true,
                    disable_file_start: false,
                    disable_pause_resume: false,
                    disable_speed: false,
                    disable_seek: false,
                    timeout_secs: 1.0,
                    text_size: invalid,
                    display_time_and_battery_in_fullscreen: false,
                }
                .validate()
                .is_err()
            );
        }
    }
}
