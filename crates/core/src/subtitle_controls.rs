//! Subtitle edit intent shared by native application frontends.

use crate::Command;
use serde::{Deserialize, Serialize};

pub const DEFAULT_SUBTITLE_FONT: &str = "sans-serif";

pub const SUBTITLE_AUTO_LOAD_MODES: &[&str] = &[
    "Disabled",
    "Subtitles containing media filename",
    "Detect intelligently by IINA",
];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoLoadMode {
    Disabled,
    Filename,
    #[default]
    Intelligent,
}

impl AutoLoadMode {
    pub const fn index(self) -> i32 {
        match self {
            Self::Disabled => 0,
            Self::Filename => 1,
            Self::Intelligent => 2,
        }
    }

    pub const fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::Disabled),
            1 => Some(Self::Filename),
            2 => Some(Self::Intelligent),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AutoLoadPreferences {
    pub mode: AutoLoadMode,
    pub priority_keywords: String,
    pub search_paths: String,
}

impl Default for AutoLoadPreferences {
    fn default() -> Self {
        Self {
            mode: AutoLoadMode::default(),
            priority_keywords: String::new(),
            search_paths: "./*".into(),
        }
    }
}

impl AutoLoadPreferences {
    pub fn validate(&self) -> Result<(), &'static str> {
        for value in [&self.priority_keywords, &self.search_paths] {
            if value.len() > 4096 || value.chars().any(char::is_control) {
                return Err("Subtitle auto-load preferences contain invalid text");
            }
        }
        Ok(())
    }

    /// Map the preference intent onto mpv's portable sidecar matching modes.
    /// Priority-keyword selection is applied after native sidecar discovery;
    /// mpv still owns directory traversal and external-track loading.
    pub const fn mpv_matching_mode(&self) -> &'static str {
        match self.mode {
            AutoLoadMode::Disabled => "no",
            AutoLoadMode::Filename => "fuzzy",
            AutoLoadMode::Intelligent => "exact",
        }
    }

    /// mpv accepts literal directories, while IINA's default path is a glob.
    /// The player-side discovery worker expands trailing child-directory globs
    /// after the current media path is known.
    pub fn mpv_search_paths(&self) -> Option<String> {
        let paths = std::env::split_paths(&self.search_paths).collect::<Vec<_>>();
        if paths.iter().any(|path| {
            path.as_os_str()
                .to_string_lossy()
                .chars()
                .any(|character| matches!(character, '*' | '?'))
        }) {
            return None;
        }
        std::env::join_paths(paths)
            .ok()
            .map(|paths| paths.to_string_lossy().into_owned())
    }
}

/// Display labels and mpv values from the pinned IINA encoding menu.
pub const SUBTITLE_ENCODING_CHOICES: &[(&str, &str)] = &[
    ("Auto detect", "auto"),
    ("Universal (UTF-8)", "UTF-8"),
    ("Universal (UTF-16)", "UTF-16"),
    ("Universal (UTF-16BE)", "UTF-16BE"),
    ("Universal (UTF-16LE)", "UTF-16LE"),
    ("Arabic (ISO-8859-6)", "ISO-8859-6"),
    ("Arabic (WINDOWS-1256)", "WINDOWS-1256"),
    ("Baltic (LATIN7)", "LATIN7"),
    ("Baltic (WINDOWS-1257)", "WINDOWS-1257"),
    ("Celtic (LATIN8)", "LATIN8"),
    ("Central European (WINDOWS-1250)", "WINDOWS-1250"),
    ("Cyrillic (ISO-8859-5)", "ISO-8859-5"),
    ("Cyrillic (WINDOWS-1251)", "WINDOWS-1251"),
    ("Eastern European (ISO-8859-2)", "ISO-8859-2"),
    ("Western Languages (WINDOWS-1252)", "WINDOWS-1252"),
    ("Greek (ISO-8859-7)", "ISO-8859-7"),
    ("Greek (WINDOWS-1253)", "WINDOWS-1253"),
    ("Hebrew (ISO-8859-8)", "ISO-8859-8"),
    ("Hebrew (WINDOWS-1255)", "WINDOWS-1255"),
    ("Japanese (SHIFT-JIS)", "SHIFT-JIS"),
    ("Japanese (ISO-2022-JP-2)", "ISO-2022-JP-2"),
    ("Korean (EUC-KR)", "EUC-KR"),
    ("Korean (CP949)", "CP949"),
    ("Korean (ISO-2022-KR)", "ISO-2022-KR"),
    ("Nordic (LATIN6)", "LATIN6"),
    ("North European (LATIN4)", "LATIN4"),
    ("Russian (KOI8-R)", "KOI8-R"),
    ("Simplified Chinese (GBK)", "GBK"),
    ("Simplified Chinese (GB18030)", "GB18030"),
    ("Simplified Chinese (ISO-2022-CN-EXT)", "ISO-2022-CN-EXT"),
    ("South European (LATIN3)", "LATIN3"),
    ("South-Eastern European (LATIN10)", "LATIN10"),
    ("Thai (TIS-620)", "TIS-620"),
    ("Thai (WINDOWS-874)", "WINDOWS-874"),
    ("Traditional Chinese (EUC-TW)", "EUC-TW"),
    ("Traditional Chinese (BIG5)", "BIG5"),
    ("Traditional Chinese (BIG5-HKSCS)", "BIG5-HKSCS"),
    ("Turkish (LATIN5)", "LATIN5"),
    ("Turkish (WINDOWS-1254)", "WINDOWS-1254"),
    ("Ukrainian (KOI8-U)", "KOI8-U"),
    ("Vietnamese (WINDOWS-1258)", "WINDOWS-1258"),
    ("Vietnamese (VISCII)", "VISCII"),
    ("Western European (LATIN1)", "LATIN1"),
    ("Western European (LATIN-9)", "LATIN-9"),
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OtherPreferences {
    /// IINA stores preferred ISO 639-2 identifiers as a comma-separated string.
    pub preferred_languages: String,
    pub default_encoding: String,
}

impl Default for OtherPreferences {
    fn default() -> Self {
        Self {
            preferred_languages: String::new(),
            default_encoding: "auto".into(),
        }
    }
}

impl OtherPreferences {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.preferred_languages.len() > 256
            || self.preferred_languages.chars().any(char::is_control)
        {
            return Err("Preferred subtitle languages contain invalid text");
        }
        if !SUBTITLE_ENCODING_CHOICES
            .iter()
            .any(|(_, code)| *code == self.default_encoding)
        {
            return Err("Unsupported subtitle encoding");
        }
        Ok(())
    }

    pub fn encoding_index(&self) -> Option<i32> {
        SUBTITLE_ENCODING_CHOICES
            .iter()
            .position(|(_, code)| *code == self.default_encoding)
            .and_then(|index| i32::try_from(index).ok())
    }

    pub fn commands(&self) -> [Command; 2] {
        [
            Command::SubtitlePreferredLanguages(self.preferred_languages.clone()),
            Command::SubtitleEncoding(self.default_encoding.clone()),
        ]
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HorizontalAlignment {
    Left,
    #[default]
    Center,
    Right,
}

impl HorizontalAlignment {
    pub const fn mpv_value(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VerticalAlignment {
    Top,
    Center,
    #[default]
    Bottom,
}

impl VerticalAlignment {
    pub const fn mpv_value(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Center => "center",
            Self::Bottom => "bottom",
        }
    }
}

/// Persistent subtitle placement policy, shared with each native frontend.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PositionPreferences {
    pub horizontal: HorizontalAlignment,
    pub vertical: VerticalAlignment,
    pub position: f64,
    pub display_in_letterbox: bool,
    pub scale_with_window: bool,
    pub margin_x: f64,
    pub margin_y: f64,
}

impl Default for PositionPreferences {
    fn default() -> Self {
        Self {
            horizontal: HorizontalAlignment::Center,
            vertical: VerticalAlignment::Bottom,
            position: 100.0,
            display_in_letterbox: true,
            scale_with_window: true,
            margin_x: 25.0,
            margin_y: 22.0,
        }
    }
}

impl PositionPreferences {
    pub fn validate(self) -> Result<(), &'static str> {
        if !self.position.is_finite() || !(0.0..=100.0).contains(&self.position) {
            return Err("Subtitle vertical position must be between zero and one hundred");
        }
        if [self.margin_x, self.margin_y]
            .into_iter()
            .any(|margin| !margin.is_finite() || !(0.0..=2_000_000_000.0).contains(&margin))
        {
            return Err("Subtitle margins must be finite and non-negative");
        }
        Ok(())
    }

    pub fn commands(self) -> [Command; 7] {
        [
            Command::SubtitleAlignX(self.horizontal),
            Command::SubtitleAlignY(self.vertical),
            Command::SubtitlePosition(self.position),
            Command::SubtitleLetterboxMargins(self.display_in_letterbox),
            Command::SubtitleScaleWithWindow(self.scale_with_window),
            Command::SubtitleMarginX(self.margin_x),
            Command::SubtitleMarginY(self.margin_y),
        ]
    }
}

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
        // Keep numeric editing in native double precision rather than rounding
        // through the UI toolkit's single-precision property.
        let delay = crate::timing_controls::parse_delay_seconds(text)?;
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

    #[test]
    fn subtitle_position_defaults_match_the_pinned_preferences() {
        let position = PositionPreferences::default();
        assert_eq!(position.horizontal, HorizontalAlignment::Center);
        assert_eq!(position.vertical, VerticalAlignment::Bottom);
        assert_eq!(position.position, 100.0);
        assert!(position.display_in_letterbox);
        assert!(position.scale_with_window);
        assert_eq!((position.margin_x, position.margin_y), (25.0, 22.0));
        assert!(position.validate().is_ok());
    }

    #[test]
    fn subtitle_position_rejects_values_outside_engine_and_source_ranges() {
        for position in [f64::NAN, f64::INFINITY, -0.1, 100.1] {
            assert!(
                PositionPreferences {
                    position,
                    ..Default::default()
                }
                .validate()
                .is_err()
            );
        }
        for margin_x in [f64::NAN, f64::INFINITY, -0.1, 2_000_000_001.0] {
            assert!(
                PositionPreferences {
                    margin_x,
                    ..Default::default()
                }
                .validate()
                .is_err()
            );
        }
    }

    #[test]
    fn subtitle_position_commands_keep_alignment_and_letterbox_semantics_typed() {
        let position = PositionPreferences {
            horizontal: HorizontalAlignment::Right,
            vertical: VerticalAlignment::Top,
            position: 84.25,
            display_in_letterbox: false,
            scale_with_window: false,
            margin_x: 31.5,
            margin_y: 18.0,
        };
        assert_eq!(
            position.commands(),
            [
                Command::SubtitleAlignX(HorizontalAlignment::Right),
                Command::SubtitleAlignY(VerticalAlignment::Top),
                Command::SubtitlePosition(84.25),
                Command::SubtitleLetterboxMargins(false),
                Command::SubtitleScaleWithWindow(false),
                Command::SubtitleMarginX(31.5),
                Command::SubtitleMarginY(18.0),
            ]
        );
        assert_eq!(
            Command::SubtitleAlignX(HorizontalAlignment::Right)
                .mpv_args()
                .unwrap(),
            ["set", "sub-align-x", "right"]
        );
        assert_eq!(
            Command::SubtitleScaleWithWindow(false).mpv_args().unwrap(),
            ["set", "sub-scale-by-window", "no"]
        );
        assert_eq!(
            Command::SubtitleLetterboxMargins(false)
                .plan(&crate::Playback::default())
                .unwrap()
                .commands,
            [
                ["set", "sub-use-margins", "no"],
                ["set", "sub-ass-force-margins", "no"],
            ]
        );
    }
}
