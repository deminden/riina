//! Shared policy for when the on-screen controller fades after interaction.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub auto_hide_enabled: bool,
    pub auto_hide_timeout_secs: f64,
}

impl Settings {
    pub fn validate(self) -> Result<Self, &'static str> {
        std::time::Duration::try_from_secs_f64(self.auto_hide_timeout_secs)
            .map_err(|_| "The control-bar auto-hide time must be a finite nonnegative duration")?;
        Ok(self)
    }
}

impl Default for Settings {
    fn default() -> Self {
        // IINA defaults to showing the controller for 2.5 seconds after activity.
        Self {
            auto_hide_enabled: true,
            auto_hide_timeout_secs: 2.5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Settings;

    #[test]
    fn defaults_and_validation_match_iina_auto_hide_policy() {
        let defaults = Settings::default();
        assert!(defaults.auto_hide_enabled);
        assert_eq!(defaults.auto_hide_timeout_secs, 2.5);
        assert!(defaults.validate().is_ok());

        for timeout in [0.0, 0.25, 2.5] {
            assert!(
                Settings {
                    auto_hide_timeout_secs: timeout,
                    ..defaults
                }
                .validate()
                .is_ok()
            );
        }
        for timeout in [-0.1, f64::NAN, f64::INFINITY] {
            assert!(
                Settings {
                    auto_hide_timeout_secs: timeout,
                    ..defaults
                }
                .validate()
                .is_err()
            );
        }
    }
}
