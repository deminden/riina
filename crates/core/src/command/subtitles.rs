use super::{Command, cmd, flag, set};
use crate::subtitle_controls;

impl Command {
    pub(super) fn subtitles_args(&self) -> Result<Vec<String>, String> {
        let args = match self {
            Self::MultiplySubtitleScale(factor) if factor.is_finite() && *factor > 0.0 => {
                vec!["multiply".into(), "sub-scale".into(), factor.to_string()]
            }
            Self::SubtitleAssOverride { lane, level } => set(
                match lane {
                    subtitle_controls::SubtitleLane::Primary => "sub-ass-override",
                    subtitle_controls::SubtitleLane::Secondary => "secondary-sub-ass-override",
                },
                level.mpv_value(),
            ),
            Self::AudioDelay(value) | Self::SubtitleDelay(value) if value.is_finite() => set(
                if matches!(self, Self::AudioDelay(_)) {
                    "audio-delay"
                } else {
                    "sub-delay"
                },
                value.to_string(),
            ),
            Self::ToggleSubtitleVisibility => cmd(["cycle", "sub-visibility"]),
            Self::SubtitleFont(font) if subtitle_controls::valid_font_name(font) => {
                set("sub-font", font.clone())
            }
            Self::SubtitlePreferredLanguages(languages)
                if languages.len() <= 256 && !languages.chars().any(char::is_control) =>
            {
                set("slang", languages.clone())
            }
            Self::SubtitleBold(value) => set("sub-bold", flag(*value)),
            Self::SubtitleItalic(value) => set("sub-italic", flag(*value)),
            Self::SubtitleBlur(value) if value.is_finite() && (0.0..=20.0).contains(value) => {
                set("sub-blur", value.to_string())
            }
            Self::SubtitleSpacing(value) if value.is_finite() => {
                set("sub-spacing", value.to_string())
            }
            Self::SubtitleShadowOffset(value) if value.is_finite() && *value >= 0.0 => {
                set("sub-shadow-offset", value.to_string())
            }
            Self::SubtitleTextColor(color)
            | Self::SubtitleBorderColor(color)
            | Self::SubtitleShadowColor(color) => {
                let option = match self {
                    Self::SubtitleTextColor(_) => "sub-color",
                    Self::SubtitleBorderColor(_) => "sub-outline-color",
                    _ => "sub-back-color",
                };
                set(option, color.mpv_value())
            }
            Self::SubtitleOutlineSize(value) if value.is_finite() && *value >= 0.0 => {
                set("sub-outline-size", value.to_string())
            }
            Self::SubtitleFontSize(value) if value.is_finite() && *value > 0.0 => {
                set("sub-font-size", value.to_string())
            }
            Self::SubtitleScale(value) if value.is_finite() && (0.0..=100.0).contains(value) => {
                set("sub-scale", value.to_string())
            }
            Self::SecondarySubtitleDelay(value) if value.is_finite() => {
                set("secondary-sub-delay", value.to_string())
            }
            Self::SubtitlePosition(value) | Self::SecondarySubtitlePosition(value)
                if value.is_finite() && (0.0..=150.0).contains(value) =>
            {
                set(
                    if matches!(self, Self::SubtitlePosition(_)) {
                        "sub-pos"
                    } else {
                        "secondary-sub-pos"
                    },
                    value.to_string(),
                )
            }
            Self::SubtitleAlignX(alignment) => set("sub-align-x", alignment.mpv_value()),
            Self::SubtitleAlignY(alignment) => set("sub-align-y", alignment.mpv_value()),
            Self::SubtitleScaleWithWindow(enabled) => set("sub-scale-by-window", flag(*enabled)),
            Self::SubtitleMarginX(value) | Self::SubtitleMarginY(value)
                if value.is_finite() && (0.0..=2_000_000_000.0).contains(value) =>
            {
                set(
                    if matches!(self, Self::SubtitleMarginX(_)) {
                        "sub-margin-x"
                    } else {
                        "sub-margin-y"
                    },
                    value.to_string(),
                )
            }
            Self::ToggleSecondarySubtitleVisibility => cmd(["cycle", "secondary-sub-visibility"]),
            Self::AdjustAudioDelay(delta)
            | Self::AdjustSubtitleDelay(delta)
            | Self::AdjustSubtitleScale(delta)
            | Self::AdjustSubtitlePosition(delta)
                if delta.is_finite() =>
            {
                // Apply on the decoder worker's current value; rapid key presses
                // must not reuse a stale value from a coalesced UI snapshot.
                vec![
                    "add".into(),
                    match self {
                        Self::AdjustAudioDelay(_) => "audio-delay",
                        Self::AdjustSubtitleDelay(_) => "sub-delay",
                        Self::AdjustSubtitleScale(_) => "sub-scale",
                        Self::AdjustSubtitlePosition(_) => "sub-pos",
                        _ => unreachable!(),
                    }
                    .into(),
                    delta.to_string(),
                ]
            }
            Self::SubtitleSeek(skip) => {
                // Let the engine resolve selected-track cues and subtitle delay;
                // a UI timestamp search would bypass both timing semantics.
                vec!["sub-seek".into(), skip.to_string()]
            }
            _ => return Err("The requested value is out of range or non-finite".into()),
        };
        Ok(args)
    }
}
