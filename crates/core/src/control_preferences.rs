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

/// Portable meaning of a configured click gesture; platform UI applies it to
/// window state or routes its playback intent to the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseEffect {
    None,
    Fullscreen,
    SetPaused(bool),
    HideControls,
    PictureInPicture,
    CycleAbLoop,
    ResetSpeed,
}

impl MouseAction {
    pub const fn effect(self, currently_paused: bool) -> MouseEffect {
        match self {
            Self::None => MouseEffect::None,
            Self::Fullscreen => MouseEffect::Fullscreen,
            Self::Pause => MouseEffect::SetPaused(!currently_paused),
            Self::HideOsc => MouseEffect::HideControls,
            Self::PictureInPicture => MouseEffect::PictureInPicture,
            Self::AbLoop => MouseEffect::CycleAbLoop,
            Self::ResetSpeed => MouseEffect::ResetSpeed,
        }
    }

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

/// A playback change produced by one configured scroll gesture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScrollEffect {
    None,
    AdjustVolume(f64),
    Seek(f64),
    MultiplySpeed(f64),
}

/// Convert a platform scroll delta to an IINA-style adjustment. `delta` is in
/// logical pixels; coarse wheel events are normalized by the host adapter.
pub fn scroll_effect(
    settings: &Settings,
    horizontal: bool,
    delta: f64,
    precise: bool,
) -> ScrollEffect {
    let action = if horizontal {
        settings.horizontal_scroll_action
    } else {
        settings.vertical_scroll_action
    };
    scroll_effect_for_action(settings, action, horizontal, delta, precise)
}

/// Apply a scroll override supplied by a control such as the timeline or volume slider.
pub fn scroll_effect_for_action(
    settings: &Settings,
    action: ScrollAction,
    horizontal: bool,
    delta: f64,
    precise: bool,
) -> ScrollEffect {
    // Slint expands one wheel line to 60 logical pixels. Preserve fractional
    // wheel lines instead of promoting every high-resolution report to a notch.
    let delta = if precise { delta } else { delta / 60.0 };
    scroll_effect_for_input(settings, action, horizontal, delta, precise, !precise)
}

/// Apply an IINA scroll action while keeping event precision separate from the
/// gesture class. AppKit can report precise deltas for mouse wheels whose phase
/// is empty; IINA still uses its mouse seek map for those events.
/// `delta` is logical pixels for precise input and wheel lines otherwise. The
/// AppKit adapter supplies signed unit lines for its non-precise mouse events.
pub fn scroll_effect_for_input(
    settings: &Settings,
    action: ScrollAction,
    horizontal: bool,
    delta: f64,
    precise_delta: bool,
    mouse_wheel: bool,
) -> ScrollEffect {
    if !delta.is_finite() || delta == 0.0 || settings.validate().is_err() {
        return ScrollEffect::None;
    }
    let delta = if precise_delta {
        delta
    } else {
        delta * if horizontal { 1.0 } else { 2.0 }
    };
    let sensitivity = |values: [f64; 4], index: u8| values[usize::from(index - 1)];

    match action {
        ScrollAction::None => ScrollEffect::None,
        ScrollAction::Volume => ScrollEffect::AdjustVolume(if mouse_wheel {
            delta
        } else {
            delta * sensitivity([0.25, 0.5, 0.75, 1.0], settings.volume_sensitivity)
        }),
        ScrollAction::Seek => {
            let amount = if mouse_wheel {
                [0.5, 1.0, 2.0, 4.0][usize::from(settings.relative_seek_amount - 1)]
            } else {
                [0.05, 0.1, 0.25, 0.5][usize::from(settings.relative_seek_amount - 1)]
            };
            ScrollEffect::Seek(delta * amount)
        }
        ScrollAction::PlaybackSpeed => ScrollEffect::MultiplySpeed(
            1.0 + delta * sensitivity([0.001, 0.002, 0.005, 0.01], settings.speed_sensitivity),
        ),
    }
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
    pub disable_play_slider_scrolling: bool,
    pub disable_volume_slider_scrolling: bool,
    pub seek_type: SeekType,
    #[serde(alias = "seek_sensitivity")]
    pub relative_seek_amount: u8,
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
            disable_play_slider_scrolling: false,
            disable_volume_slider_scrolling: false,
            seek_type: SeekType::Keyframe,
            relative_seek_amount: 3,
            volume_sensitivity: 3,
            speed_sensitivity: 3,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !(1..=4).contains(&self.relative_seek_amount) {
            return Err("Relative seek amount must be between 1 and 4");
        }
        if !(1..=4).contains(&self.volume_sensitivity) || !(1..=4).contains(&self.speed_sensitivity)
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
        assert_eq!(settings.relative_seek_amount, 3);
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
    fn click_actions_map_to_explicit_window_and_playback_effects() {
        assert_eq!(MouseAction::None.effect(false), MouseEffect::None);
        assert_eq!(
            MouseAction::Fullscreen.effect(false),
            MouseEffect::Fullscreen
        );
        assert_eq!(
            MouseAction::Pause.effect(false),
            MouseEffect::SetPaused(true)
        );
        assert_eq!(
            MouseAction::Pause.effect(true),
            MouseEffect::SetPaused(false)
        );
        assert_eq!(
            MouseAction::HideOsc.effect(false),
            MouseEffect::HideControls
        );
        assert_eq!(
            MouseAction::PictureInPicture.effect(false),
            MouseEffect::PictureInPicture
        );
        assert_eq!(MouseAction::AbLoop.effect(false), MouseEffect::CycleAbLoop);
        assert_eq!(
            MouseAction::ResetSpeed.effect(false),
            MouseEffect::ResetSpeed
        );
    }

    #[test]
    fn scroll_amounts_reject_values_outside_source_slider_bounds() {
        let below_range = Settings {
            relative_seek_amount: 0,
            ..Settings::default()
        };
        assert!(below_range.validate().is_err());

        let upper_boundary = Settings {
            relative_seek_amount: 4,
            ..Settings::default()
        };
        assert!(upper_boundary.validate().is_ok());
    }

    #[test]
    fn default_scrolls_match_volume_and_keyframe_seek_controls() {
        let settings = Settings::default();
        assert_eq!(
            scroll_effect(&settings, false, 60.0, false),
            ScrollEffect::AdjustVolume(2.0)
        );
        assert_eq!(
            scroll_effect(&settings, true, 60.0, false),
            ScrollEffect::Seek(2.0)
        );
        // Twelve 1/12-notch reports must equal one full notch, in either
        // direction. Smooth Windows mice commonly emit these small fractions.
        for delta in [-60.0, -5.0, -0.5, 0.5, 5.0, 60.0, 120.0] {
            assert_eq!(
                scroll_effect(&settings, true, delta, false),
                ScrollEffect::Seek(delta / 60.0 * 2.0)
            );
            assert_eq!(
                scroll_effect(&settings, false, delta, false),
                ScrollEffect::AdjustVolume(delta / 60.0 * 2.0)
            );
        }
    }

    #[test]
    fn precise_scroll_uses_selected_sensitivity_and_speed_is_bounded() {
        let precise = Settings {
            vertical_scroll_action: ScrollAction::Seek,
            relative_seek_amount: 4,
            ..Settings::default()
        };
        assert_eq!(
            scroll_effect(&precise, false, 4.0, true),
            ScrollEffect::Seek(2.0)
        );

        let speed = Settings {
            vertical_scroll_action: ScrollAction::PlaybackSpeed,
            speed_sensitivity: 4,
            ..Settings::default()
        };
        assert_eq!(
            scroll_effect(&speed, false, 60.0, false),
            ScrollEffect::MultiplySpeed(1.02)
        );
    }

    #[test]
    fn mouse_wheel_and_trackpad_keep_their_independent_iina_seek_curves() {
        let settings = Settings {
            relative_seek_amount: 3,
            ..Settings::default()
        };
        assert_eq!(
            scroll_effect_for_input(&settings, ScrollAction::Seek, false, 1.0, true, true,),
            ScrollEffect::Seek(2.0)
        );
        assert_eq!(
            scroll_effect_for_input(&settings, ScrollAction::Seek, false, 1.0, true, false,),
            ScrollEffect::Seek(0.25)
        );
        assert_eq!(
            scroll_effect_for_input(&settings, ScrollAction::Volume, false, 1.0, true, true,),
            ScrollEffect::AdjustVolume(1.0)
        );
        assert_eq!(
            scroll_effect_for_input(&settings, ScrollAction::Volume, false, 1.0, true, false,),
            ScrollEffect::AdjustVolume(0.75)
        );
    }

    #[test]
    fn invalid_or_zero_scroll_does_not_create_playback_commands() {
        let invalid = Settings {
            volume_sensitivity: 0,
            ..Settings::default()
        };
        assert_eq!(
            scroll_effect(&Settings::default(), false, f64::NAN, true),
            ScrollEffect::None
        );
        assert_eq!(
            scroll_effect(&invalid, false, 1.0, true),
            ScrollEffect::None
        );
    }
}
