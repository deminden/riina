//! Capture content is distinct from the eventual file/clipboard destination.
use crate::{
    PreferenceError,
    key_bindings::{
        InputCommand, mpv_command_prefix_len, mpv_command_suppresses_osd, parse_mpv_input,
    },
};
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
    pub destinations: Destinations,
    pub encoding: Encoding,
    pub directory: Option<String>,
    pub include_subtitles: bool,
    pub show_preview: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            destinations: Destinations::default(),
            encoding: Encoding::default(),
            directory: None,
            include_subtitles: true,
            show_preview: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Destinations {
    pub save_to_file: bool,
    pub copy_to_clipboard: bool,
}

impl Default for Destinations {
    fn default() -> Self {
        Self {
            save_to_file: true,
            copy_to_clipboard: false,
        }
    }
}

impl Destinations {
    pub const fn enabled(self) -> bool {
        self.save_to_file || self.copy_to_clipboard
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), PreferenceError> {
        if self.encoding.template.contains('\0')
            || self
                .directory
                .as_ref()
                .is_some_and(|path| path.contains('\0'))
        {
            return Err(PreferenceError::new(
                "screenshot",
                "Screenshot settings contain a NUL byte",
            ));
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

/// A validated single screenshot command from input.conf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding(InputCommand);

impl Binding {
    pub fn parse(action: &str) -> Option<Self> {
        let mut commands = parse_mpv_input(action).ok()?;
        if commands.len() != 1 {
            return None;
        }
        let command = commands.pop()?;
        (command.arguments().first()?.as_str() == "screenshot").then_some(Self(command))
    }

    pub fn reports_completion(&self) -> bool {
        let args = self.0.arguments();
        let flags = args.get(1).map_or("", String::as_str);
        (args.len() <= 2 || (args.len() == 3 && args[2] == "single"))
            && !mpv_command_suppresses_osd(self.0.prefixes())
            && (flags.is_empty()
                || flags
                    .split('+')
                    .all(|flag| matches!(flag, "video" | "subtitles" | "window")))
    }

    pub(crate) fn native_arguments(self, default_content: Content) -> Vec<String> {
        let mut args = self.0.into_native();
        let flags = mpv_command_prefix_len(&args) + 1;
        match args.get_mut(flags) {
            None => args.push(default_content.native_value().into()),
            Some(value) if value.is_empty() => *value = default_content.native_value().into(),
            Some(_) => {}
        }
        args
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
        for save_to_file in [false, true] {
            for copy_to_clipboard in [false, true] {
                let mut preferences = Preferences::default();
                preferences.screenshot.destinations = Destinations {
                    save_to_file,
                    copy_to_clipboard,
                };
                let restored =
                    Preferences::parse(&serde_json::to_string(&preferences).unwrap()).unwrap();
                assert_eq!(
                    restored.screenshot.destinations,
                    preferences.screenshot.destinations
                );
            }
        }
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
                    destinations: Destinations::default(),
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
