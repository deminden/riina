//! Portable video intent; presentation labels remain distinct from native ratios.
use std::sync::LazyLock;

pub const ASPECT_PRESETS: [&str; 6] = ["Default", "4:3", "16:9", "16:10", "21:9", "5:4"];
pub const ASPECT_MENU_PRESETS: [&str; 10] = [
    "Default", "4:3", "5:4", "16:9", "16:10", "1:1", "3:2", "2.21:1", "2.35:1", "2.39:1",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Equalizer {
    Brightness,
    Contrast,
    Saturation,
    Gamma,
    Hue,
}

impl Equalizer {
    pub const ALL: [Self; 5] = [
        Self::Brightness,
        Self::Contrast,
        Self::Saturation,
        Self::Gamma,
        Self::Hue,
    ];

    pub fn property(self) -> &'static str {
        match self {
            Self::Brightness => "brightness",
            Self::Contrast => "contrast",
            Self::Saturation => "saturation",
            Self::Gamma => "gamma",
            Self::Hue => "hue",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|option| *option == self)
            .expect("All equalizer options have a display slot")
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HardwareDecoder {
    Disabled,
    #[default]
    Auto,
    AutoCopy,
}

impl HardwareDecoder {
    pub fn native_value(self) -> &'static str {
        match self {
            Self::Disabled => "no",
            Self::Auto => "auto",
            Self::AutoCopy => "auto-copy",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AspectRatio {
    text: String,
    valid: bool,
}

impl Default for AspectRatio {
    fn default() -> Self {
        Self::from_text("Default")
    }
}

impl AspectRatio {
    pub fn from_text(text: &str) -> Self {
        // Invalid nonempty custom input resets the override, matching the player
        // action. Empty custom edits are ignored by the UI before reaching here.
        let valid = text.split_once(':').is_some_and(|(width, height)| {
            decimal(width) && decimal(height) && !height.contains(':')
        });
        Self {
            text: text.to_owned(),
            valid,
        }
    }

    pub fn native_value(&self) -> &str {
        if self.valid { &self.text } else { "-1" }
    }

    pub fn label(&self) -> &str {
        if self.valid { &self.text } else { "Default" }
    }

    pub fn requested_text(&self) -> &str {
        &self.text
    }

    pub fn menu_index(&self) -> Option<usize> {
        ASPECT_MENU_PRESETS
            .iter()
            .position(|preset| *preset == self.label())
    }

    pub fn preset_index(&self) -> Option<usize> {
        ASPECT_PRESETS
            .iter()
            .position(|preset| *preset == self.label())
    }
}

fn decimal(value: &str) -> bool {
    static DIGITS: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"\A\p{Nd}+\z").expect("The fixed decimal-digit pattern is valid")
    });
    // ICU decimal digits include Arabic/Indic/fullwidth digits, unlike ASCII
    // parsing or Rust's broader numeric category (which includes fractions).
    // The pinned reference's character-count search range excludes supplementary
    // characters; keep that acceptance boundary pending a native comparison.
    let digits = |part: &str| part.chars().all(|ch| ch <= '\u{ffff}') && DIGITS.is_match(part);
    value.split_once('.').map_or_else(
        || digits(value),
        |(integer, fraction)| digits(integer) && digits(fraction),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rotation {
    Zero,
    Clockwise90,
    Clockwise180,
    Clockwise270,
}

impl Rotation {
    pub fn from_degrees(degrees: i64) -> Option<Self> {
        match degrees {
            0 => Some(Self::Zero),
            90 => Some(Self::Clockwise90),
            180 => Some(Self::Clockwise180),
            270 => Some(Self::Clockwise270),
            _ => None,
        }
    }

    pub fn degrees(self) -> i64 {
        match self {
            Self::Zero => 0,
            Self::Clockwise90 => 90,
            Self::Clockwise180 => 180,
            Self::Clockwise270 => 270,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Command;

    #[test]
    fn equalizer_intents_validate_all_channels_and_signed_boundaries() {
        for option in Equalizer::ALL {
            for value in [-100, -1, 0, 1, 100] {
                assert_eq!(
                    Command::VideoEqualizer { option, value }
                        .mpv_args()
                        .unwrap(),
                    [
                        "set".to_owned(),
                        option.property().to_owned(),
                        value.to_string()
                    ]
                );
            }
            for value in [-101, 101, i64::MIN, i64::MAX] {
                assert!(
                    Command::VideoEqualizer { option, value }
                        .mpv_args()
                        .is_err()
                );
            }
        }
    }

    #[test]
    fn hardware_preferences_keep_auto_default_and_restore_copy_policy() {
        let legacy = crate::Preferences::parse(r#"{"volume":37}"#).unwrap();
        assert_eq!(legacy.hardware_decoder, HardwareDecoder::Auto);
        let prefs = crate::Preferences {
            hardware_decoder: HardwareDecoder::AutoCopy,
            ..Default::default()
        };
        let restored = crate::Preferences::parse(&serde_json::to_string(&prefs).unwrap()).unwrap();
        assert_eq!(restored.hardware_decoder, HardwareDecoder::AutoCopy);
        assert!(crate::Preferences::parse(r#"{"hardware_decoder":"unknown"}"#).is_err());
        for mode in [
            HardwareDecoder::Disabled,
            HardwareDecoder::Auto,
            HardwareDecoder::AutoCopy,
        ] {
            assert_eq!(
                Command::HardwareDecoding(mode).mpv_args().unwrap(),
                ["set", "hwdec", mode.native_value()]
            );
        }
    }

    #[test]
    fn aspect_input_keeps_exact_ratio_text_and_resets_invalid_syntax() {
        for ratio in ["4:3", "8:6", "2.39:1", "1:1.5", "01.00:02.00", "0:1"] {
            let aspect = AspectRatio::from_text(ratio);
            assert_eq!(aspect.label(), ratio);
            assert_eq!(aspect.requested_text(), ratio);
            assert_eq!(
                Command::VideoAspect(aspect).mpv_args().unwrap(),
                ["set", "video-aspect-override", ratio]
            );
        }
        for invalid in [
            "", "Default", "16/9", " 16:9", "16:9 ", ".5:1", "1.:1", "1:2:3", "NaN:1", "1:1\0",
        ] {
            let aspect = AspectRatio::from_text(invalid);
            assert_eq!(aspect.label(), "Default");
            assert_eq!(aspect.requested_text(), invalid);
            assert_eq!(aspect.preset_index(), Some(0));
            assert_eq!(aspect.native_value(), "-1");
        }
        assert_eq!(AspectRatio::from_text("8:6").preset_index(), None);
        assert_eq!(AspectRatio::from_text("8:6").menu_index(), None);
        assert_eq!(AspectRatio::from_text("21:9").menu_index(), None);
        assert_eq!(AspectRatio::default().menu_index(), Some(0));
        assert_eq!(AspectRatio::from_text("1:1").preset_index(), None);
        assert_eq!(AspectRatio::from_text("1:1").menu_index(), Some(5));
        for (index, preset) in ASPECT_PRESETS.iter().enumerate() {
            assert_eq!(AspectRatio::from_text(preset).preset_index(), Some(index));
        }
    }

    #[test]
    fn rotation_choices_accept_only_the_reference_quarter_turns() {
        for degrees in [0, 90, 180, 270] {
            let rotation = Rotation::from_degrees(degrees).unwrap();
            assert_eq!(rotation.degrees(), degrees);
            assert_eq!(
                Command::VideoRotation(rotation).mpv_args().unwrap(),
                [
                    "set".to_owned(),
                    "video-rotate".to_owned(),
                    degrees.to_string()
                ]
            );
        }
        for invalid in [-90, 45, 360, i64::MAX] {
            assert_eq!(Rotation::from_degrees(invalid), None);
        }
    }

    #[test]
    fn aspect_digits_preserve_unicode_decimal_syntax_without_transliterating() {
        for ratio in ["١٦:٩", "१६:९", "１６:９", "۱۶.۵:۹", "1６:9"] {
            let aspect = AspectRatio::from_text(ratio);
            assert_eq!(aspect.native_value(), ratio);
            assert_eq!(aspect.label(), ratio);
            assert_eq!(aspect.preset_index(), None);
        }
        for invalid in ["²:1", "Ⅳ:Ⅲ", "½:1", "١٫٥:١", "𝟙𝟞:𝟡"] {
            assert_eq!(AspectRatio::from_text(invalid).native_value(), "-1");
        }
    }
}
