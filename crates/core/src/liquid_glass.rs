//! Per-surface material choices from IINA's macOS 26 Appearance settings.
use serde::{Deserialize, Serialize};

/// Keep the three independent surfaces grouped in saved settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SurfaceSettings {
    pub osd: bool,
    pub osc: bool,
    pub sidebar: bool,
}

impl Default for SurfaceSettings {
    fn default() -> Self {
        // IINA enables each Liquid Glass surface in its macOS 26 defaults.
        Self {
            osd: true,
            osc: true,
            sidebar: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SurfaceSettings;

    #[test]
    fn surface_choices_default_on_and_round_trip_independently() {
        let default = SurfaceSettings::default();
        assert!(default.osd && default.osc && default.sidebar);

        let saved = SurfaceSettings {
            osd: false,
            osc: true,
            sidebar: false,
        };
        let encoded = serde_json::to_string(&saved).unwrap();
        assert_eq!(
            serde_json::from_str::<SurfaceSettings>(&encoded).unwrap(),
            saved
        );
    }
}
