//! Shared policy for when the on-screen controller fades after interaction.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub auto_hide_enabled: bool,
    pub auto_hide_timeout_secs: f64,
    /// Visibility mask for IINA's configurable controller actions.
    /// The first four bits retain the original Riina setting assignments.
    pub toolbar_buttons: u16,
    /// Packed as IINA toolbar action IDs: settings, playlist, PiP, fullscreen,
    /// music mode, subtitle track, screenshot, plugins, and Live Text.
    pub toolbar_order: [u8; 9],
}

impl Settings {
    pub const TOOLBAR_PLUGINS: u16 = 1 << 0;
    pub const TOOLBAR_PIP: u16 = 1 << 1;
    pub const TOOLBAR_PLAYLIST: u16 = 1 << 2;
    pub const TOOLBAR_QUICK_SETTINGS: u16 = 1 << 3;
    pub const TOOLBAR_FULLSCREEN: u16 = 1 << 4;
    pub const TOOLBAR_MUSIC_MODE: u16 = 1 << 5;
    pub const TOOLBAR_SUBTITLE_TRACK: u16 = 1 << 6;
    pub const TOOLBAR_SCREENSHOT: u16 = 1 << 7;
    pub const TOOLBAR_LIVE_TEXT: u16 = 1 << 8;
    pub const TOOLBAR_DEFAULT: u16 = Self::TOOLBAR_PLUGINS
        | Self::TOOLBAR_PIP
        | Self::TOOLBAR_PLAYLIST
        | Self::TOOLBAR_QUICK_SETTINGS;
    pub const TOOLBAR_ALL: u16 = Self::TOOLBAR_PLUGINS
        | Self::TOOLBAR_PIP
        | Self::TOOLBAR_PLAYLIST
        | Self::TOOLBAR_QUICK_SETTINGS
        | Self::TOOLBAR_FULLSCREEN
        | Self::TOOLBAR_MUSIC_MODE
        | Self::TOOLBAR_SUBTITLE_TRACK
        | Self::TOOLBAR_SCREENSHOT
        | Self::TOOLBAR_LIVE_TEXT;
}

impl Settings {
    pub fn validate(self) -> Result<Self, &'static str> {
        std::time::Duration::try_from_secs_f64(self.auto_hide_timeout_secs)
            .map_err(|_| "The control-bar auto-hide time must be a finite nonnegative duration")?;
        if self.toolbar_buttons & !Self::TOOLBAR_ALL != 0 {
            return Err("The control-bar toolbar selection contains an unknown action");
        }
        let mut seen = 0_u16;
        for action in self.toolbar_order {
            if action >= 9 || seen & (1 << action) != 0 {
                return Err("The control-bar toolbar order must contain every action once");
            }
            seen |= 1 << action;
        }
        Ok(self)
    }
}

impl Default for Settings {
    fn default() -> Self {
        // IINA defaults to showing the controller for 2.5 seconds after activity.
        Self {
            auto_hide_enabled: true,
            auto_hide_timeout_secs: 2.5,
            toolbar_buttons: Self::TOOLBAR_DEFAULT,
            toolbar_order: [7, 2, 1, 0, 3, 4, 5, 6, 8],
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
