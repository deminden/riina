//! Subtitle edit intent shared by native application frontends.

use crate::Command;

pub const DEFAULT_SUBTITLE_FONT: &str = "sans-serif";

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
}
