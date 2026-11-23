use super::{Command, CommandError, cmd, flag, set};
use crate::subtitle_controls;
use crate::{Playback, TrackKind, valid_path};

impl Command {
    pub(super) fn subtitles_args(&self) -> Result<Vec<String>, CommandError> {
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
            _ => {
                return Err(CommandError::InvalidArgument {
                    what: "The requested value is out of range or non-finite",
                });
            }
        };
        Ok(args)
    }
}

pub(super) fn encoding(state: &Playback, encoding: &str) -> Result<Vec<Vec<String>>, CommandError> {
    if !subtitle_controls::SUBTITLE_ENCODING_CHOICES
        .iter()
        .any(|(_, code)| *code == encoding)
    {
        return Err(CommandError::InvalidArgument {
            what: "Unsupported subtitle encoding",
        });
    }
    let mut commands = vec![set("sub-codepage", encoding)];
    commands.extend(
        state
            .tracks
            .iter()
            .filter(|track| track.kind == TrackKind::Sub)
            .map(|track| vec!["sub-reload".into(), track.id.to_string()]),
    );
    Ok(commands)
}

pub(super) fn auto_load(
    preferences: &subtitle_controls::AutoLoadPreferences,
) -> Result<Vec<Vec<String>>, CommandError> {
    preferences.validate().map_err(CommandError::Preference)?;
    let mut commands = vec![set("sub-auto", preferences.mpv_matching_mode())];
    commands.push(set(
        "sub-file-paths",
        preferences.mpv_search_paths().unwrap_or_default(),
    ));
    Ok(commands)
}

pub(super) fn add(state: &Playback, path: &str) -> Result<Vec<Vec<String>>, CommandError> {
    if !valid_path(path) {
        return Err(CommandError::InvalidArgument {
            what: "Invalid subtitle path",
        });
    }
    // Reload an existing external file to avoid duplicate tracks, as IINA does.
    Ok(
        if let Some(track) = state.tracks.iter().find(|track| {
            track.kind == TrackKind::Sub
                && track
                    .external_filename
                    .as_deref()
                    .is_some_and(|existing| same_external_path(existing, path))
        }) {
            vec![vec!["sub-reload".into(), track.id.to_string()]]
        } else {
            vec![cmd(["sub-add", path])]
        },
    )
}

pub(super) fn add_without_selection(
    state: &Playback,
    paths: &[String],
) -> Result<Vec<Vec<String>>, CommandError> {
    const MAX_SUBTITLES_PER_SCAN: usize = 64;
    if paths.len() > MAX_SUBTITLES_PER_SCAN {
        return Err(CommandError::InvalidArgument {
            what: "Subtitle search returned too many matching files",
        });
    }
    let mut added = Vec::<&str>::new();
    let mut commands = Vec::new();
    for path in paths {
        if !valid_path(path) {
            return Err(CommandError::InvalidArgument {
                what: "Invalid subtitle path",
            });
        }
        let exists = state.tracks.iter().any(|track| {
            track.kind == TrackKind::Sub
                && track
                    .external_filename
                    .as_deref()
                    .is_some_and(|existing| same_external_path(existing, path))
        }) || added
            .iter()
            .any(|existing| same_external_path(existing, path));
        if !exists {
            added.push(path);
            commands.push(vec!["sub-add".into(), path.clone(), "auto".into()]);
        }
    }
    Ok(commands)
}

fn same_external_path(left: &str, right: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        fn identity(path: &str) -> (bool, &str) {
            if let Some(path) = path.strip_prefix(r"\\?\UNC\") {
                (true, path)
            } else if let Some(path) = path.strip_prefix(r"\\?\") {
                (false, path)
            } else if let Some(path) = path.strip_prefix(r"\\") {
                (true, path)
            } else {
                (false, path)
            }
        }
        // mpv may omit the verbatim prefix returned by Windows canonicalize.
        // Compare that spelling equivalently without filesystem I/O or treating
        // case-sensitive names, aliases or different volumes as the same file.
        identity(left) == identity(right)
    }
    #[cfg(not(target_os = "windows"))]
    {
        left == right
    }
}
