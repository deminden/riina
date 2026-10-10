//! Subtitle edit intent shared by native application frontends.

use crate::Command;

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
