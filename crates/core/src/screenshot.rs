//! Capture content is distinct from the eventual file/clipboard destination.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    #[default]
    Png,
    Jpg,
    Jpeg,
    Webp,
    Jxl,
    Avif,
}

impl Format {
    pub const fn native_value(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpg => "jpg",
            Self::Jpeg => "jpeg",
            Self::Webp => "webp",
            Self::Jxl => "jxl",
            Self::Avif => "avif",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Encoding {
    pub format: Format,
    pub template: String,
}

impl Default for Encoding {
    fn default() -> Self {
        Self {
            format: Format::Png,
            template: "%F-%n".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub encoding: Encoding,
    pub directory: Option<String>,
    pub include_subtitles: bool,
    pub show_preview: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            encoding: Encoding::default(),
            directory: None,
            include_subtitles: true,
            show_preview: true,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if self.encoding.template.contains('\0')
            || self
                .directory
                .as_ref()
                .is_some_and(|path| path.contains('\0'))
        {
            return Err("Screenshot settings contain a NUL byte".into());
        }
        Ok(())
    }

    pub const fn content(&self) -> Content {
        if self.include_subtitles {
            Content::Subtitles
        } else {
            Content::Video
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Content {
    Video,
    #[default]
    Subtitles,
    Window,
}

impl Content {
    pub const fn native_value(self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Subtitles => "subtitles",
            Self::Window => "window",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Preferences;

    #[test]
    fn old_preferences_keep_defaults_and_custom_capture_settings_round_trip() {
        assert_eq!(
            Preferences::parse("{}").unwrap().screenshot,
            Settings::default()
        );
        for format in [
            Format::Png,
            Format::Jpg,
            Format::Jpeg,
            Format::Webp,
            Format::Jxl,
            Format::Avif,
        ] {
            let prefs = Preferences {
                screenshot: Settings {
                    encoding: Encoding {
                        format,
                        template: "Снимок-%F-%n".into(),
                    },
                    directory: Some("~/Pictures/Снимки 字".into()),
                    include_subtitles: false,
                    show_preview: false,
                },
                ..Default::default()
            };
            let restored = Preferences::parse(&serde_json::to_string(&prefs).unwrap()).unwrap();
            assert_eq!(restored.screenshot, prefs.screenshot);
            assert_eq!(restored.screenshot.content(), Content::Video);
        }
        assert!(Preferences::parse(r#"{"screenshot":{"encoding":{"format":"gif"}}}"#).is_err());
        assert!(
            Preferences::parse(r#"{"screenshot":{"encoding":{"template":"bad\u0000"}}}"#).is_err()
        );
    }
}
