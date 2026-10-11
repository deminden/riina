//! Persisted Picture-in-Picture policies shared by every platform adapter.

use serde::{Deserialize, Serialize};

/// What to do with the main player window after the PiP surface opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WindowBehavior {
    DoNothing,
    #[default]
    Hide,
    Minimize,
}

impl WindowBehavior {
    pub const fn index(self) -> i32 {
        match self {
            Self::DoNothing => 0,
            Self::Hide => 1,
            Self::Minimize => 2,
        }
    }

    pub const fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::DoNothing),
            1 => Some(Self::Hide),
            2 => Some(Self::Minimize),
            _ => None,
        }
    }
}

/// IINA-compatible PiP settings. Window behavior remains a platform action;
/// these values only describe the user's policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Settings {
    pub window_behavior: WindowBehavior,
    pub pause_when_entering: bool,
    pub toggle_by_minimizing: bool,
    pub minimize_toggle_video_only: bool,
}

#[cfg(test)]
mod tests {
    use super::{Settings, WindowBehavior};

    #[test]
    fn iina_defaults_and_window_behavior_tags_are_preserved() {
        let defaults = Settings::default();
        assert_eq!(defaults.window_behavior, WindowBehavior::Hide);
        assert!(!defaults.pause_when_entering);
        assert!(!defaults.toggle_by_minimizing);
        assert!(!defaults.minimize_toggle_video_only);

        for behavior in [
            WindowBehavior::DoNothing,
            WindowBehavior::Hide,
            WindowBehavior::Minimize,
        ] {
            assert_eq!(WindowBehavior::from_index(behavior.index()), Some(behavior));
        }
        assert_eq!(WindowBehavior::from_index(-1), None);
        assert_eq!(WindowBehavior::from_index(3), None);
    }

    #[test]
    fn settings_round_trip_and_legacy_partial_settings_keep_defaults() {
        let configured = Settings {
            window_behavior: WindowBehavior::Minimize,
            pause_when_entering: true,
            toggle_by_minimizing: true,
            minimize_toggle_video_only: true,
        };
        let saved = serde_json::to_string(&configured).unwrap();
        assert_eq!(
            serde_json::from_str::<Settings>(&saved).unwrap(),
            configured
        );

        let legacy = serde_json::from_str::<Settings>(r#"{"pause_when_entering":true}"#).unwrap();
        assert_eq!(legacy.window_behavior, WindowBehavior::Hide);
        assert!(legacy.pause_when_entering);
        assert!(!legacy.toggle_by_minimizing);
        assert!(!legacy.minimize_toggle_video_only);
    }
}
