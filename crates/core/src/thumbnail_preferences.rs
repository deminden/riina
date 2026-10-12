//! Thumbnail generation preferences shared by the player and settings UI.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub enabled: bool,
    pub maximum_cache_size_mb: u64,
    pub enable_for_remote_files: bool,
    pub width: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            maximum_cache_size_mb: 500,
            enable_for_remote_files: false,
            width: 120,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.maximum_cache_size_mb > i64::MAX as u64 {
            return Err("Thumbnail cache size is out of range");
        }
        if !(1..=2000).contains(&self.width) {
            return Err("Thumbnail width must be between 1 and 2000 points");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Settings;

    #[test]
    fn defaults_match_iina_thumbnail_preferences() {
        assert_eq!(
            Settings::default(),
            Settings {
                enabled: true,
                maximum_cache_size_mb: 500,
                enable_for_remote_files: false,
                width: 120,
            }
        );
    }

    #[test]
    fn preferences_round_trip_and_reject_out_of_range_values() {
        let settings = Settings {
            enabled: false,
            maximum_cache_size_mb: 0,
            enable_for_remote_files: true,
            width: 320,
        };
        let text = serde_json::to_string(&settings).unwrap();
        assert_eq!(serde_json::from_str::<Settings>(&text).unwrap(), settings);
        assert!(settings.validate().is_ok());
        assert!(
            Settings {
                width: 0,
                ..settings.clone()
            }
            .validate()
            .is_err()
        );
        assert!(
            Settings {
                width: 2001,
                ..settings
            }
            .validate()
            .is_err()
        );
    }
}
