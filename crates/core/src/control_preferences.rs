//! Portable input choices mirror IINA's Control page; host adapters own gestures.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseAction {
    None,
    Fullscreen,
    Pause,
    #[default]
    HideOsc,
    PictureInPicture,
    AbLoop,
    ResetSpeed,
}

impl MouseAction {
    pub const fn index(self) -> i32 {
        match self {
            Self::None => 0,
            Self::Fullscreen => 1,
            Self::Pause => 2,
            Self::HideOsc => 3,
            Self::PictureInPicture => 4,
            Self::AbLoop => 5,
            Self::ResetSpeed => 6,
        }
    }

    pub const fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::None),
            1 => Some(Self::Fullscreen),
            2 => Some(Self::Pause),
            3 => Some(Self::HideOsc),
            4 => Some(Self::PictureInPicture),
            5 => Some(Self::AbLoop),
            6 => Some(Self::ResetSpeed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollAction {
    Volume,
    Seek,
    None,
    PlaybackSpeed,
}

impl ScrollAction {
    pub const fn index(self) -> i32 {
        match self {
            Self::Volume => 0,
            Self::Seek => 1,
            Self::None => 2,
            Self::PlaybackSpeed => 3,
        }
    }

    pub const fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::Volume),
            1 => Some(Self::Seek),
            2 => Some(Self::None),
            3 => Some(Self::PlaybackSpeed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinchAction {
    AdjustWindowSize,
    Fullscreen,
    None,
}

impl PinchAction {
    pub const fn index(self) -> i32 {
        match self {
            Self::AdjustWindowSize => 0,
            Self::Fullscreen => 1,
            Self::None => 2,
        }
    }

    pub const fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::AdjustWindowSize),
            1 => Some(Self::Fullscreen),
            2 => Some(Self::None),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeekType {
    Keyframe,
    Exact,
    Auto,
}

impl SeekType {
    pub const fn index(self) -> i32 {
        match self {
            Self::Keyframe => 0,
            Self::Exact => 1,
            Self::Auto => 2,
        }
    }

    pub const fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::Keyframe),
            1 => Some(Self::Exact),
            2 => Some(Self::Auto),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub pinch_action: PinchAction,
    pub force_touch_action: MouseAction,
    pub single_click_action: MouseAction,
    pub double_click_action: MouseAction,
    pub right_click_action: MouseAction,
    pub middle_click_action: MouseAction,
    pub accepts_first_mouse: bool,
    pub vertical_scroll_action: ScrollAction,
    pub horizontal_scroll_action: ScrollAction,
    pub seek_type: SeekType,
    pub seek_sensitivity: u8,
    pub volume_sensitivity: u8,
    pub speed_sensitivity: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            pinch_action: PinchAction::AdjustWindowSize,
            force_touch_action: MouseAction::None,
            single_click_action: MouseAction::HideOsc,
            double_click_action: MouseAction::Fullscreen,
            right_click_action: MouseAction::Pause,
            middle_click_action: MouseAction::None,
            accepts_first_mouse: false,
            vertical_scroll_action: ScrollAction::Volume,
            horizontal_scroll_action: ScrollAction::Seek,
            seek_type: SeekType::Keyframe,
            seek_sensitivity: 3,
            volume_sensitivity: 3,
            speed_sensitivity: 3,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !(1..=4).contains(&self.seek_sensitivity)
            || !(1..=4).contains(&self.volume_sensitivity)
            || !(1..=4).contains(&self.speed_sensitivity)
        {
            return Err("Control sensitivity must be between 1 and 4");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_iina_control_preferences() {
        let settings = Settings::default();
        assert_eq!(settings.single_click_action, MouseAction::HideOsc);
        assert_eq!(settings.double_click_action, MouseAction::Fullscreen);
        assert_eq!(settings.right_click_action, MouseAction::Pause);
        assert_eq!(settings.vertical_scroll_action, ScrollAction::Volume);
        assert_eq!(settings.horizontal_scroll_action, ScrollAction::Seek);
        assert_eq!(settings.seek_type, SeekType::Keyframe);
        assert_eq!(settings.seek_sensitivity, 3);
        assert_eq!(settings.volume_sensitivity, 3);
        assert_eq!(settings.speed_sensitivity, 3);
    }

    #[test]
    fn action_indices_preserve_iina_popup_tag_order() {
        for index in 0..=6 {
            assert_eq!(MouseAction::from_index(index).unwrap().index(), index);
        }
        assert_eq!(ScrollAction::PlaybackSpeed.index(), 3);
        assert_eq!(PinchAction::AdjustWindowSize.index(), 0);
        assert_eq!(SeekType::Auto.index(), 2);
    }

    #[test]
    fn sensitivities_reject_values_outside_source_slider_bounds() {
        let below_range = Settings {
            seek_sensitivity: 0,
            ..Settings::default()
        };
        assert!(below_range.validate().is_err());

        let upper_boundary = Settings {
            seek_sensitivity: 4,
            ..Settings::default()
        };
        assert!(upper_boundary.validate().is_ok());
    }
}
