//! Persist the opt-in mpv controls separately from Riina's playback defaults.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Verbose,
    #[default]
    Debug,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MpvOption {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub enabled: bool,
    pub logging_enabled: bool,
    pub log_level: LogLevel,
    pub use_mpv_osd: bool,
    pub use_config_directory: bool,
    pub config_directory: String,
    pub user_options: Vec<MpvOption>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            logging_enabled: false,
            log_level: LogLevel::Debug,
            use_mpv_osd: false,
            use_config_directory: false,
            config_directory: "~/.config/mpv/".into(),
            user_options: Vec::new(),
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if self.config_directory.contains('\0')
            || (self.use_config_directory && self.config_directory.trim().is_empty())
        {
            return Err("The mpv configuration directory is invalid".into());
        }
        if self.user_options.len() > 256
            || self.user_options.iter().any(|option| {
                option.name.trim().is_empty()
                    || option.name.len() > 256
                    || option.value.len() > 4096
                    || option.name.contains('\0')
                    || option.value.contains('\0')
            })
        {
            return Err("An additional mpv option is invalid".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{LogLevel, MpvOption, Settings};

    #[test]
    fn advanced_options_keep_reference_defaults_and_validate_entries() {
        let defaults = Settings::default();
        assert!(!defaults.enabled);
        assert!(!defaults.logging_enabled);
        assert_eq!(defaults.log_level, LogLevel::Debug);
        assert!(!defaults.use_mpv_osd);
        assert!(!defaults.use_config_directory);
        assert_eq!(defaults.config_directory, "~/.config/mpv/");
        assert!(defaults.user_options.is_empty());
        defaults.validate().unwrap();

        let selected = Settings {
            enabled: true,
            user_options: vec![MpvOption {
                name: "profile".into(),
                value: "fast".into(),
            }],
            ..defaults
        };
        selected.validate().unwrap();

        Settings {
            user_options: vec![MpvOption {
                name: "force-window".into(),
                value: String::new(),
            }],
            ..Settings::default()
        }
        .validate()
        .unwrap();

        for invalid in [
            MpvOption {
                name: " ".into(),
                value: "fast".into(),
            },
            MpvOption {
                name: "profile\0x".into(),
                value: "fast".into(),
            },
        ] {
            let settings = Settings {
                user_options: vec![invalid],
                ..Settings::default()
            };
            assert!(settings.validate().is_err());
        }
    }

    #[test]
    fn config_directory_is_required_only_when_enabled() {
        let mut settings = Settings {
            config_directory: " ".into(),
            ..Settings::default()
        };
        settings.validate().unwrap();
        settings.use_config_directory = true;
        assert!(settings.validate().is_err());
    }
}
