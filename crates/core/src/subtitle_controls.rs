//! Subtitle edit intent shared by native application frontends.

use crate::Command;
use serde::{Deserialize, Serialize};

pub const DEFAULT_SUBTITLE_FONT: &str = "sans-serif";

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TextStylePreferences {
    pub text_color: crate::subtitle_color::SubtitleColor,
    pub bold: bool,
    pub italic: bool,
    pub blur: f64,
    pub spacing: f64,
    pub outline_size: f64,
    pub outline_color: crate::subtitle_color::SubtitleColor,
    pub shadow_offset: f64,
    pub shadow_color: crate::subtitle_color::SubtitleColor,
}

impl Default for TextStylePreferences {
    fn default() -> Self {
        Self {
            text_color: crate::subtitle_color::SubtitleColor::WHITE,
            bold: false,
            italic: false,
            blur: 0.0,
            spacing: 0.0,
            outline_size: 3.0,
            outline_color: crate::subtitle_color::SubtitleColor::BLACK,
            shadow_offset: 0.0,
            shadow_color: crate::subtitle_color::SubtitleColor::CLEAR,
        }
    }
}

impl TextStylePreferences {
    pub fn validate(self) -> Result<(), &'static str> {
        if !self.blur.is_finite() || !(0.0..=20.0).contains(&self.blur) {
            return Err("Subtitle blur must be between zero and twenty");
        }
        if !self.spacing.is_finite() {
            return Err("Subtitle spacing must be finite");
        }
        if !self.outline_size.is_finite() || self.outline_size < 0.0 {
            return Err("Subtitle outline size must be finite and non-negative");
        }
        if !self.shadow_offset.is_finite() || self.shadow_offset < 0.0 {
            return Err("Subtitle shadow offset must be finite and non-negative");
        }
        Ok(())
    }
}

pub fn valid_preference_font_size(size: f64) -> bool {
    size.is_finite() && size > 0.0 && size <= 2_000_000_000.0
}

/// IINA's slider uses a visual order that differs from its persisted enum tags.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AssOverrideLevel {
    Yes,
    Force,
    Strip,
    #[default]
    Scale,
    No,
}

impl AssOverrideLevel {
    pub const fn slider_index(self) -> i32 {
        match self {
            Self::No => 0,
            Self::Yes => 1,
            Self::Scale => 2,
            Self::Force => 3,
            Self::Strip => 4,
        }
    }

    pub const fn from_slider_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::No),
            1 => Some(Self::Yes),
            2 => Some(Self::Scale),
            3 => Some(Self::Force),
            4 => Some(Self::Strip),
            _ => None,
        }
    }

    pub const fn mpv_value(self) -> &'static str {
        match self {
            Self::Yes => "yes",
            Self::Force => "force",
            Self::Strip => "strip",
            Self::Scale => "scale",
            Self::No => "no",
        }
    }
}

pub fn valid_font_name(name: &str) -> bool {
    !name.is_empty() && !name.chars().any(char::is_control)
}

pub fn font_from_text(text: &str) -> Result<Command, &'static str> {
    // The reference's custom field permits names not present in the installed
    // font list. Resolution/fallback belongs to libass, not the UI catalog.
    let name = if text.is_empty() {
        DEFAULT_SUBTITLE_FONT
    } else {
        text
    };
    if !valid_font_name(name) {
        return Err("Enter a font name without control characters");
    }
    Ok(Command::SubtitleFont(name.to_owned()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubtitleLane {
    Primary,
    Secondary,
}

impl SubtitleLane {
    pub fn delay_from_text(self, text: &str) -> Result<Command, &'static str> {
        let text = text.trim();
        // Numeric entry can exceed the slider range. Parse directly as a native
        // double so edits never round through a UI toolkit's single precision.
        let delay = if text.is_empty() {
            0.0
        } else {
            text.parse::<f64>()
                .map_err(|_| "Enter a numeric delay in seconds")?
        };
        if !delay.is_finite() {
            return Err("Enter a finite delay in seconds");
        }
        Ok(match self {
            Self::Primary => Command::SubtitleDelay(delay),
            Self::Secondary => Command::SecondarySubtitleDelay(delay),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_font_names_preserve_unicode_and_remain_one_native_argument() {
        assert_eq!(
            font_from_text(""),
            Ok(Command::SubtitleFont(DEFAULT_SUBTITLE_FONT.into()))
        );
        for name in [
            "Segoe UI",
            "字体 Привет",
            "Example, serif",
            "Font \"quoted\"",
            " Custom font ",
        ] {
            assert_eq!(
                font_from_text(name).unwrap().mpv_args().unwrap(),
                ["set", "sub-font", name]
            );
        }
    }

    #[test]
    fn invalid_font_strings_are_rejected_before_native_dispatch() {
        assert!(Command::SubtitleFont(String::new()).mpv_args().is_err());
        for name in ["bad\0font", "bad\nfont", "bad\rfont", "bad\tfont"] {
            assert!(font_from_text(name).is_err());
            assert!(Command::SubtitleFont(name.into()).mpv_args().is_err());
        }
    }

    #[test]
    fn numeric_edits_preserve_precision_and_target_only_the_chosen_lane() {
        for (text, value) in [
            ("", 0.0),
            (" \t", 0.0),
            (" +6.125 ", 6.125),
            ("-7.375", -7.375),
            ("1.0000000000000002", 1.0000000000000002),
        ] {
            assert_eq!(
                SubtitleLane::Primary.delay_from_text(text),
                Ok(Command::SubtitleDelay(value))
            );
            assert_eq!(
                SubtitleLane::Secondary.delay_from_text(text),
                Ok(Command::SecondarySubtitleDelay(value))
            );
        }
    }

    #[test]
    fn invalid_numeric_edits_cannot_create_playback_intent() {
        for text in ["NaN", "inf", "-inf", "1e999", "1,5", "--1", "caption"] {
            for lane in [SubtitleLane::Primary, SubtitleLane::Secondary] {
                assert!(lane.delay_from_text(text).is_err(), "{text}");
            }
        }
    }

    #[test]
    fn ass_override_slider_keeps_iina_order_and_scale_default() {
        let levels = [
            AssOverrideLevel::No,
            AssOverrideLevel::Yes,
            AssOverrideLevel::Scale,
            AssOverrideLevel::Force,
            AssOverrideLevel::Strip,
        ];
        for (index, level) in levels.into_iter().enumerate() {
            assert_eq!(
                AssOverrideLevel::from_slider_index(index as i32),
                Some(level)
            );
            assert_eq!(level.slider_index(), index as i32);
        }
        assert_eq!(AssOverrideLevel::default(), AssOverrideLevel::Scale);
        assert_eq!(AssOverrideLevel::from_slider_index(-1), None);
        assert_eq!(AssOverrideLevel::from_slider_index(5), None);
        for (lane, property) in [
            (SubtitleLane::Primary, "sub-ass-override"),
            (SubtitleLane::Secondary, "secondary-sub-ass-override"),
        ] {
            assert_eq!(
                Command::SubtitleAssOverride {
                    lane,
                    level: AssOverrideLevel::Force,
                }
                .mpv_args()
                .unwrap(),
                ["set", property, "force"]
            );
        }
    }
}
