//! Player intent and observed state. The backend is the only playback clock.

pub mod advanced_preferences;
pub mod audio_preferences;
pub mod control_preferences;
pub mod crop;
pub mod crop_drag;
pub mod crop_geometry;
pub mod crop_session;
pub mod file_selection;
pub mod hdr_preferences;
pub mod key_bindings;
pub mod liquid_glass;
pub mod media_reference;
pub mod music_mode;
pub mod network_preferences;
pub mod online_subtitles;
pub mod osc_preferences;
pub mod pip_lifecycle;
pub mod pip_preferences;
pub mod playlist;
pub mod plugin;
pub mod screenshot;
pub mod speed_controls;
pub mod subtitle_color;
pub mod subtitle_controls;
pub mod thumbnail_preferences;
pub mod time_input;
mod track_labels;
pub mod video_controls;
pub mod video_reflection;
pub mod video_viewport;
pub mod window_aspect;
pub mod window_resize;

use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Load(String),
    LoadAtPosition {
        path: String,
        position: f64,
    },
    OpenFiles(Vec<String>),
    SetPauseWhenOpen(bool),
    SetKeepOpenOnFileEnd(bool),
    SetResumeLastPosition(bool),
    SetPaused(bool),
    TogglePause,
    /// A validated mpv input.conf action from the active key binding profile.
    MpvInputAction(String),
    Stop,
    Seek {
        seconds: f64,
        absolute: bool,
    },
    SeekWithType {
        seconds: f64,
        seek_type: control_preferences::SeekType,
    },
    Volume(f64),
    AdjustVolume {
        delta: f64,
        boost: bool,
    },
    SetVolumeBoost(bool),
    ToggleMute,
    Speed(f64),
    MultiplySpeed(f64),
    FrameStep,
    FrameBackStep,
    CaptureFrame,
    Screenshot {
        directory: String,
        content: screenshot::Content,
        encoding: screenshot::Encoding,
    },
    ScreenshotToFile {
        path: String,
        content: screenshot::Content,
    },
    VideoAspect(video_controls::AspectRatio),
    VideoRotation(video_controls::Rotation),
    Deinterlace(bool),
    ToggleDeinterlace,
    VideoCrop(Option<crop::Preset>),
    VideoCropRectangle(crop::Rectangle),
    ToggleVideoReflection(video_reflection::Axis),
    VideoViewport(Option<video_viewport::Viewport>),
    VideoReflection {
        axis: video_reflection::Axis,
        enabled: bool,
    },
    EnterCrop(crop_session::Session),
    ExitCrop {
        session: crop_session::Session,
        rectangle: Option<crop::Rectangle>,
    },
    HardwareDecoding(video_controls::HardwareDecoder),
    VideoThreads(u32),
    SetToneMapping(hdr_preferences::Settings),
    SetHdrEnabled(bool),
    SubtitleAssOverride {
        lane: subtitle_controls::SubtitleLane,
        level: subtitle_controls::AssOverrideLevel,
    },
    SubtitleAlignX(subtitle_controls::HorizontalAlignment),
    SubtitleAlignY(subtitle_controls::VerticalAlignment),
    SubtitleLetterboxMargins(bool),
    SubtitleScaleWithWindow(bool),
    SubtitleMarginX(f64),
    SubtitleMarginY(f64),
    SubtitlePreferredLanguages(String),
    SubtitleEncoding(String),
    SubtitleAutoLoad(subtitle_controls::AutoLoadPreferences),
    NetworkCache(network_preferences::Cache),
    NetworkOnlineVideos(network_preferences::OnlineVideos),
    VideoEqualizer {
        option: video_controls::Equalizer,
        value: i64,
    },
    PlaylistNext,
    PlaylistPrevious,
    TogglePlaylistShuffle,
    SortPlaylist(playlist::PlaylistSort),
    SelectTrack {
        kind: TrackKind,
        id: Option<i64>,
    },
    CycleTrack(TrackKind),
    SelectSecondarySubtitle(Option<i64>),
    SelectChapter(usize),
    AdjustChapter(i32),
    SelectPlaylistEntry(usize),
    SelectPlaylistId(i64),
    RemovePlaylistId(i64),
    RemovePlaylistIds(Vec<i64>),
    MovePlaylistId {
        id: i64,
        direction: i32,
    },
    AppendFiles(Vec<String>),
    InsertPlaylistFiles {
        paths: Vec<String>,
        index: usize,
    },
    InsertPlaylistFilesAroundCurrent {
        before: Vec<String>,
        after: Vec<String>,
    },
    RemovePlaylistEntry(usize),
    MovePlaylistEntry {
        from: usize,
        to: usize,
    },
    ClearPlaylist,
    AddSubtitle(String),
    AddSubtitlesWithoutSelection(Vec<String>),
    AddAudio(String),
    AudioDelay(f64),
    AdjustAudioDelay(f64),
    SubtitleDelay(f64),
    SecondarySubtitleDelay(f64),
    SecondarySubtitlePosition(f64),
    SubtitlePosition(f64),
    SubtitleScale(f64),
    SubtitleFontSize(f64),
    SubtitleFont(String),
    SubtitleOutlineSize(f64),
    SubtitleTextColor(subtitle_color::SubtitleColor),
    SubtitleBorderColor(subtitle_color::SubtitleColor),
    SubtitleShadowColor(subtitle_color::SubtitleColor),
    SubtitleBold(bool),
    SubtitleItalic(bool),
    SubtitleBlur(f64),
    SubtitleSpacing(f64),
    SubtitleShadowOffset(f64),
    AdjustSubtitleDelay(f64),
    AdjustSubtitleScale(f64),
    AdjustSubtitlePosition(f64),
    SubtitleSeek(i32),
    ToggleSubtitleVisibility,
    ToggleSecondarySubtitleVisibility,
    CycleLoop,
    ToggleFileLoop,
    CycleAbLoop,
    SetAbLoop {
        a: Option<f64>,
        b: Option<f64>,
    },
    SetAbLoopPoint {
        point: AbLoopPoint,
        value: Option<f64>,
    },
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbLoopPoint {
    A,
    B,
}

impl Command {
    pub fn mpv_args(&self) -> Result<Vec<String>, String> {
        let args: Vec<String> = match self {
            Self::Load(path) if !path.is_empty() && !path.contains('\0') => {
                vec!["loadfile".into(), path.clone(), "replace".into()]
            }
            Self::Load(_) => return Err("The media path is empty or contains a NUL byte".into()),
            Self::LoadAtPosition { path, position }
                if valid_path(path) && position.is_finite() && *position >= 0.0 =>
            {
                vec![
                    "loadfile".into(),
                    path.clone(),
                    "replace".into(),
                    "-1".into(),
                    format!("start={position}"),
                ]
            }
            Self::LoadAtPosition { .. } => {
                return Err("The resume file or position is invalid".into());
            }
            Self::SetPaused(paused) => vec![
                "set".into(),
                "pause".into(),
                if *paused { "yes" } else { "no" }.into(),
            ],
            Self::TogglePause => vec!["cycle".into(), "pause".into()],
            Self::SetKeepOpenOnFileEnd(enabled) => vec![
                "set".into(),
                "keep-open".into(),
                if *enabled { "yes" } else { "no" }.into(),
            ],
            Self::MpvInputAction(action) => key_bindings::mpv_action_args(action)?,
            Self::Stop => vec!["stop".into()],
            Self::Seek { seconds, absolute } if seconds.is_finite() => vec![
                "seek".into(),
                seconds.to_string(),
                if *absolute {
                    "absolute+exact"
                } else {
                    "relative"
                }
                .into(),
            ],
            Self::SeekWithType { seconds, seek_type } if seconds.is_finite() => vec![
                "seek".into(),
                seconds.to_string(),
                match seek_type {
                    control_preferences::SeekType::Keyframe
                    | control_preferences::SeekType::Auto => "relative",
                    control_preferences::SeekType::Exact => "relative+exact",
                }
                .into(),
            ],
            Self::Volume(value) if value.is_finite() && (0.0..=200.0).contains(value) => {
                vec!["set".into(), "volume".into(), value.to_string()]
            }
            // The reference's custom field has no upper bound. Submit finite
            // requests above the engine limit so native rejection remains an
            // observed failure instead of inventing a narrower input contract.
            Self::Speed(value) if value.is_finite() && *value >= 0.01 => {
                vec!["set".into(), "speed".into(), value.to_string()]
            }
            Self::MultiplySpeed(factor) if factor.is_finite() && *factor > 0.0 => {
                vec!["multiply".into(), "speed".into(), factor.to_string()]
            }
            Self::ToggleMute => vec!["cycle".into(), "mute".into()],
            Self::FrameStep => vec!["frame-step".into()],
            Self::FrameBackStep => vec!["frame-back-step".into()],
            Self::CaptureFrame => vec!["screenshot-raw".into(), "video".into(), "rgba".into()],
            Self::ScreenshotToFile { path, content } if valid_path(path) => vec![
                "screenshot-to-file".into(),
                path.clone(),
                content.native_value().into(),
            ],
            Self::PlaylistNext => vec!["playlist-next".into(), "weak".into()],
            Self::PlaylistPrevious => vec!["playlist-prev".into(), "weak".into()],
            Self::SelectTrack { kind, id } if id.is_none_or(|id| id > 0) => vec![
                "set".into(),
                kind.property().into(),
                id.map_or_else(|| "no".into(), |id| id.to_string()),
            ],
            Self::CycleTrack(kind) => {
                // Native cycling owns track ordering and the disabled sentinel;
                // a catalog snapshot can lag external-track insertion/removal.
                vec!["cycle".into(), kind.property().into()]
            }
            Self::VideoAspect(aspect) => vec![
                "set".into(),
                "video-aspect-override".into(),
                aspect.native_value().into(),
            ],
            // Read the native chain at execution; rapid activations must not
            // invert the same stale observed state twice.
            Self::ToggleVideoReflection(axis) => vec!["vf".into(), "toggle".into(), axis.filter()],
            Self::VideoRotation(rotation) => vec![
                "set".into(),
                "video-rotate".into(),
                rotation.degrees().to_string(),
            ],
            Self::Deinterlace(enabled) => vec![
                "set".into(),
                "deinterlace".into(),
                if *enabled { "yes" } else { "no" }.into(),
            ],
            // Limit the native cycle to the reference's two states; newer mpv
            // also offers auto, which is not a third state in IINA's checkbox.
            Self::ToggleDeinterlace => vec![
                "cycle-values".into(),
                "deinterlace".into(),
                "no".into(),
                "yes".into(),
            ],
            Self::HardwareDecoding(mode) => {
                vec!["set".into(), "hwdec".into(), mode.native_value().into()]
            }
            Self::SubtitleAssOverride { lane, level } => vec![
                "set".into(),
                match lane {
                    subtitle_controls::SubtitleLane::Primary => "sub-ass-override",
                    subtitle_controls::SubtitleLane::Secondary => "secondary-sub-ass-override",
                }
                .into(),
                level.mpv_value().into(),
            ],
            Self::VideoEqualizer { option, value } if (-100..=100).contains(value) => {
                vec!["set".into(), option.property().into(), value.to_string()]
            }
            Self::SelectSecondarySubtitle(id) if id.is_none_or(|id| id > 0) => vec![
                "set".into(),
                "secondary-sid".into(),
                id.map_or_else(|| "no".into(), |id| id.to_string()),
            ],
            Self::SelectChapter(index) => vec!["set".into(), "chapter".into(), index.to_string()],
            Self::AdjustChapter(delta) => {
                // Relative navigation keeps native restart-threshold and boundary
                // semantics; selecting a sidebar chapter intentionally resumes.
                vec!["add".into(), "chapter".into(), delta.to_string()]
            }
            Self::SelectPlaylistEntry(index) => {
                // Setting playlist-pos does not reload an already selected entry.
                vec!["playlist-play-index".into(), index.to_string()]
            }
            Self::RemovePlaylistEntry(index) => vec!["playlist-remove".into(), index.to_string()],
            Self::MovePlaylistEntry { from, to } => {
                vec!["playlist-move".into(), from.to_string(), to.to_string()]
            }
            Self::ClearPlaylist => vec!["playlist-clear".into()],
            Self::AddSubtitle(path) | Self::AddAudio(path) if valid_path(path) => {
                let args = vec![
                    if matches!(self, Self::AddAudio(_)) {
                        "audio-add"
                    } else {
                        "sub-add"
                    }
                    .into(),
                    path.clone(),
                ];
                args
            }
            Self::AudioDelay(value) | Self::SubtitleDelay(value) if value.is_finite() => vec![
                "set".into(),
                if matches!(self, Self::AudioDelay(_)) {
                    "audio-delay"
                } else {
                    "sub-delay"
                }
                .into(),
                value.to_string(),
            ],
            Self::ToggleSubtitleVisibility => vec!["cycle".into(), "sub-visibility".into()],
            Self::SubtitleFont(font) if subtitle_controls::valid_font_name(font) => {
                vec!["set".into(), "sub-font".into(), font.clone()]
            }
            Self::SubtitlePreferredLanguages(languages)
                if languages.len() <= 256 && !languages.chars().any(char::is_control) =>
            {
                vec!["set".into(), "slang".into(), languages.clone()]
            }
            Self::SubtitleBold(value) => vec![
                "set".into(),
                "sub-bold".into(),
                if *value { "yes" } else { "no" }.into(),
            ],
            Self::SubtitleItalic(value) => vec![
                "set".into(),
                "sub-italic".into(),
                if *value { "yes" } else { "no" }.into(),
            ],
            Self::SubtitleBlur(value) if value.is_finite() && (0.0..=20.0).contains(value) => {
                vec!["set".into(), "sub-blur".into(), value.to_string()]
            }
            Self::SubtitleSpacing(value) if value.is_finite() => {
                vec!["set".into(), "sub-spacing".into(), value.to_string()]
            }
            Self::SubtitleShadowOffset(value) if value.is_finite() && *value >= 0.0 => {
                vec!["set".into(), "sub-shadow-offset".into(), value.to_string()]
            }
            Self::SubtitleTextColor(color)
            | Self::SubtitleBorderColor(color)
            | Self::SubtitleShadowColor(color) => {
                let option = match self {
                    Self::SubtitleTextColor(_) => "sub-color",
                    Self::SubtitleBorderColor(_) => "sub-outline-color",
                    _ => "sub-back-color",
                };
                vec!["set".into(), option.into(), color.mpv_value()]
            }
            Self::SubtitleOutlineSize(value) if value.is_finite() && *value >= 0.0 => {
                vec!["set".into(), "sub-outline-size".into(), value.to_string()]
            }
            Self::SubtitleFontSize(value) if value.is_finite() && *value > 0.0 => {
                vec!["set".into(), "sub-font-size".into(), value.to_string()]
            }
            Self::SubtitleScale(value) if value.is_finite() && (0.0..=100.0).contains(value) => {
                vec!["set".into(), "sub-scale".into(), value.to_string()]
            }
            Self::SecondarySubtitleDelay(value) if value.is_finite() => vec![
                "set".into(),
                "secondary-sub-delay".into(),
                value.to_string(),
            ],
            Self::SubtitlePosition(value) | Self::SecondarySubtitlePosition(value)
                if value.is_finite() && (0.0..=150.0).contains(value) =>
            {
                vec![
                    "set".into(),
                    if matches!(self, Self::SubtitlePosition(_)) {
                        "sub-pos"
                    } else {
                        "secondary-sub-pos"
                    }
                    .into(),
                    value.to_string(),
                ]
            }
            Self::SubtitleAlignX(alignment) => vec![
                "set".into(),
                "sub-align-x".into(),
                alignment.mpv_value().into(),
            ],
            Self::SubtitleAlignY(alignment) => vec![
                "set".into(),
                "sub-align-y".into(),
                alignment.mpv_value().into(),
            ],
            Self::SubtitleScaleWithWindow(enabled) => vec![
                "set".into(),
                "sub-scale-by-window".into(),
                if *enabled { "yes" } else { "no" }.into(),
            ],
            Self::SubtitleMarginX(value) | Self::SubtitleMarginY(value)
                if value.is_finite() && (0.0..=2_000_000_000.0).contains(value) =>
            {
                vec![
                    "set".into(),
                    if matches!(self, Self::SubtitleMarginX(_)) {
                        "sub-margin-x"
                    } else {
                        "sub-margin-y"
                    }
                    .into(),
                    value.to_string(),
                ]
            }
            Self::ToggleSecondarySubtitleVisibility => {
                vec!["cycle".into(), "secondary-sub-visibility".into()]
            }
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
            Self::CycleAbLoop => vec!["ab-loop".into()],
            Self::ToggleFileLoop => {
                // This shortcut changes only file repeat; a playlist repeat
                // setting survives while file repeat temporarily takes priority.
                vec![
                    "cycle-values".into(),
                    "loop-file".into(),
                    "inf".into(),
                    "no".into(),
                ]
            }
            Self::OpenFiles(_)
            | Self::SetPauseWhenOpen(_)
            | Self::SetResumeLastPosition(_)
            | Self::Screenshot { .. }
            | Self::VideoCrop(_)
            | Self::VideoCropRectangle(_)
            | Self::VideoViewport(_)
            | Self::VideoReflection { .. }
            | Self::EnterCrop(_)
            | Self::ExitCrop { .. }
            | Self::AdjustVolume { .. }
            | Self::SetVolumeBoost(_)
            | Self::AppendFiles(_)
            | Self::AddSubtitlesWithoutSelection(_)
            | Self::InsertPlaylistFiles { .. }
            | Self::SubtitleLetterboxMargins(_)
            | Self::SubtitleEncoding(_)
            | Self::SubtitleAutoLoad(_)
            | Self::CycleLoop
            | Self::SetAbLoop { .. }
            | Self::SetAbLoopPoint { .. }
            | Self::SetToneMapping(_)
            | Self::SetHdrEnabled(_)
            | Self::VideoThreads(_)
            | Self::SelectPlaylistId(_)
            | Self::RemovePlaylistId(_)
            | Self::RemovePlaylistIds(_)
            | Self::MovePlaylistId { .. } => {
                return Err("This intent requires a command plan".into());
            }
            Self::Shutdown => return Err("Shutdown is a lifecycle command".into()),
            _ => return Err("The requested value is out of range or non-finite".into()),
        };
        Ok(args)
    }

    pub fn plan(&self, state: &Playback) -> Result<CommandPlan, String> {
        let mut resume_on_load = false;
        if matches!(self, Self::CaptureFrame) && state.phase != Phase::Paused {
            return Err("Text recognition is available only while video is paused".into());
        }
        let commands = match self {
            Self::MpvInputAction(action) => key_bindings::mpv_action_commands(action)?,
            Self::SubtitleLetterboxMargins(enabled) => {
                let value = if *enabled { "yes" } else { "no" };
                vec![
                    vec!["set".into(), "sub-use-margins".into(), value.into()],
                    vec!["set".into(), "sub-ass-force-margins".into(), value.into()],
                ]
            }
            Self::SubtitleEncoding(encoding) => {
                if !subtitle_controls::SUBTITLE_ENCODING_CHOICES
                    .iter()
                    .any(|(_, code)| *code == encoding)
                {
                    return Err("Unsupported subtitle encoding".into());
                }
                let mut commands =
                    vec![vec!["set".into(), "sub-codepage".into(), encoding.clone()]];
                commands.extend(
                    state
                        .tracks
                        .iter()
                        .filter(|track| track.kind == TrackKind::Sub)
                        .map(|track| vec!["sub-reload".into(), track.id.to_string()]),
                );
                commands
            }
            Self::SubtitleAutoLoad(preferences) => {
                preferences.validate().map_err(str::to_owned)?;
                let mut commands = vec![vec![
                    "set".into(),
                    "sub-auto".into(),
                    preferences.mpv_matching_mode().into(),
                ]];
                commands.push(vec![
                    "set".into(),
                    "sub-file-paths".into(),
                    preferences.mpv_search_paths().unwrap_or_default(),
                ]);
                commands
            }
            Self::TogglePause if state.phase == Phase::Ended => {
                // mpv stays at EOF after toggling its pause flag. Treat Play on
                // the ended state as replay so the visible Play control works.
                vec![
                    vec!["seek".into(), "0".into(), "absolute+exact".into()],
                    vec!["set".into(), "pause".into(), "no".into()],
                ]
            }
            Self::SetPaused(false) if state.phase == Phase::Ended => {
                // Play from EOF must seek first; setting pause=no alone does not replay.
                vec![
                    vec!["seek".into(), "0".into(), "absolute+exact".into()],
                    vec!["set".into(), "pause".into(), "no".into()],
                ]
            }
            Self::NetworkCache(preferences) => preferences.commands()?,
            Self::NetworkOnlineVideos(preferences) => preferences
                .engine_options()?
                .into_iter()
                .map(|(name, value)| vec!["set".into(), name, value])
                .collect(),
            Self::Screenshot {
                directory,
                content,
                encoding,
            } => {
                if !valid_path(directory) || encoding.template.contains('\0') {
                    return Err(
                        "Screenshot directory is empty or settings contain a NUL byte".into(),
                    );
                }
                vec![
                    vec![
                        "set".into(),
                        "screenshot-format".into(),
                        encoding.format.native_value().into(),
                    ],
                    vec![
                        "set".into(),
                        "screenshot-template".into(),
                        encoding.template.clone(),
                    ],
                    // Match the reference's JXL software path without changing
                    // screenshot conversion policy for the other formats.
                    vec![
                        "set".into(),
                        "screenshot-sw".into(),
                        if encoding.format == screenshot::Format::Jxl {
                            "yes"
                        } else {
                            "no"
                        }
                        .into(),
                    ],
                    // Keep the native trailing separator: mixed '/' in a
                    // verbatim Windows path is rejected by screenshot writing.
                    vec![
                        "set".into(),
                        "screenshot-dir".into(),
                        std::path::Path::new(directory)
                            .join("")
                            .to_string_lossy()
                            .into_owned(),
                    ],
                    vec!["screenshot".into(), content.native_value().into()],
                ]
            }
            Self::VideoViewport(viewport) => video_viewport::Viewport::commands(*viewport),
            Self::VideoReflection { axis, enabled } => axis.commands(*enabled, state)?,
            Self::EnterCrop(session) => session.enter_commands(state)?,
            Self::ExitCrop { session, rectangle } => session.exit_commands(state, *rectangle)?,
            Self::VideoCropRectangle(rectangle) => {
                let source = state
                    .video
                    .as_ref()
                    .ok_or("Source video dimensions are unavailable")?;
                if rectangle.x == 0
                    && rectangle.y == 0
                    && rectangle.width != 0
                    && rectangle.height != 0
                    && rectangle.width == source.w
                    && rectangle.height == source.h
                {
                    return Self::VideoCrop(None).plan(state);
                }
                vec![vec![
                    "vf".into(),
                    "add".into(),
                    rectangle.editor_filter(source.w, source.h)?,
                ]]
            }
            Self::VideoCrop(preset) => match preset {
                Some(preset) => {
                    let source = state
                        .video
                        .as_ref()
                        .ok_or("Source video dimensions are unavailable")?;
                    vec![vec![
                        "vf".into(),
                        "add".into(),
                        preset.filter(source.w, source.h)?,
                    ]]
                }
                None => {
                    let filters = state
                        .video_filters
                        .as_ref()
                        .ok_or("Video filters are unavailable")?;
                    if filters
                        .iter()
                        .any(|filter| filter.label.as_deref() == Some(crop::FILTER_LABEL))
                    {
                        vec![vec![
                            "vf".into(),
                            "remove".into(),
                            format!("@{}", crop::FILTER_LABEL),
                        ]]
                    } else {
                        Vec::new()
                    }
                }
            },
            Self::AdjustVolume { delta, boost } => {
                if !delta.is_finite() {
                    return Err("The volume adjustment must be finite".into());
                }
                volume_adjustment(*delta, *boost)
            }
            Self::SetVolumeBoost(boost) => volume_adjustment(0.0, *boost),
            Self::SetPauseWhenOpen(_) => Vec::new(),
            Self::SetResumeLastPosition(enabled) => {
                let value = if *enabled { "yes" } else { "no" };
                vec![
                    vec!["set".into(), "save-position-on-quit".into(), value.into()],
                    vec!["set".into(), "resume-playback".into(), value.into()],
                ]
            }
            Self::SetAbLoop { a, b } => {
                let point = |value: Option<f64>| -> Result<String, String> {
                    match value {
                        None => Ok("no".into()),
                        Some(value) if value.is_finite() && value >= 0.0 => {
                            // The pinned IINA contract preserves a point at the
                            // beginning using its minimum nonzero loop timestamp.
                            Ok(value.max(0.000001).to_string())
                        }
                        _ => Err("Invalid A–B loop point".into()),
                    }
                };
                let (a, b) = (point(*a)?, point(*b)?);
                vec![
                    vec!["set".into(), "ab-loop-a".into(), a],
                    vec!["set".into(), "ab-loop-b".into(), b],
                ]
            }
            Self::SetAbLoopPoint { point, value } => {
                let name = match point {
                    AbLoopPoint::A => "ab-loop-a",
                    AbLoopPoint::B => "ab-loop-b",
                };
                let value = match value {
                    None => "no".to_owned(),
                    Some(value) if value.is_finite() && *value >= 0.0 => {
                        value.max(0.000001).to_string()
                    }
                    _ => return Err("Invalid A–B loop point".into()),
                };
                vec![vec!["set".into(), name.into(), value]]
            }
            Self::SetToneMapping(settings) => settings.engine_commands()?,
            Self::SetHdrEnabled(enabled) => {
                let primaries = if *enabled {
                    let source = state
                        .video
                        .as_ref()
                        .or(state.video_target.as_ref())
                        .or(state.video_output.as_ref())
                        .filter(|video| video.is_hdr())
                        .ok_or("HDR output is available only for a recognized HDR source")?;
                    source.primaries.as_deref()
                } else {
                    None
                };
                hdr_preferences::output_commands(*enabled, primaries)?
            }
            Self::VideoThreads(threads) if *threads <= i32::MAX as u32 => vec![vec![
                "set".into(),
                "vd-lavc-threads".into(),
                threads.to_string(),
            ]],
            Self::VideoThreads(_) => {
                return Err("Video decoder threads must be between 0 and 2,147,483,647".into());
            }
            Self::TogglePlaylistShuffle => vec![vec![
                if state.playlist_shuffled {
                    "playlist-unshuffle"
                } else {
                    "playlist-shuffle"
                }
                .into(),
            ]],
            Self::SortPlaylist(sort) => playlist_reorder_commands(&state.playlist, *sort)?,
            Self::SelectPlaylistId(id)
            | Self::RemovePlaylistId(id)
            | Self::MovePlaylistId { id, .. } => {
                // Stable native IDs survive inserts/reorders while a UI intent is queued.
                let index = state
                    .playlist
                    .iter()
                    .position(|entry| entry.id == *id)
                    .ok_or("The playlist entry is no longer available")?;
                let command = match self {
                    Self::SelectPlaylistId(_) => Self::SelectPlaylistEntry(index),
                    Self::RemovePlaylistId(_) => Self::RemovePlaylistEntry(index),
                    Self::MovePlaylistId { direction: -1, .. } if index > 0 => {
                        Self::MovePlaylistEntry {
                            from: index,
                            to: index - 1,
                        }
                    }
                    Self::MovePlaylistId { direction: 1, .. }
                        if index + 1 < state.playlist.len() =>
                    {
                        Self::MovePlaylistEntry {
                            from: index,
                            to: index + 2,
                        }
                    }
                    Self::MovePlaylistId { .. } => {
                        return Err("The playlist entry cannot move in that direction".into());
                    }
                    _ => unreachable!(),
                };
                return command.plan(state);
            }
            Self::RemovePlaylistIds(ids) => {
                let mut indices = Vec::with_capacity(ids.len());
                for id in ids {
                    let Some(index) = state.playlist.iter().position(|entry| entry.id == *id)
                    else {
                        return Err("A selected playlist entry is no longer available".into());
                    };
                    indices.push(index);
                }
                indices.sort_unstable();
                if indices.windows(2).any(|pair| pair[0] == pair[1]) {
                    return Err("A playlist entry was selected more than once".into());
                }
                // IINA removes ascending index sets while accounting for each prior
                // removal; stable IDs are resolved before any native row can shift.
                indices
                    .into_iter()
                    .enumerate()
                    .map(|(removed, index)| {
                        vec!["playlist-remove".into(), (index - removed).to_string()]
                    })
                    .collect()
            }
            Self::OpenFiles(paths) | Self::AppendFiles(paths) => {
                if paths.is_empty() || paths.iter().any(|path| !valid_path(path)) {
                    return Err("No valid files were supplied".into());
                }
                let mut commands: Vec<Vec<String>> = paths
                    .iter()
                    .map(|path| vec!["loadfile".into(), path.clone(), "append".into()])
                    .collect();
                if matches!(self, Self::OpenFiles(_)) {
                    // PlayerCore.openURLs opens the first playable item, then appends
                    // the remainder. Validate everything before replacing old media.
                    commands[0][2] = "replace".into();
                    commands.insert(
                        0,
                        vec![
                            "set".into(),
                            "pause".into(),
                            if state.pause_when_open { "yes" } else { "no" }.into(),
                        ],
                    );
                    // HDR overrides target the current source's primaries. Clear
                    // them before replacement so the next source inherits auto/ICC.
                    let hdr_reset = hdr_preferences::output_commands(false, None)?;
                    commands.splice(0..0, hdr_reset);
                }
                commands
            }
            Self::InsertPlaylistFiles { paths, index } => {
                if paths.is_empty()
                    || paths.iter().any(|path| !valid_path(path))
                    || *index > state.playlist.len()
                {
                    return Err("Playlist items or insertion position are invalid".into());
                }
                let first_new = state.playlist.len();
                let mut commands = Vec::with_capacity(paths.len() * 2);
                for (offset, path) in paths.iter().enumerate() {
                    commands.push(vec!["loadfile".into(), path.clone(), "append".into()]);
                    let appended_index = first_new + offset;
                    let destination = index + offset;
                    if appended_index != destination {
                        commands.push(vec![
                            "playlist-move".into(),
                            appended_index.to_string(),
                            destination.to_string(),
                        ]);
                    }
                }
                commands
            }
            Self::InsertPlaylistFilesAroundCurrent { before, after } => {
                if before.is_empty() && after.is_empty()
                    || before.iter().chain(after).any(|path| !valid_path(path))
                {
                    return Err("No valid folder media files were supplied".into());
                }
                let current_index = state
                    .playlist
                    .iter()
                    .position(|entry| entry.current)
                    .ok_or_else(|| "The current playlist item is no longer available".to_owned())?;
                let first_new = state.playlist.len();
                let mut commands = Vec::with_capacity(before.len() * 2 + after.len());
                for (offset, path) in before.iter().enumerate() {
                    commands.push(vec!["loadfile".into(), path.clone(), "append".into()]);
                    let appended_index = first_new + offset;
                    let destination = current_index + offset;
                    if appended_index != destination {
                        commands.push(vec![
                            "playlist-move".into(),
                            appended_index.to_string(),
                            destination.to_string(),
                        ]);
                    }
                }
                commands.extend(
                    after
                        .iter()
                        .map(|path| vec!["loadfile".into(), path.clone(), "append".into()]),
                );
                commands
            }
            Self::Load(_) | Self::LoadAtPosition { .. } => {
                let mut commands = hdr_preferences::output_commands(false, None)?;
                commands.extend([
                    vec![
                        "set".into(),
                        "pause".into(),
                        if state.pause_when_open { "yes" } else { "no" }.into(),
                    ],
                    self.mpv_args()?,
                ]);
                commands
            }
            Self::SelectChapter(index) => {
                let chapter = state
                    .chapters
                    .get(*index)
                    .ok_or("The chapter is no longer available")?;
                if !chapter.time.is_finite() || chapter.time < 0.0 {
                    return Err("Invalid chapter time".into());
                }
                // PlayerCore.playChapter seeks to the observed time and always resumes.
                vec![
                    vec!["seek".into(), chapter.time.to_string(), "absolute".into()],
                    vec!["set".into(), "pause".into(), "no".into()],
                ]
            }
            Self::SelectPlaylistEntry(index) => {
                if *index >= state.playlist.len() {
                    return Err("The playlist entry is no longer available".into());
                }
                resume_on_load = true;
                vec![
                    vec!["set".into(), "pause".into(), "yes".into()],
                    self.mpv_args()?,
                ]
            }
            Self::PlaylistPrevious if state.playlist.first().is_some_and(|entry| entry.current) => {
                vec![vec!["seek".into(), "0".into(), "absolute".into()]]
            }
            Self::PlaylistNext | Self::PlaylistPrevious => {
                resume_on_load = true;
                vec![
                    vec!["set".into(), "pause".into(), "yes".into()],
                    self.mpv_args()?,
                ]
            }
            Self::RemovePlaylistEntry(index) if *index >= state.playlist.len() => {
                return Err("The playlist entry is no longer available".into());
            }
            Self::MovePlaylistEntry { from, to }
                if *from >= state.playlist.len() || *to > state.playlist.len() =>
            {
                return Err("The playlist entry is no longer available".into());
            }
            Self::AddSubtitle(path) => {
                if !valid_path(path) {
                    return Err("Invalid subtitle path".into());
                }
                // Reload an existing external file to avoid duplicate tracks, as IINA does.
                if let Some(track) = state.tracks.iter().find(|track| {
                    track.kind == TrackKind::Sub
                        && track
                            .external_filename
                            .as_deref()
                            .is_some_and(|existing| same_external_path(existing, path))
                }) {
                    vec![vec!["sub-reload".into(), track.id.to_string()]]
                } else {
                    vec![self.mpv_args()?]
                }
            }
            Self::AddSubtitlesWithoutSelection(paths) => {
                const MAX_SUBTITLES_PER_SCAN: usize = 64;
                if paths.len() > MAX_SUBTITLES_PER_SCAN {
                    return Err("Subtitle search returned too many matching files".into());
                }
                let mut added = Vec::<&str>::new();
                let mut commands = Vec::new();
                for path in paths {
                    if !valid_path(path) {
                        return Err("Invalid subtitle path".into());
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
                commands
            }
            Self::CycleLoop => match state.loop_mode().next() {
                LoopMode::Off => vec![
                    vec!["set".into(), "loop-playlist".into(), "no".into()],
                    vec!["set".into(), "loop-file".into(), "no".into()],
                ],
                LoopMode::File => vec![vec!["set".into(), "loop-file".into(), "inf".into()]],
                LoopMode::Playlist => vec![
                    vec!["set".into(), "loop-playlist".into(), "inf".into()],
                    vec!["set".into(), "loop-file".into(), "no".into()],
                ],
            },
            _ => vec![self.mpv_args()?],
        };
        Ok(CommandPlan {
            commands,
            resume_on_load,
        })
    }
}

fn valid_path(path: &str) -> bool {
    !path.is_empty() && !path.contains('\0')
}

fn volume_adjustment(delta: f64, boost: bool) -> Vec<Vec<String>> {
    // Native add clamps against the active range and uses the engine's current
    // volume. A zero adjustment also clamps when the user turns boost off.
    vec![
        vec![
            "set".into(),
            "volume-max".into(),
            if boost { "200" } else { "100" }.into(),
        ],
        vec!["add".into(), "volume".into(), delta.to_string()],
    ]
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

pub struct CommandPlan {
    pub commands: Vec<Vec<String>>,
    pub resume_on_load: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopMode {
    Off,
    File,
    Playlist,
}
impl LoopMode {
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::File,
            Self::File => Self::Playlist,
            Self::Playlist => Self::Off,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrackKind {
    Video,
    Audio,
    Sub,
}

impl TrackKind {
    pub fn property(self) -> &'static str {
        match self {
            Self::Video => "vid",
            Self::Audio => "aid",
            Self::Sub => "sid",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Track {
    pub id: i64,
    #[serde(rename = "type")]
    pub kind: TrackKind,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub codec: Option<String>,
    #[serde(default)]
    pub selected: bool,
    #[serde(default, rename = "main-selection")]
    pub main_selection: Option<i64>,
    #[serde(default)]
    pub external: bool,
    #[serde(default)]
    pub albumart: bool,
    #[serde(default, rename = "external-filename")]
    pub external_filename: Option<String>,
    #[serde(default, rename = "default")]
    pub is_default: bool,
    #[serde(default, rename = "demux-channel-count")]
    pub channels: Option<i64>,
    #[serde(default, rename = "demux-samplerate")]
    pub sample_rate: Option<i64>,
    #[serde(default, rename = "demux-w")]
    pub width: Option<i64>,
    #[serde(default, rename = "demux-h")]
    pub height: Option<i64>,
    #[serde(default, rename = "demux-fps")]
    pub frame_rate: Option<f64>,
}

impl Track {
    pub fn is_primary_selected(&self) -> bool {
        // Both subtitle lanes can be decoded simultaneously. `selected` alone
        // cannot identify which lane should be highlighted in the primary list.
        self.selected && self.main_selection == Some(0)
    }

    pub fn display_name(&self) -> String {
        // MPVTrack.readableString excludes the language in the sidebar; the UI
        // displays it separately. Preserve title and codec/channel/rate metadata.
        let mut info = self.codec.iter().cloned().collect::<Vec<_>>();
        match self.kind {
            TrackKind::Video => {
                // Demux dimensions identify the track; a crop or rotation must
                // not rewrite its catalog description using output dimensions.
                if let Some((width, height)) = self.width.zip(self.height) {
                    info.push(format!("{width}×{height}"));
                }
                if let Some(rate) = self.frame_rate {
                    info.push(format!("{}fps", track_labels::number(rate)));
                }
            }
            TrackKind::Audio => {
                if let Some(channels) = self.channels {
                    info.push(format!("{channels}ch"));
                }
                if let Some(rate) = self.sample_rate {
                    info.push(format!("{}kHz", track_labels::number(rate as f64 / 1000.0)));
                }
            }
            TrackKind::Sub => {}
        }
        if self.is_default {
            info.push("Default".into());
        }
        let title = self.title.as_deref().unwrap_or("No Title");
        if info.is_empty() {
            title.into()
        } else if title.is_empty() {
            format!("({})", info.join(", "))
        } else {
            format!("{title} ({})", info.join(", "))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Chapter {
    pub time: f64,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PlaylistEntry {
    pub id: i64,
    pub filename: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub current: bool,
    #[serde(default)]
    pub playing: bool,
}

fn playlist_reorder_commands(
    entries: &[PlaylistEntry],
    sort: playlist::PlaylistSort,
) -> Result<Vec<Vec<String>>, String> {
    let desired = playlist::sorted_ids(entries, sort);
    let mut current = entries.iter().map(|entry| entry.id).collect::<Vec<_>>();
    let mut commands = Vec::new();
    for (target, id) in desired.into_iter().enumerate() {
        let from = current
            .iter()
            .position(|candidate| *candidate == id)
            .ok_or("The playlist changed before sorting")?;
        if from == target {
            continue;
        }
        // mpv's destination is an insertion position before removal; increment
        // downward moves to leave the item at its requested final index.
        let destination = if target > from { target + 1 } else { target };
        commands.push(vec![
            "playlist-move".into(),
            from.to_string(),
            destination.to_string(),
        ]);
        let moved = current.remove(from);
        current.insert(target, moved);
    }
    Ok(commands)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Stopped,
    Loading,
    Playing,
    Paused,
    Buffering,
    Seeking,
    Ended,
    Failed,
}

/// A tightly packed RGBA snapshot copied from the native engine's event-owned buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
}

impl CapturedFrame {
    pub fn new(width: u32, height: u32, stride: usize, bytes: &[u8]) -> Result<Self, String> {
        const MAX_DIMENSION: u32 = 16_384;
        const MAX_BYTES: usize = 512 * 1024 * 1024;
        if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
            return Err("The captured frame dimensions are invalid".into());
        }
        let row_bytes = (width as usize)
            .checked_mul(4)
            .ok_or("The captured frame is too large")?;
        if stride < row_bytes {
            return Err("The captured frame row is shorter than its RGBA pixels".into());
        }
        let source_len = stride
            .checked_mul(height as usize)
            .filter(|length| *length <= MAX_BYTES)
            .ok_or("The captured frame is too large")?;
        if bytes.len() < source_len {
            return Err("The captured frame buffer is incomplete".into());
        }
        let packed_len = row_bytes
            .checked_mul(height as usize)
            .ok_or("The captured frame is too large")?;
        let mut rgba = Vec::with_capacity(packed_len);
        for row in bytes[..source_len]
            .chunks_exact(stride)
            .take(height as usize)
        {
            rgba.extend_from_slice(&row[..row_bytes]);
        }
        Ok(Self {
            width,
            height,
            rgba: rgba.into(),
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct Playback {
    /// File-opening policy is intent; `paused` remains native observed state.
    pub pause_when_open: bool,
    pub generation: u64,
    pub phase: Phase,
    pub position: Option<f64>,
    pub duration: Option<f64>,
    pub paused: bool,
    pub buffering: bool,
    pub seeking: bool,
    /// Completed native seeks; retained across coalesced UI snapshots.
    pub completed_seeks: u64,
    /// Completed seeks associated with application navigation commands. Decoder
    /// refresh seeks remain in completed_seeks without creating a seek OSD.
    pub completed_navigation_seeks: u64,
    pub seekable: bool,
    pub volume: f64,
    pub muted: bool,
    pub speed: f64,
    pub audio_output: Option<String>,
    pub audio_devices: Vec<audio_preferences::OutputDevice>,
    pub hardware_decoder: Option<String>,
    // Configured policy is separate from hwdec-current: unsupported codecs can
    // use software while the user's hardware-decoding switch remains enabled.
    pub hardware_decoding: Option<String>,
    pub video: Option<VideoParameters>,
    pub video_target: Option<VideoParameters>,
    pub video_output: Option<VideoParameters>,
    pub video_filters: Option<Vec<crop::VideoFilter>>,
    pub video_aspect_override: Option<f64>,
    /// Exact requested aspect, separate from the numeric backend result. The
    /// reference retains this selection even when native parsing rejects it.
    pub video_aspect: video_controls::AspectRatio,
    /// Worker-dispatched aspect requests, including repeats and native failures.
    pub aspect_requests: u64,
    pub video_rotation: Option<i64>,
    pub deinterlace: Option<bool>,
    /// Native automatic filter activity, distinct from the configured option.
    pub deinterlace_active: Option<bool>,
    pub video_frame_info: Option<VideoFrameInfo>,
    /// Native successful captures, retained through snapshot coalescing.
    pub completed_screenshots: u64,
    pub screenshot_file: Option<String>,
    /// Latest paused-frame snapshot for asynchronous OCR and selection overlays.
    pub captured_frame: Option<Arc<CapturedFrame>>,
    pub completed_frame_captures: u64,
    pub video_equalizer: [Option<i64>; 5],
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub error: Option<String>,
    pub tracks: Vec<Track>,
    pub chapters: Vec<Chapter>,
    pub playlist: Vec<PlaylistEntry>,
    /// libmpv does not expose whether its native shuffle order is active.
    pub playlist_shuffled: bool,
    pub audio_delay: f64,
    pub subtitle_delay: f64,
    pub subtitle_scale: Option<f64>,
    pub subtitle_font_size: Option<f64>,
    pub subtitle_font: Option<String>,
    pub subtitle_outline_size: Option<f64>,
    pub subtitle_text_color: Option<subtitle_color::SubtitleColor>,
    pub subtitle_border_color: Option<subtitle_color::SubtitleColor>,
    pub subtitle_shadow_color: Option<subtitle_color::SubtitleColor>,
    pub subtitle_bold: Option<bool>,
    pub subtitle_italic: Option<bool>,
    pub subtitle_blur: Option<f64>,
    pub subtitle_spacing: Option<f64>,
    pub subtitle_shadow_offset: Option<f64>,
    pub subtitle_position: Option<f64>,
    pub subtitle_visible: bool,
    pub secondary_subtitle_visible: Option<bool>,
    pub secondary_subtitle_delay: Option<f64>,
    pub secondary_subtitle_position: Option<f64>,
    pub subtitle_text: Option<String>,
    pub secondary_subtitle_text: Option<String>,
    pub loop_file: Option<String>,
    pub loop_playlist: Option<String>,
    pub ab_loop_a: Option<f64>,
    pub ab_loop_b: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubtitleStyleAvailability {
    pub text_controls: bool,
    pub ass_warning: bool,
}

impl Playback {
    pub fn audio_status(&self) -> music_mode::AudioStatus {
        if self
            .playlist
            .iter()
            .find(|entry| entry.current)
            .is_some_and(|entry| entry.filename.contains("://"))
        {
            return music_mode::AudioStatus::Video;
        }
        let video_tracks = self
            .tracks
            .iter()
            .filter(|track| track.kind == TrackKind::Video)
            .collect::<Vec<_>>();
        let has_audio_track = self
            .tracks
            .iter()
            .any(|track| track.kind == TrackKind::Audio);
        if video_tracks.is_empty() && !has_audio_track {
            return music_mode::AudioStatus::Unknown;
        }
        if video_tracks.iter().all(|track| track.albumart) {
            music_mode::AudioStatus::Audio
        } else {
            music_mode::AudioStatus::Video
        }
    }

    pub fn subtitle_style_availability(&self) -> SubtitleStyleAvailability {
        let mut any_selected = false;
        let mut any_text = false;
        let mut any_ass = false;
        for track in self.tracks.iter().filter(|track| {
            track.kind == TrackKind::Sub
                && track.selected
                && matches!(track.main_selection, Some(0 | 1))
        }) {
            any_selected = true;
            // Match the pinned pane's codec classification, including unknown
            // codecs as text, rather than guessing from filename extensions.
            any_text |= !matches!(
                track.codec.as_deref(),
                Some("hdmv_pgs_subtitle" | "dvb_subtitle")
            );
            any_ass |= track.codec.as_deref() == Some("ass");
        }
        SubtitleStyleAvailability {
            text_controls: !any_selected || any_text,
            ass_warning: any_ass,
        }
    }
    pub fn loop_mode(&self) -> LoopMode {
        fn active(value: &Option<String>) -> bool {
            value.as_deref().is_some_and(|value| {
                value == "inf"
                    || value == "force"
                    || value.parse::<i64>().is_ok_and(|count| count != 0)
            })
        }
        if active(&self.loop_file) {
            LoopMode::File
        } else if active(&self.loop_playlist) {
            LoopMode::Playlist
        } else {
            LoopMode::Off
        }
    }
    pub fn begin_load(&mut self) {
        self.generation += 1;
        self.phase = Phase::Loading;
        self.position = None;
        self.duration = None;
        self.seekable = false;
        self.ab_loop_a = None;
        self.ab_loop_b = None;
        self.error = None;
        // Track IDs belong to one file; never offer a stale selector during replacement.
        self.tracks.clear();
        self.chapters.clear();
        self.subtitle_text = None;
        self.secondary_subtitle_text = None;
        self.video = None;
        self.artist = None;
        self.album = None;
        self.video_target = None;
        self.video_output = None;
        self.deinterlace_active = None;
        self.video_frame_info = None;
        self.screenshot_file = None;
        self.captured_frame = None;
    }

    pub fn reconcile(&mut self) {
        // A paused stream may still be caching: retain both facts, show buffering first.
        if matches!(
            self.phase,
            Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Seeking
        ) {
            self.phase = if self.buffering {
                Phase::Buffering
            } else if self.seeking {
                Phase::Seeking
            } else if self.paused {
                Phase::Paused
            } else {
                Phase::Playing
            };
        }
    }
}

/// Effective source parameters, not the renderer's output/display color space.
/// mpv can publish only container dimensions before a decoded frame exists.
/// Approximate current-frame flags from mpv, separate from source parameters.
/// They can lag presentation; pixel validation must not rely on these alone.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct VideoFrameInfo {
    pub interlaced: Option<bool>,
    #[serde(rename = "tff")]
    pub top_field_first: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct VideoParameters {
    pub pixelformat: Option<String>,
    pub w: u32,
    pub h: u32,
    pub dw: Option<u32>,
    pub dh: Option<u32>,
    pub rotate: Option<i64>,
    pub colormatrix: Option<String>,
    pub colorlevels: Option<String>,
    pub primaries: Option<String>,
    pub gamma: Option<String>,
    #[serde(rename = "max-luma")]
    pub max_luma: Option<f64>,
    #[serde(rename = "min-luma")]
    pub min_luma: Option<f64>,
    #[serde(rename = "sig-peak")]
    pub signal_peak: Option<f64>,
}

impl VideoParameters {
    /// IINA v1.5.0 VideoView.isHDRVideo accepts PQ/HLG only with BT.2020 or P3.
    /// This classifies the source; a matching display/output route is a separate fact.
    pub fn is_hdr(&self) -> bool {
        matches!(self.gamma.as_deref(), Some("pq" | "hlg"))
            && matches!(self.primaries.as_deref(), Some("bt.2020" | "display-p3"))
    }
}

pub fn time_label(seconds: Option<f64>) -> String {
    let Some(seconds) = seconds.filter(|s| s.is_finite() && *s >= 0.0) else {
        return "--:--".into();
    };
    let total = seconds.min(359_999_999.0) as u64;
    if total >= 3600 {
        format!("{}:{:02}:{:02}", total / 3600, total / 60 % 60, total % 60)
    } else {
        format!("{}:{:02}", total / 60, total % 60)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
// Root-level preferences grow additively between releases. Older binaries must
// ignore fields they do not understand so a settings save cannot prevent media
// playback after a user temporarily launches an earlier installed version.
#[serde(default)]
pub struct Preferences {
    pub leading_sidebar_width: u32,
    pub trailing_sidebar_width: u32,
    pub settings_at_leading: bool,
    pub playlist_at_leading: bool,
    pub edge_to_edge_video: bool,
    pub docked_controls: bool,
    pub unlock_window_aspect_ratio: bool,
    pub window_resize: window_resize::Settings,
    pub compact_ui: bool,
    pub osc_position: u8,
    pub control_bar: osc_preferences::Settings,
    pub always_float_on_top: bool,
    pub always_show_on_top_icon: bool,
    pub action_after_launch: ActionAfterLaunch,
    pub allow_multiple_windows: bool,
    pub group_simultaneous_opens_in_playlist: bool,
    pub allow_duplicate_players: bool,
    pub playlist_auto_add: bool,
    pub quit_when_no_opened_window: bool,
    pub keep_open_on_file_end: bool,
    pub pause_when_open: bool,
    pub play_when_entering_fullscreen: bool,
    pub pause_when_leaving_fullscreen: bool,
    pub pause_when_inactive: bool,
    pub pause_when_minimized: bool,
    pub pause_when_goes_to_sleep: bool,
    pub fullscreen_when_open: bool,
    pub resume_last_position: bool,
    pub auto_switch_to_music_mode: bool,
    pub disable_animations: bool,
    pub music_mode_show_playlist: bool,
    pub music_mode_show_album_art: bool,
    pub schema_version: u32,
    pub volume: f64,
    pub volume_boost: bool,
    pub advanced: advanced_preferences::Settings,
    pub audio: audio_preferences::Settings,
    pub hdr_enabled: bool,
    pub live_text_enabled: bool,
    pub hdr_tone_mapping: hdr_preferences::Settings,
    pub video_threads: u32,
    pub controls: control_preferences::Settings,
    pub key_bindings: key_bindings::Settings,
    pub liquid_glass: liquid_glass::SurfaceSettings,
    pub hardware_decoder: video_controls::HardwareDecoder,
    pub subtitle_ass_override: subtitle_controls::AssOverrideLevel,
    pub secondary_subtitle_ass_override: subtitle_controls::AssOverrideLevel,
    pub subtitle_position: subtitle_controls::PositionPreferences,
    pub subtitle_other: subtitle_controls::OtherPreferences,
    pub subtitle_auto_load: subtitle_controls::AutoLoadPreferences,
    pub online_subtitles: online_subtitles::Preferences,
    pub subtitle_font: String,
    pub subtitle_font_size: f64,
    pub subtitle_text_style: subtitle_controls::TextStylePreferences,
    // `None` means a settings file predates explicit appearance selection; in
    // that case keep honoring its saved `dark` bit instead of changing themes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_material: Option<ThemeMaterial>,
    pub dark: bool,
    pub compact: bool,
    pub enable_osd: bool,
    pub disable_osd_file_start: bool,
    pub disable_osd_pause_resume: bool,
    pub disable_osd_speed: bool,
    pub disable_osd_seek: bool,
    pub osd_timeout_secs: f64,
    pub osd_text_size: f64,
    pub display_time_and_battery_in_fullscreen: bool,
    pub show_chapter_pos: bool,
    pub picture_in_picture: pip_preferences::Settings,
    pub screenshot: screenshot::Settings,
    pub network: network_preferences::Connection,
    pub network_cache: network_preferences::Cache,
    pub network_online_videos: network_preferences::OnlineVideos,
    pub thumbnail_preview: thumbnail_preferences::Settings,
    pub recent_media: Vec<RecentMedia>,
}

/// Appearance intent. `System` follows the effective theme reported by the
/// native window system while keeping the player palette in sync with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMaterial {
    Dark,
    Light,
    System,
}

/// Startup action for launches without a media argument, matching IINA's
/// General > Behavior choice while keeping supplied media arguments primary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ActionAfterLaunch {
    #[default]
    WelcomeWindow,
    OpenPanel,
    None,
}

impl ActionAfterLaunch {
    pub const fn show_welcome(self, has_media_argument: bool) -> bool {
        !has_media_argument && matches!(self, Self::WelcomeWindow)
    }

    pub const fn open_file_panel(self, has_media_argument: bool) -> bool {
        !has_media_argument && matches!(self, Self::OpenPanel)
    }

    pub const fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::WelcomeWindow),
            1 => Some(Self::OpenPanel),
            2 => Some(Self::None),
            _ => None,
        }
    }

    pub const fn index(self) -> i32 {
        match self {
            Self::WelcomeWindow => 0,
            Self::OpenPanel => 1,
            Self::None => 2,
        }
    }
}

impl ThemeMaterial {
    pub const fn from_index(index: i32) -> Option<Self> {
        match index {
            0 => Some(Self::Dark),
            1 => Some(Self::Light),
            2 => Some(Self::System),
            _ => None,
        }
    }

    pub const fn index(self) -> i32 {
        match self {
            Self::Dark => 0,
            Self::Light => 1,
            Self::System => 2,
        }
    }

    pub const fn dark_for(self, system_is_dark: bool) -> bool {
        match self {
            Self::Dark => true,
            Self::Light => false,
            Self::System => system_is_dark,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecentMedia {
    pub path: String,
    pub title: String,
    pub position: f64,
}

impl Default for Preferences {
    fn default() -> Self {
        // IINA Preference.defaultPreference: softVolume=100, themeMaterial=dark.
        Self {
            leading_sidebar_width: 360,
            trailing_sidebar_width: 360,
            settings_at_leading: false,
            playlist_at_leading: true,
            edge_to_edge_video: true,
            docked_controls: false,
            unlock_window_aspect_ratio: false,
            window_resize: window_resize::Settings::default(),
            compact_ui: false,
            osc_position: 0,
            control_bar: osc_preferences::Settings::default(),
            always_float_on_top: false,
            always_show_on_top_icon: false,
            action_after_launch: ActionAfterLaunch::default(),
            allow_multiple_windows: true,
            group_simultaneous_opens_in_playlist: false,
            allow_duplicate_players: false,
            playlist_auto_add: true,
            quit_when_no_opened_window: false,
            keep_open_on_file_end: true,
            pause_when_open: false,
            play_when_entering_fullscreen: false,
            pause_when_leaving_fullscreen: false,
            pause_when_inactive: false,
            pause_when_minimized: false,
            pause_when_goes_to_sleep: true,
            fullscreen_when_open: false,
            resume_last_position: true,
            auto_switch_to_music_mode: true,
            disable_animations: false,
            music_mode_show_playlist: false,
            music_mode_show_album_art: true,
            schema_version: 1,
            volume: 100.0,
            volume_boost: false,
            advanced: advanced_preferences::Settings::default(),
            audio: audio_preferences::Settings::default(),
            hdr_enabled: true,
            live_text_enabled: false,
            hdr_tone_mapping: hdr_preferences::Settings::default(),
            video_threads: 0,
            controls: control_preferences::Settings::default(),
            key_bindings: key_bindings::Settings::default(),
            liquid_glass: liquid_glass::SurfaceSettings::default(),
            hardware_decoder: video_controls::HardwareDecoder::Auto,
            subtitle_ass_override: subtitle_controls::AssOverrideLevel::Scale,
            secondary_subtitle_ass_override: subtitle_controls::AssOverrideLevel::Scale,
            subtitle_position: subtitle_controls::PositionPreferences::default(),
            subtitle_other: subtitle_controls::OtherPreferences::default(),
            subtitle_auto_load: subtitle_controls::AutoLoadPreferences::default(),
            online_subtitles: online_subtitles::Preferences::default(),
            subtitle_font: subtitle_controls::DEFAULT_SUBTITLE_FONT.into(),
            subtitle_font_size: 55.0,
            subtitle_text_style: subtitle_controls::TextStylePreferences::default(),
            theme_material: Some(ThemeMaterial::Dark),
            dark: true,
            compact: false,
            enable_osd: true,
            disable_osd_file_start: false,
            disable_osd_pause_resume: false,
            disable_osd_speed: false,
            disable_osd_seek: false,
            osd_timeout_secs: 1.0,
            osd_text_size: 20.0,
            display_time_and_battery_in_fullscreen: false,
            show_chapter_pos: false,
            picture_in_picture: pip_preferences::Settings::default(),
            screenshot: screenshot::Settings::default(),
            network: network_preferences::Connection::default(),
            network_cache: network_preferences::Cache::default(),
            network_online_videos: network_preferences::OnlineVideos::default(),
            thumbnail_preview: thumbnail_preferences::Settings::default(),
            recent_media: Vec::new(),
        }
    }
}

impl Preferences {
    pub fn effective_theme_material(&self) -> ThemeMaterial {
        self.theme_material.unwrap_or(if self.dark {
            ThemeMaterial::Dark
        } else {
            ThemeMaterial::Light
        })
    }

    pub fn volume_limit(&self) -> f64 {
        if self.volume_boost { 200.0 } else { 100.0 }
    }

    pub fn remember_media(&mut self, media: RecentMedia) {
        // Keep one durable resume point per path and cap history like the compact
        // welcome list; paths are user data and are never canonicalized here.
        self.recent_media.retain(|item| item.path != media.path);
        self.recent_media.insert(0, media);
        self.recent_media.truncate(10);
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let mut prefs: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if prefs.schema_version != 1
            || !prefs.volume.is_finite()
            || !(0.0..=200.0).contains(&prefs.volume)
            || prefs.video_threads > i32::MAX as u32
            || !subtitle_controls::valid_preference_font_size(prefs.subtitle_font_size)
            || !subtitle_controls::valid_font_name(&prefs.subtitle_font)
        {
            return Err("Unsupported preference version or invalid volume".into());
        }
        prefs
            .subtitle_text_style
            .validate()
            .map_err(str::to_owned)?;
        prefs.subtitle_position.validate().map_err(str::to_owned)?;
        prefs.network_cache.validate().map_err(str::to_owned)?;
        prefs.thumbnail_preview.validate().map_err(str::to_owned)?;
        prefs.network.validate().map_err(str::to_owned)?;
        prefs
            .network_online_videos
            .validate()
            .map_err(str::to_owned)?;
        prefs.audio.validate().map_err(str::to_owned)?;
        prefs.hdr_tone_mapping.validate().map_err(str::to_owned)?;
        prefs.advanced.validate()?;
        prefs.controls.validate().map_err(str::to_owned)?;
        prefs.control_bar.validate().map_err(str::to_owned)?;
        prefs.key_bindings.validate().map_err(str::to_owned)?;
        prefs.subtitle_other.validate().map_err(str::to_owned)?;
        prefs.subtitle_auto_load.validate().map_err(str::to_owned)?;
        prefs.online_subtitles.validate().map_err(str::to_owned)?;
        // Old settings keep their default cap unless boosting was explicitly enabled.
        prefs.volume = prefs.volume.min(prefs.volume_limit());
        prefs.leading_sidebar_width = prefs.leading_sidebar_width.clamp(300, 500);
        prefs.trailing_sidebar_width = prefs.trailing_sidebar_width.clamp(300, 500);
        if prefs.osc_position > 2 {
            return Err("Unsupported on-screen controller position".into());
        }
        prefs.screenshot.validate()?;
        if prefs.recent_media.len() > 10
            || prefs.recent_media.iter().any(|item| {
                !valid_path(&item.path)
                    || item.title.contains('\0')
                    || !item.position.is_finite()
                    || item.position < 0.0
            })
        {
            return Err("Recent media history is invalid".into());
        }
        // Persisted values must be representable by the event-loop timer. Do not
        // silently turn an invalid duration into an immediate or endless OSD.
        let timeout = std::time::Duration::try_from_secs_f64(prefs.osd_timeout_secs)
            .map_err(|_| "Invalid on-screen display timeout")?;
        if timeout.as_millis() > u64::MAX as u128 {
            return Err("On-screen display timeout exceeds timer capacity".into());
        }
        if !prefs.osd_text_size.is_finite() || !(5.0..=500.0).contains(&prefs.osd_text_size) {
            return Err("On-screen display text size must be between 5 and 500 points".into());
        }
        Ok(prefs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_from_end_seeks_to_start_then_unpauses() {
        let ended = Playback {
            phase: Phase::Ended,
            ..Playback::default()
        };
        assert_eq!(
            Command::TogglePause.plan(&ended).unwrap().commands,
            [["seek", "0", "absolute+exact"], ["set", "pause", "no"],]
        );

        let playing = Playback {
            phase: Phase::Playing,
            ..Playback::default()
        };
        assert_eq!(
            Command::TogglePause.plan(&playing).unwrap().commands,
            [["cycle", "pause"]]
        );
    }

    #[test]
    fn set_paused_command_is_idempotent_and_replays_at_end() {
        let playing = Playback {
            phase: Phase::Playing,
            ..Playback::default()
        };
        assert_eq!(
            Command::SetPaused(true).plan(&playing).unwrap().commands,
            [["set", "pause", "yes"]]
        );
        assert_eq!(
            Command::SetPaused(false).plan(&playing).unwrap().commands,
            [["set", "pause", "no"]]
        );

        let ended = Playback {
            phase: Phase::Ended,
            ..Playback::default()
        };
        assert_eq!(
            Command::SetPaused(false).plan(&ended).unwrap().commands,
            [["seek", "0", "absolute+exact"], ["set", "pause", "no"]]
        );
    }

    #[test]
    fn live_text_frame_capture_requires_a_paused_source_and_packs_rgba_rows() {
        let paused = Playback {
            phase: Phase::Paused,
            ..Playback::default()
        };
        assert_eq!(
            Command::CaptureFrame.plan(&paused).unwrap().commands,
            [["screenshot-raw", "video", "rgba"]]
        );
        assert!(Command::CaptureFrame.plan(&Playback::default()).is_err());

        let frame = CapturedFrame::new(
            2,
            2,
            12,
            &[
                1, 2, 3, 4, 5, 6, 7, 8, 99, 99, 99, 99, 9, 10, 11, 12, 13, 14, 15, 16, 99, 99, 99,
                99,
            ],
        )
        .unwrap();
        assert_eq!(frame.width, 2);
        assert_eq!(frame.height, 2);
        assert_eq!(
            &*frame.rgba,
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
        );
        assert!(CapturedFrame::new(2, 2, 4, &[0; 16]).is_err());
        assert!(CapturedFrame::new(2, 2, 8, &[0; 15]).is_err());
    }

    #[test]
    fn unknown_root_preferences_do_not_block_launch_or_playback() {
        // A newer install may have saved additive settings before an older
        // executable is launched again; keep recognized values and defaults.
        let preferences = Preferences::parse(
            r#"{"volume":37,"unlock_window_aspect_ratio":false,"future_setting":{"mode":"auto"}}"#,
        )
        .unwrap();
        assert_eq!(preferences.volume, 37.0);
        assert!(!preferences.unlock_window_aspect_ratio);

        // Forward compatibility must not weaken validation of known controls.
        assert!(Preferences::parse(r#"{"volume":"loud","future_setting":true}"#).is_err());
    }

    #[test]
    fn tone_mapping_preferences_keep_reference_defaults_and_round_trip() {
        let legacy = Preferences::parse("{}").unwrap();
        assert_eq!(
            legacy.hdr_tone_mapping,
            hdr_preferences::Settings::default()
        );

        let selected = Preferences {
            hdr_tone_mapping: hdr_preferences::Settings {
                enabled: false,
                target_peak_override_enabled: true,
                target_peak_nits: 1200,
                algorithm: hdr_preferences::Algorithm::Bt2390,
                param_override_enabled: true,
                param: -0.25,
            },
            ..legacy
        };
        let encoded = serde_json::to_string(&selected).unwrap();
        assert_eq!(
            Preferences::parse(&encoded).unwrap().hdr_tone_mapping,
            selected.hdr_tone_mapping
        );
        assert!(
            Preferences::parse(r#"{"hdr_tone_mapping":{"enabled":true,"param":1e999}}"#).is_err()
        );
    }

    #[test]
    fn thumbnail_preferences_default_and_round_trip_with_root_settings() {
        let legacy = Preferences::parse("{}").unwrap();
        assert_eq!(
            legacy.thumbnail_preview,
            thumbnail_preferences::Settings::default()
        );
        let selected = Preferences {
            thumbnail_preview: thumbnail_preferences::Settings {
                enabled: false,
                maximum_cache_size_mb: 0,
                enable_for_remote_files: true,
                width: 320,
            },
            ..legacy
        };
        let encoded = serde_json::to_string(&selected).unwrap();
        assert_eq!(
            Preferences::parse(&encoded).unwrap().thumbnail_preview,
            selected.thumbnail_preview
        );
        assert!(Preferences::parse(
            r#"{"thumbnail_preview":{"enabled":true,"maximum_cache_size_mb":500,"enable_for_remote_files":false,"width":0}}"#
        )
        .is_err());
    }

    #[test]
    fn tone_mapping_command_plan_sets_the_live_renderer_options() {
        let selected = hdr_preferences::Settings {
            target_peak_override_enabled: true,
            target_peak_nits: 1000,
            algorithm: hdr_preferences::Algorithm::Bt2390,
            param_override_enabled: true,
            param: -0.25,
            ..Default::default()
        };
        assert_eq!(
            Command::SetToneMapping(selected)
                .plan(&Playback::default())
                .unwrap()
                .commands,
            [
                ["set", "target-peak", "1000"],
                ["set", "tone-mapping", "bt.2390"],
                ["set", "tone-mapping-param", "-0.25"],
            ]
        );
    }

    #[test]
    fn video_decoder_threads_default_to_auto_and_reach_mpv() {
        let defaults = Preferences::parse("{}").unwrap();
        assert_eq!(defaults.video_threads, 0);
        let configured = Preferences {
            video_threads: 8,
            ..defaults
        };
        assert_eq!(
            Preferences::parse(&serde_json::to_string(&configured).unwrap())
                .unwrap()
                .video_threads,
            8
        );
        assert!(Preferences::parse(r#"{"video_threads":2147483648}"#).is_err());
        assert_eq!(
            Command::VideoThreads(8)
                .plan(&Playback::default())
                .unwrap()
                .commands,
            [["set", "vd-lavc-threads", "8"]]
        );
        assert!(
            Command::VideoThreads(i32::MAX as u32 + 1)
                .plan(&Playback::default())
                .is_err()
        );
    }

    #[test]
    fn sidebar_widths_keep_legacy_defaults_and_bound_persisted_sizes() {
        let defaults = Preferences::parse("{}").unwrap();
        assert_eq!(
            (
                defaults.leading_sidebar_width,
                defaults.trailing_sidebar_width
            ),
            (360, 360)
        );
        let bounded =
            Preferences::parse(r#"{"leading_sidebar_width":1,"trailing_sidebar_width":900}"#)
                .unwrap();
        assert_eq!(
            (
                bounded.leading_sidebar_width,
                bounded.trailing_sidebar_width
            ),
            (300, 500)
        );
        let restored = Preferences::parse(&serde_json::to_string(&bounded).unwrap()).unwrap();
        assert_eq!(
            (
                restored.leading_sidebar_width,
                restored.trailing_sidebar_width
            ),
            (300, 500)
        );
        assert!(Preferences::parse(r#"{"leading_sidebar_width":-1}"#).is_err());
    }

    #[test]
    fn layout_preferences_keep_reference_defaults_and_reject_unknown_controller_positions() {
        let legacy = Preferences::parse("{}").unwrap();
        assert!(!legacy.compact_ui);
        assert_eq!(legacy.osc_position, 0);
        assert!(!legacy.settings_at_leading);
        assert!(legacy.playlist_at_leading);
        assert!(legacy.edge_to_edge_video);
        assert!(!legacy.docked_controls);
        assert!(!legacy.unlock_window_aspect_ratio);
        assert!(legacy.dark);
        assert!(!legacy.always_float_on_top);
        assert!(!legacy.always_show_on_top_icon);
        assert_eq!(legacy.window_resize, window_resize::Settings::default());
        assert_eq!(legacy.effective_theme_material(), ThemeMaterial::Dark);
        let selected = Preferences::parse(r#"{"compact_ui":true,"osc_position":2}"#).unwrap();
        let restored = Preferences::parse(&serde_json::to_string(&selected).unwrap()).unwrap();
        assert!(restored.compact_ui);
        let light = Preferences::parse(r#"{"dark":false}"#).unwrap();
        assert!(!light.dark);
        assert_eq!(light.effective_theme_material(), ThemeMaterial::Light);
        let system = Preferences::parse(r#"{"theme_material":"system","dark":false}"#).unwrap();
        assert_eq!(system.effective_theme_material(), ThemeMaterial::System);
        let restored_system = Preferences::parse(&serde_json::to_string(&system).unwrap()).unwrap();
        assert_eq!(
            restored_system.effective_theme_material(),
            ThemeMaterial::System
        );
        assert!(Preferences::parse(r#"{"theme_material":"automatic"}"#).is_err());
        let floating = Preferences::parse(r#"{"always_float_on_top":true}"#).unwrap();
        let restored_floating =
            Preferences::parse(&serde_json::to_string(&floating).unwrap()).unwrap();
        assert!(restored_floating.always_float_on_top);
        let with_pin_indicator = Preferences::parse(r#"{"always_show_on_top_icon":true}"#).unwrap();
        let restored_pin_indicator =
            Preferences::parse(&serde_json::to_string(&with_pin_indicator).unwrap()).unwrap();
        assert!(restored_pin_indicator.always_show_on_top_icon);
        assert_eq!(restored.osc_position, 2);
        assert!(Preferences::parse(r#"{"osc_position":3}"#).is_err());
    }

    #[test]
    fn sidebar_positions_round_trip_without_changing_existing_layout_defaults() {
        let selected =
            Preferences::parse(
                r#"{"settings_at_leading":true,"playlist_at_leading":false,"edge_to_edge_video":false,"docked_controls":true}"#,
            )
            .unwrap();
        let restored = Preferences::parse(&serde_json::to_string(&selected).unwrap()).unwrap();
        assert!(restored.settings_at_leading);
        assert!(!restored.playlist_at_leading);
        assert!(!restored.edge_to_edge_video);
        assert!(restored.docked_controls);
        assert!(!Preferences::parse("{}").unwrap().settings_at_leading);
        assert!(Preferences::parse("{}").unwrap().playlist_at_leading);
    }

    #[test]
    fn unlocked_window_aspect_persists_and_defaults_to_iina_locking() {
        let legacy = Preferences::parse("{}").unwrap();
        assert!(!legacy.unlock_window_aspect_ratio);
        let selected = Preferences::parse(r#"{"unlock_window_aspect_ratio":true}"#).unwrap();
        let restored = Preferences::parse(&serde_json::to_string(&selected).unwrap()).unwrap();
        assert!(restored.unlock_window_aspect_ratio);
    }

    #[test]
    fn window_resize_preferences_migrate_and_round_trip() {
        let legacy = Preferences::parse(r#"{"window_resize":{}}"#).unwrap();
        assert_eq!(legacy.window_resize, window_resize::Settings::default());

        let selected = Preferences::parse(
            r#"{"window_resize":{"option":"video_size15","timing":"always","use_physical_resolution":false}}"#,
        )
        .unwrap();
        let restored = Preferences::parse(&serde_json::to_string(&selected).unwrap()).unwrap();
        assert_eq!(restored.window_resize, selected.window_resize);
        assert_eq!(
            restored.window_resize.option,
            window_resize::ResizeOption::VideoSize15
        );
        assert_eq!(
            restored.window_resize.timing,
            window_resize::ResizeTiming::Always
        );
        assert!(!restored.window_resize.use_physical_resolution);
    }

    #[test]
    fn control_preferences_keep_iina_defaults_and_round_trip_validated_values() {
        let legacy = Preferences::parse("{}").unwrap();
        assert_eq!(legacy.controls, control_preferences::Settings::default());

        let selected = Preferences::parse(
            r#"{"controls":{"single_click_action":"pause","vertical_scroll_action":"playback_speed","seek_sensitivity":4}}"#,
        )
        .unwrap();
        let restored = Preferences::parse(&serde_json::to_string(&selected).unwrap()).unwrap();
        assert_eq!(restored.controls, selected.controls);

        assert!(Preferences::parse(r#"{"controls":{"seek_sensitivity":5}}"#).is_err());
    }

    #[test]
    fn key_binding_preferences_keep_reference_defaults_and_round_trip() {
        let defaults = Preferences::parse("{}").unwrap().key_bindings;
        assert_eq!(defaults, key_bindings::Settings::default());
        let selected = Preferences {
            key_bindings: key_bindings::Settings {
                current_profile: "Cinema".into(),
                use_system_media_controls: false,
                display_raw_values: true,
            },
            ..Preferences::default()
        };
        assert_eq!(
            Preferences::parse(&serde_json::to_string(&selected).unwrap())
                .unwrap()
                .key_bindings,
            selected.key_bindings
        );
        assert!(
            Preferences::parse(r#"{"key_bindings":{"current_profile":"../outside"}}"#).is_err()
        );
    }

    #[test]
    fn pause_when_open_keeps_legacy_defaults_and_round_trips_enabled_policy() {
        assert!(!Preferences::parse("{}").unwrap().pause_when_open);
        assert!(!Preferences::parse("{}").unwrap().fullscreen_when_open);
        assert!(Preferences::parse("{}").unwrap().resume_last_position);
        let enabled = Preferences::parse(
            r#"{"pause_when_open":true,"fullscreen_when_open":true,"resume_last_position":false}"#,
        )
        .unwrap();
        let saved = serde_json::to_string(&enabled).unwrap();
        assert!(Preferences::parse(&saved).unwrap().pause_when_open);
        assert!(Preferences::parse(&saved).unwrap().fullscreen_when_open);
        assert!(!Preferences::parse(&saved).unwrap().resume_last_position);
    }

    #[test]
    fn hdr_output_preference_defaults_on_and_round_trips() {
        assert!(Preferences::parse("{}").unwrap().hdr_enabled);
        let selected = Preferences {
            hdr_enabled: false,
            ..Preferences::default()
        };
        let saved = serde_json::to_string(&selected).unwrap();
        assert!(!Preferences::parse(&saved).unwrap().hdr_enabled);
    }

    #[test]
    fn resume_policy_updates_native_save_and_restore_together() {
        assert_eq!(
            Command::SetResumeLastPosition(false)
                .plan(&Playback::default())
                .unwrap()
                .commands,
            [
                vec!["set", "save-position-on-quit", "no"],
                vec!["set", "resume-playback", "no"],
            ]
        );
        assert_eq!(
            Command::SetResumeLastPosition(true)
                .plan(&Playback::default())
                .unwrap()
                .commands,
            [
                vec!["set", "save-position-on-quit", "yes"],
                vec!["set", "resume-playback", "yes"],
            ]
        );
    }

    #[test]
    fn ab_loop_points_preserve_zero_and_validate_the_whole_intent() {
        let state = Playback::default();
        assert_eq!(
            Command::SetAbLoop {
                a: Some(0.0),
                b: Some(2.0)
            }
            .plan(&state)
            .unwrap()
            .commands,
            vec![
                vec!["set", "ab-loop-a", "0.000001"],
                vec!["set", "ab-loop-b", "2"]
            ]
        );
        assert!(
            Command::SetAbLoop {
                a: Some(0.0),
                b: Some(f64::NAN)
            }
            .plan(&state)
            .is_err()
        );
        assert!(
            Command::SetAbLoop {
                a: Some(-1.0),
                b: None
            }
            .plan(&state)
            .is_err()
        );
        assert_eq!(
            Command::SetAbLoop { a: None, b: None }
                .plan(&state)
                .unwrap()
                .commands,
            vec![
                vec!["set", "ab-loop-a", "no"],
                vec!["set", "ab-loop-b", "no"]
            ]
        );
        let mut loaded = Playback {
            ab_loop_a: Some(1.0),
            ab_loop_b: Some(2.0),
            ..Default::default()
        };
        loaded.begin_load();
        assert!(loaded.ab_loop_a.is_none() && loaded.ab_loop_b.is_none());
    }

    #[test]
    fn dragging_an_ab_loop_point_updates_only_the_selected_endpoint() {
        let state = Playback {
            ab_loop_a: Some(1.0),
            ab_loop_b: Some(4.0),
            ..Default::default()
        };
        assert_eq!(
            Command::SetAbLoopPoint {
                point: AbLoopPoint::A,
                value: Some(2.5),
            }
            .plan(&state)
            .unwrap()
            .commands,
            [vec!["set", "ab-loop-a", "2.5"]]
        );
        assert_eq!(
            Command::SetAbLoopPoint {
                point: AbLoopPoint::B,
                value: None,
            }
            .plan(&state)
            .unwrap()
            .commands,
            [vec!["set", "ab-loop-b", "no"]]
        );
        assert!(
            Command::SetAbLoopPoint {
                point: AbLoopPoint::A,
                value: Some(f64::INFINITY),
            }
            .plan(&state)
            .is_err()
        );
    }

    #[test]
    fn legacy_preferences_keep_osd_defaults_and_customization_round_trips() {
        let old = Preferences::parse(
            r#"{"schema_version":1,"volume":37,"volume_boost":false,"dark":true,"compact":false}"#,
        )
        .unwrap();
        assert!(old.enable_osd);
        assert!(!old.disable_osd_file_start);
        assert!(!old.disable_osd_pause_resume);
        assert!(!old.disable_osd_speed);
        assert!(!old.disable_osd_seek);
        assert_eq!(old.osd_timeout_secs, 1.0);
        assert_eq!(old.osd_text_size, 20.0);
        assert!(old.control_bar.auto_hide_enabled);
        assert_eq!(old.control_bar.auto_hide_timeout_secs, 2.5);
        let partial_control_bar =
            Preferences::parse(r#"{"control_bar":{"auto_hide_timeout_secs":1.5}}"#).unwrap();
        assert!(partial_control_bar.control_bar.auto_hide_enabled);
        assert_eq!(partial_control_bar.control_bar.auto_hide_timeout_secs, 1.5);
        assert!(!old.display_time_and_battery_in_fullscreen);
        assert!(!old.live_text_enabled);
        let configured = Preferences {
            enable_osd: false,
            live_text_enabled: true,
            control_bar: osc_preferences::Settings {
                auto_hide_enabled: false,
                auto_hide_timeout_secs: 4.25,
            },
            disable_osd_speed: true,
            disable_osd_seek: true,
            osd_timeout_secs: 0.125,
            ..old
        };
        let saved = serde_json::to_string(&configured).unwrap();
        let restored = Preferences::parse(&saved).unwrap();
        assert!(!restored.enable_osd);
        assert!(restored.live_text_enabled);
        assert!(restored.disable_osd_speed);
        assert!(restored.disable_osd_seek);
        assert_eq!(restored.osd_timeout_secs, 0.125);
        assert!(!restored.control_bar.auto_hide_enabled);
        assert_eq!(restored.control_bar.auto_hide_timeout_secs, 4.25);
        assert_eq!(restored.volume, 37.0);
        for timeout in ["-1", "1e300"] {
            assert!(Preferences::parse(&format!("{{\"osd_timeout_secs\":{timeout}}}")).is_err());
        }
        for timeout in ["-0.5", "1e300"] {
            assert!(
                Preferences::parse(&format!(
                    "{{\"control_bar\":{{\"auto_hide_timeout_secs\":{timeout}}}}}"
                ))
                .is_err()
            );
        }
        assert_eq!(
            Preferences::parse(r#"{"osd_timeout_secs":0}"#)
                .unwrap()
                .osd_timeout_secs,
            0.0
        );
        for value in [4.999, 500.001, f64::NAN, f64::INFINITY] {
            assert!(Preferences::parse(&format!(r#"{{"osd_text_size":{value}}}"#)).is_err());
        }
    }

    #[test]
    fn chapter_position_markers_default_off_and_round_trip_when_enabled() {
        let legacy = Preferences::parse("{}").unwrap();
        assert!(!legacy.show_chapter_pos);

        let enabled = Preferences {
            show_chapter_pos: true,
            ..legacy
        };
        let saved = serde_json::to_string(&enabled).unwrap();
        assert!(Preferences::parse(&saved).unwrap().show_chapter_pos);
    }

    #[test]
    fn picture_in_picture_preferences_keep_iina_defaults_and_round_trip() {
        let legacy = Preferences::parse("{}").unwrap();
        assert_eq!(
            legacy.picture_in_picture.window_behavior,
            pip_preferences::WindowBehavior::Hide
        );
        assert!(!legacy.picture_in_picture.pause_when_entering);

        let configured = Preferences {
            picture_in_picture: pip_preferences::Settings {
                window_behavior: pip_preferences::WindowBehavior::Minimize,
                pause_when_entering: true,
                toggle_by_minimizing: true,
                minimize_toggle_video_only: true,
            },
            ..legacy
        };
        let saved = serde_json::to_string(&configured).unwrap();
        assert_eq!(
            Preferences::parse(&saved).unwrap().picture_in_picture,
            configured.picture_in_picture
        );
    }

    #[test]
    fn action_after_launch_matches_welcome_default_and_round_trips_all_choices() {
        assert_eq!(
            Preferences::parse("{} ").unwrap().action_after_launch,
            ActionAfterLaunch::WelcomeWindow
        );
        for action in [
            ActionAfterLaunch::WelcomeWindow,
            ActionAfterLaunch::OpenPanel,
            ActionAfterLaunch::None,
        ] {
            let preferences = Preferences {
                action_after_launch: action,
                ..Preferences::default()
            };
            let saved = serde_json::to_string(&preferences).unwrap();
            assert_eq!(
                Preferences::parse(&saved).unwrap().action_after_launch,
                action
            );
            assert_eq!(ActionAfterLaunch::from_index(action.index()), Some(action));
        }
        assert_eq!(ActionAfterLaunch::from_index(-1), None);
        assert_eq!(ActionAfterLaunch::from_index(3), None);
        assert!(ActionAfterLaunch::WelcomeWindow.show_welcome(false));
        assert!(!ActionAfterLaunch::OpenPanel.show_welcome(false));
        assert!(ActionAfterLaunch::OpenPanel.open_file_panel(false));
        assert!(!ActionAfterLaunch::OpenPanel.open_file_panel(true));
        assert!(!ActionAfterLaunch::None.show_welcome(false));
        assert!(!ActionAfterLaunch::None.open_file_panel(false));
    }

    #[test]
    fn network_cache_preferences_match_reference_defaults_and_reach_native_options() {
        use network_preferences::Cache;

        let defaults = Preferences::parse("{}").unwrap().network_cache;
        assert_eq!(defaults, Cache::default());
        assert_eq!(defaults.prefetch_secs, 36_000.0);
        assert_eq!(defaults.buffer_size_kib, 153_600.0);
        assert_eq!(
            Command::NetworkCache(defaults.clone())
                .plan(&Playback::default())
                .unwrap()
                .commands,
            vec![
                vec!["set", "cache", "yes"],
                vec!["set", "cache-pause-initial", "no"],
                vec!["set", "cache-pause-wait", "1"],
                vec!["set", "cache-secs", "36000"],
                vec!["set", "demuxer-max-bytes", "153600KiB"],
            ]
        );

        let configured = Cache {
            enabled: false,
            pause_initial: true,
            pause_wait_secs: 2.5,
            prefetch_secs: 12_000.0,
            buffer_size_kib: 512_000.0,
            show_buffering_throbber: false,
            show_seeking_throbber: false,
        };
        let persisted = Preferences {
            network_cache: configured.clone(),
            ..Preferences::default()
        };
        assert_eq!(
            Preferences::parse(&serde_json::to_string(&persisted).unwrap())
                .unwrap()
                .network_cache,
            configured
        );
        assert!(
            Cache {
                pause_wait_secs: f64::NAN,
                ..defaults
            }
            .validate()
            .is_err()
        );
        assert!(
            Cache {
                prefetch_secs: 2_000_000_000.0,
                ..Cache::default()
            }
            .validate()
            .is_ok()
        );
        assert!(
            Cache {
                buffer_size_kib: 2_000_000_001.0,
                ..Cache::default()
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn network_connection_preferences_default_and_round_trip() {
        use network_preferences::{Connection, RtspTransport};

        let defaults = Preferences::parse("{}").unwrap().network;
        assert_eq!(defaults, Connection::default());
        let configured = Connection {
            user_agent: "player agent".into(),
            http_proxy: "http://proxy.example:8080".into(),
            rtsp_transport: RtspTransport::Http,
        };
        let saved = Preferences {
            network: configured.clone(),
            ..Preferences::default()
        };
        assert_eq!(
            Preferences::parse(&serde_json::to_string(&saved).unwrap())
                .unwrap()
                .network,
            configured
        );
        assert!(Preferences::parse(r#"{"network":{"user_agent":"bad\u0000value"}}"#).is_err());
    }

    #[test]
    fn online_video_preferences_keep_reference_defaults_and_update_mpv() {
        use network_preferences::OnlineVideos;

        let defaults = Preferences::parse("{}").unwrap().network_online_videos;
        assert_eq!(defaults, OnlineVideos::default());
        assert!(defaults.enabled);
        assert_eq!(
            Command::NetworkOnlineVideos(defaults.clone())
                .plan(&Playback::default())
                .unwrap()
                .commands,
            vec![
                vec!["set", "ytdl", "yes"],
                vec!["set", "ytdl-raw-options", ""],
            ]
        );
        let configured = OnlineVideos {
            enabled: false,
            search_path: "C:\\apps\\yt-dlp".into(),
            raw_options: "proxy=http://proxy.example:8080".into(),
        };
        let preferences = Preferences {
            network_online_videos: configured.clone(),
            ..Preferences::default()
        };
        assert_eq!(
            Preferences::parse(&serde_json::to_string(&preferences).unwrap())
                .unwrap()
                .network_online_videos,
            configured
        );
        assert!(
            OnlineVideos {
                raw_options: "bad\0option".into(),
                ..OnlineVideos::default()
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn subtitle_ass_override_preferences_default_to_scale_and_round_trip_per_lane() {
        use subtitle_controls::AssOverrideLevel;

        let legacy = Preferences::parse("{}").unwrap();
        assert_eq!(legacy.subtitle_ass_override, AssOverrideLevel::Scale);
        assert_eq!(
            legacy.secondary_subtitle_ass_override,
            AssOverrideLevel::Scale
        );
        let configured = Preferences {
            subtitle_ass_override: AssOverrideLevel::Force,
            secondary_subtitle_ass_override: AssOverrideLevel::No,
            ..legacy
        };
        let restored = Preferences::parse(&serde_json::to_string(&configured).unwrap()).unwrap();
        assert_eq!(restored.subtitle_ass_override, AssOverrideLevel::Force);
        assert_eq!(
            restored.secondary_subtitle_ass_override,
            AssOverrideLevel::No
        );
    }

    #[test]
    fn subtitle_position_preferences_keep_source_defaults_and_round_trip() {
        use subtitle_controls::{HorizontalAlignment, PositionPreferences, VerticalAlignment};

        let legacy = Preferences::parse("{}").unwrap();
        assert_eq!(legacy.subtitle_position, PositionPreferences::default());

        let configured = Preferences {
            subtitle_position: PositionPreferences {
                horizontal: HorizontalAlignment::Right,
                vertical: VerticalAlignment::Top,
                position: 84.25,
                display_in_letterbox: false,
                scale_with_window: false,
                margin_x: 31.5,
                margin_y: 18.0,
            },
            ..legacy
        };
        let restored = Preferences::parse(&serde_json::to_string(&configured).unwrap()).unwrap();
        assert_eq!(restored.subtitle_position, configured.subtitle_position);

        for invalid in [
            r#"{"subtitle_position":{"position":100.1}}"#,
            r#"{"subtitle_position":{"margin_x":-0.1}}"#,
            r#"{"subtitle_position":{"margin_y":1e300}}"#,
            r#"{"subtitle_position":{"horizontal":"leading"}}"#,
        ] {
            assert!(Preferences::parse(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn subtitle_other_preferences_preserve_the_reference_encoding_menu() {
        use subtitle_controls::{OtherPreferences, SUBTITLE_ENCODING_CHOICES};

        assert_eq!(SUBTITLE_ENCODING_CHOICES.len(), 44);
        let legacy = Preferences::parse("{}").unwrap();
        assert_eq!(legacy.subtitle_other, OtherPreferences::default());
        assert_eq!(legacy.subtitle_other.encoding_index(), Some(0));

        let configured = Preferences {
            subtitle_other: OtherPreferences {
                preferred_languages: "eng,jpn,zh-Hant".into(),
                default_encoding: "SHIFT-JIS".into(),
            },
            ..legacy
        };
        let restored = Preferences::parse(&serde_json::to_string(&configured).unwrap()).unwrap();
        assert_eq!(restored.subtitle_other, configured.subtitle_other);
        assert!(
            restored
                .subtitle_other
                .commands()
                .contains(&Command::SubtitlePreferredLanguages(
                    "eng,jpn,zh-Hant".into()
                ))
        );
        assert!(
            restored
                .subtitle_other
                .commands()
                .contains(&Command::SubtitleEncoding("SHIFT-JIS".into()))
        );

        assert!(
            Preferences::parse(r#"{"subtitle_other":{"default_encoding":"invented-codepage"}}"#)
                .is_err()
        );
        assert!(
            Preferences::parse(r#"{"subtitle_other":{"preferred_languages":"eng\nfre"}}"#).is_err()
        );
    }

    #[test]
    fn subtitle_auto_load_preferences_keep_reference_choices_and_defaults() {
        use subtitle_controls::{AutoLoadMode, AutoLoadPreferences};

        let legacy = Preferences::parse("{}").unwrap();
        assert_eq!(legacy.subtitle_auto_load, AutoLoadPreferences::default());
        assert_eq!(legacy.subtitle_auto_load.mode, AutoLoadMode::Intelligent);
        assert_eq!(legacy.subtitle_auto_load.search_paths, "./*");
        assert_eq!(AutoLoadMode::Disabled.index(), 0);
        assert_eq!(AutoLoadMode::Filename.index(), 1);
        assert_eq!(AutoLoadMode::Intelligent.index(), 2);
        assert_eq!(AutoLoadMode::from_index(3), None);
        assert_eq!(
            AutoLoadPreferences {
                mode: AutoLoadMode::Disabled,
                ..AutoLoadPreferences::default()
            }
            .mpv_matching_mode(),
            "no"
        );
        assert_eq!(
            AutoLoadPreferences {
                mode: AutoLoadMode::Filename,
                ..AutoLoadPreferences::default()
            }
            .mpv_matching_mode(),
            "fuzzy"
        );
        assert_eq!(AutoLoadPreferences::default().mpv_matching_mode(), "exact");

        let configured = Preferences {
            subtitle_auto_load: AutoLoadPreferences {
                mode: AutoLoadMode::Filename,
                priority_keywords: "forced, SDH".into(),
                search_paths: std::env::join_paths(["./subs", "../captions"])
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            },
            ..legacy
        };
        let restored = Preferences::parse(&serde_json::to_string(&configured).unwrap()).unwrap();
        assert_eq!(restored.subtitle_auto_load, configured.subtitle_auto_load);
        assert_eq!(
            Command::SubtitleAutoLoad(configured.subtitle_auto_load.clone())
                .plan(&Playback::default())
                .unwrap()
                .commands,
            [
                ["set", "sub-auto", "fuzzy"].map(str::to_owned).to_vec(),
                [
                    "set",
                    "sub-file-paths",
                    std::env::join_paths(["./subs", "../captions"])
                        .unwrap()
                        .to_string_lossy()
                        .as_ref(),
                ]
                .map(str::to_owned)
                .to_vec(),
            ]
        );
        assert_eq!(
            AutoLoadPreferences::default().mpv_search_paths(),
            None,
            "IINA's default glob is not an mpv directory path"
        );
        for invalid in [
            r#"{"subtitle_auto_load":{"priority_keywords":"eng\nforced"}}"#,
            r#"{"subtitle_auto_load":{"search_paths":"subs\u0000"}}"#,
            r#"{"subtitle_auto_load":{"mode":"unknown"}}"#,
        ] {
            assert!(Preferences::parse(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn subtitle_encoding_change_updates_option_then_reloads_each_subtitle_track() {
        fn track(id: i64, kind: TrackKind) -> Track {
            Track {
                id,
                kind,
                title: None,
                lang: None,
                codec: None,
                selected: false,
                main_selection: None,
                external: false,
                albumart: false,
                external_filename: None,
                is_default: false,
                channels: None,
                sample_rate: None,
                width: None,
                height: None,
                frame_rate: None,
            }
        }
        let state = Playback {
            tracks: vec![
                Track {
                    external: true,
                    ..track(7, TrackKind::Sub)
                },
                track(9, TrackKind::Audio),
                track(11, TrackKind::Sub),
            ],
            ..Playback::default()
        };

        assert_eq!(
            Command::SubtitleEncoding("UTF-8".into())
                .plan(&state)
                .unwrap()
                .commands,
            vec![
                vec!["set", "sub-codepage", "UTF-8"],
                vec!["sub-reload", "7"],
                vec!["sub-reload", "11"],
            ]
        );
        assert_eq!(
            Command::SubtitleEncoding("auto".into())
                .plan(&Playback::default())
                .unwrap()
                .commands,
            [["set", "sub-codepage", "auto"]]
        );
        assert!(
            Command::SubtitleEncoding("invalid".into())
                .plan(&state)
                .is_err()
        );
    }

    #[test]
    fn subtitle_text_preferences_keep_legacy_defaults_and_fractional_sizes() {
        let legacy = Preferences::parse("{}").unwrap();
        assert_eq!(
            legacy.subtitle_font,
            subtitle_controls::DEFAULT_SUBTITLE_FONT
        );
        assert_eq!(legacy.subtitle_font_size, 55.0);

        let configured = Preferences {
            subtitle_font: "Noto Sans CJK JP".into(),
            subtitle_font_size: 42.375,
            ..legacy
        };
        let restored = Preferences::parse(&serde_json::to_string(&configured).unwrap()).unwrap();
        assert_eq!(restored.subtitle_font, "Noto Sans CJK JP");
        assert_eq!(restored.subtitle_font_size, 42.375);

        for invalid in ["-1", "1e300", "NaN"] {
            assert!(Preferences::parse(&format!(r#"{{"subtitle_font_size":{invalid}}}"#)).is_err());
        }
        assert!(Preferences::parse(r#"{"subtitle_font_size":0}"#).is_err());
        assert!(Preferences::parse(r#"{"subtitle_font":"bad\nfont"}"#).is_err());
    }

    #[test]
    fn subtitle_text_style_options_validate_and_round_trip() {
        use subtitle_controls::TextStylePreferences;

        let configured = Preferences {
            subtitle_text_style: TextStylePreferences {
                bold: true,
                italic: true,
                blur: 1.25,
                spacing: -0.5,
                outline_size: 4.5,
                shadow_offset: 2.25,
                text_color: subtitle_color::SubtitleColor::from_rgba([0.25, 0.5, 0.75, 1.0])
                    .unwrap(),
                ..TextStylePreferences::default()
            },
            ..Preferences::default()
        };
        let restored = Preferences::parse(&serde_json::to_string(&configured).unwrap()).unwrap();
        assert_eq!(restored.subtitle_text_style, configured.subtitle_text_style);

        for invalid in [
            r#"{"subtitle_text_style":{"blur":20.001}}"#,
            r#"{"subtitle_text_style":{"spacing":"wide"}}"#,
            r#"{"subtitle_text_style":{"outline_size":-0.1}}"#,
            r#"{"subtitle_text_style":{"shadow_offset":-1}}"#,
            r#"{"subtitle_text_style":{"text_color":[2,0,0,1]}}"#,
        ] {
            assert!(Preferences::parse(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn subtitle_text_style_commands_use_native_option_names() {
        let cases = [
            (Command::SubtitleBold(true), vec!["set", "sub-bold", "yes"]),
            (
                Command::SubtitleItalic(false),
                vec!["set", "sub-italic", "no"],
            ),
            (Command::SubtitleBlur(2.5), vec!["set", "sub-blur", "2.5"]),
            (
                Command::SubtitleSpacing(-0.25),
                vec!["set", "sub-spacing", "-0.25"],
            ),
            (
                Command::SubtitleShadowOffset(3.0),
                vec!["set", "sub-shadow-offset", "3"],
            ),
        ];
        for (command, expected) in cases {
            assert_eq!(command.mpv_args().unwrap(), expected);
        }
        for command in [
            Command::SubtitleBlur(20.1),
            Command::SubtitleBlur(f64::NAN),
            Command::SubtitleSpacing(f64::INFINITY),
            Command::SubtitleShadowOffset(-1.0),
        ] {
            assert!(command.mpv_args().is_err(), "{command:?}");
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn subtitle_reload_recognizes_mpv_normalized_windows_paths() {
        for (observed, requested) in [
            (r"C:\media\captions.srt", r"\\?\C:\media\captions.srt"),
            (
                r"\\server\share\captions.srt",
                r"\\?\UNC\server\share\captions.srt",
            ),
        ] {
            let track = serde_json::from_value(serde_json::json!({
                "id": 7, "type": "sub", "external": true,
                "external-filename": observed,
            }))
            .unwrap();
            let state = Playback {
                tracks: vec![track],
                ..Default::default()
            };
            assert_eq!(
                Command::AddSubtitle(requested.into())
                    .plan(&state)
                    .unwrap()
                    .commands,
                vec![vec!["sub-reload".to_owned(), "7".to_owned()]]
            );
            assert_eq!(
                Command::AddSubtitle(format!("{requested}.other"))
                    .plan(&state)
                    .unwrap()
                    .commands[0][0],
                "sub-add"
            );
        }
    }

    #[test]
    fn discovered_subtitles_use_auto_selection_and_skip_existing_files() {
        let path = "C:/media/subs/Movie.en.srt";
        let command = Command::AddSubtitlesWithoutSelection(vec![path.into()]);
        assert!(command.mpv_args().is_err());
        assert_eq!(
            command.plan(&Playback::default()).unwrap().commands,
            vec![vec!["sub-add", path, "auto"]]
        );

        let track: Track = serde_json::from_value(serde_json::json!({
            "id": 7, "type": "sub", "external": true,
            "external-filename": path,
        }))
        .unwrap();
        let state = Playback {
            tracks: vec![track],
            ..Playback::default()
        };
        assert!(command.plan(&state).unwrap().commands.is_empty());
    }

    #[test]
    fn hdr_classification_matches_pinned_source_without_inventing_metadata() {
        for (gamma, primaries, expected) in [
            (Some("pq"), Some("bt.2020"), true),
            (Some("hlg"), Some("display-p3"), true),
            (Some("pq"), Some("bt.709"), false),
            (Some("srgb"), Some("bt.2020"), false),
            (Some("pq"), None, false),
            (None, Some("bt.2020"), false),
        ] {
            let value = serde_json::json!({"w":192,"h":108,"gamma":gamma,"primaries":primaries});
            let parameters: VideoParameters = serde_json::from_value(value).unwrap();
            assert_eq!(parameters.is_hdr(), expected);
        }
    }

    #[test]
    fn hdr_output_intent_requires_recognized_hdr_metadata() {
        let source = VideoParameters {
            pixelformat: None,
            w: 1920,
            h: 1080,
            dw: None,
            dh: None,
            rotate: None,
            colormatrix: None,
            colorlevels: None,
            primaries: Some("bt.2020".into()),
            gamma: Some("hlg".into()),
            max_luma: None,
            min_luma: None,
            signal_peak: None,
        };
        let state = Playback {
            phase: Phase::Playing,
            video: Some(source),
            ..Playback::default()
        };
        let plan = Command::SetHdrEnabled(true).plan(&state).unwrap();
        assert_eq!(plan.commands[2], ["set", "target-prim", "bt.2020"]);
        assert_eq!(plan.commands[3], ["set", "target-trc", "pq"]);

        let sdr_state = Playback {
            video: Some(VideoParameters {
                gamma: Some("bt.1886".into()),
                ..state.video.clone().unwrap()
            }),
            ..Playback::default()
        };
        assert!(Command::SetHdrEnabled(true).plan(&sdr_state).is_err());
        assert!(Command::SetHdrEnabled(false).plan(&sdr_state).is_ok());
    }

    #[test]
    fn source_replacement_clears_current_hdr_target_before_loading() {
        let load = Command::Load("next.mkv".into())
            .plan(&Playback::default())
            .unwrap();
        assert_eq!(
            &load.commands[..4],
            [
                ["set", "icc-profile-auto", "yes"],
                ["set", "target-prim", "auto"],
                ["set", "target-trc", "auto"],
                ["set", "screenshot-tag-colorspace", "no"],
            ]
        );
        assert_eq!(
            load.commands.last().unwrap(),
            &["loadfile", "next.mkv", "replace"]
        );

        let batch = Command::OpenFiles(vec!["first.mkv".into(), "second.mkv".into()])
            .plan(&Playback::default())
            .unwrap();
        assert_eq!(&batch.commands[..4], &load.commands[..4]);
        assert_eq!(batch.commands[5], ["loadfile", "first.mkv", "replace"]);
        assert_eq!(batch.commands[6], ["loadfile", "second.mkv", "append"]);
    }

    #[test]
    fn unicode_media_stays_one_argument() {
        let path = r"C:\Фильмы\a b.mkv";
        assert_eq!(Command::Load(path.into()).mpv_args().unwrap()[1], path);
        assert!(Command::Load("a\0b".into()).mpv_args().is_err());
    }

    #[test]
    fn resume_load_uses_native_start_option_and_rejects_invalid_positions() {
        let command = Command::LoadAtPosition {
            path: r"C:\Фильмы\a b.mkv".into(),
            position: 42.5,
        };
        assert_eq!(
            command.mpv_args().unwrap(),
            [
                "loadfile",
                r"C:\Фильмы\a b.mkv",
                "replace",
                "-1",
                "start=42.5"
            ]
        );
        let plan = command.plan(&Playback::default()).unwrap();
        assert_eq!(plan.commands[4], ["set", "pause", "no"]);
        assert_eq!(plan.commands[5][4], "start=42.5");
        for position in [-1.0, f64::NAN, f64::INFINITY] {
            assert!(
                Command::LoadAtPosition {
                    path: "movie.mkv".into(),
                    position
                }
                .mpv_args()
                .is_err()
            );
        }
        assert!(
            Command::LoadAtPosition {
                path: "bad\0path".into(),
                position: 2.0
            }
            .plan(&Playback::default())
            .is_err()
        );
    }

    #[test]
    fn reject_non_finite_commands() {
        assert!(
            Command::Seek {
                seconds: f64::NAN,
                absolute: true
            }
            .mpv_args()
            .is_err()
        );
        assert!(Command::Volume(201.0).mpv_args().is_err());
        assert!(Command::SubtitleFontSize(0.0).mpv_args().is_err());
        assert!(Command::SubtitleFontSize(-1.0).mpv_args().is_err());
        for factor in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(Command::MultiplySpeed(factor).mpv_args().is_err());
        }
        assert!(
            Command::AdjustVolume {
                delta: f64::NAN,
                boost: false
            }
            .plan(&Playback::default())
            .is_err()
        );
        assert!(Command::AdjustAudioDelay(f64::NAN).mpv_args().is_err());
        assert!(
            Command::AdjustAudioDelay(f64::NEG_INFINITY)
                .mpv_args()
                .is_err()
        );
        assert!(Command::AdjustSubtitleDelay(f64::NAN).mpv_args().is_err());
        assert!(Command::AdjustSubtitleScale(f64::NAN).mpv_args().is_err());
        assert!(
            Command::AdjustSubtitlePosition(f64::INFINITY)
                .mpv_args()
                .is_err()
        );
        assert!(
            Command::AdjustSubtitleDelay(f64::INFINITY)
                .mpv_args()
                .is_err()
        );
    }

    #[test]
    fn scroll_seek_uses_the_selected_keyframe_or_exact_mode() {
        for (seek_type, mode) in [
            (control_preferences::SeekType::Keyframe, "relative"),
            (control_preferences::SeekType::Exact, "relative+exact"),
            (control_preferences::SeekType::Auto, "relative"),
        ] {
            let plan = Command::SeekWithType {
                seconds: 2.5,
                seek_type,
            }
            .plan(&Playback::default())
            .unwrap();
            assert_eq!(plan.commands, [["seek", "2.5", mode]]);
        }
    }

    #[test]
    fn unknown_time_is_not_zero() {
        assert_eq!(time_label(None), "--:--");
        assert_eq!(time_label(Some(3661.9)), "1:01:01");
    }

    #[test]
    fn subtitle_style_eligibility_combines_both_selected_lanes() {
        for (tracks, text_controls, ass_warning) in [
            ("[]", true, false),
            (
                r#"[{"id":1,"type":"sub","codec":"hdmv_pgs_subtitle","selected":true,"main-selection":0}]"#,
                false,
                false,
            ),
            (
                r#"[{"id":1,"type":"sub","codec":"dvb_subtitle","selected":true,"main-selection":1}]"#,
                false,
                false,
            ),
            (
                r#"[{"id":1,"type":"sub","codec":"hdmv_pgs_subtitle","selected":true,"main-selection":0},{"id":2,"type":"sub","codec":"subrip","selected":true,"main-selection":1}]"#,
                true,
                false,
            ),
            (
                r#"[{"id":1,"type":"sub","codec":"ass","selected":true,"main-selection":1}]"#,
                true,
                true,
            ),
            (
                r#"[{"id":1,"type":"sub","codec":"ass","selected":false},{"id":2,"type":"audio","codec":"ass","selected":true,"main-selection":0}]"#,
                true,
                false,
            ),
            (
                r#"[{"id":1,"type":"sub","selected":true,"main-selection":0}]"#,
                true,
                false,
            ),
        ] {
            let state = Playback {
                tracks: serde_json::from_str(tracks).unwrap(),
                ..Playback::default()
            };
            assert_eq!(
                state.subtitle_style_availability(),
                SubtitleStyleAvailability {
                    text_controls,
                    ass_warning
                },
                "{tracks}"
            );
        }
    }

    #[test]
    fn absolute_subtitle_position_preserves_lane_and_validates_engine_range() {
        for value in [0.0, 100.0, 150.0] {
            for (command, property) in [
                (Command::SubtitlePosition(value), "sub-pos"),
                (
                    Command::SecondarySubtitlePosition(value),
                    "secondary-sub-pos",
                ),
            ] {
                assert_eq!(
                    command.mpv_args().unwrap(),
                    ["set", property, &value.to_string()]
                );
            }
        }
        for value in [-1.0, 151.0, f64::NAN, f64::INFINITY] {
            assert!(Command::SubtitlePosition(value).mpv_args().is_err());
            assert!(
                Command::SecondarySubtitlePosition(value)
                    .mpv_args()
                    .is_err()
            );
        }
    }

    #[test]
    fn loading_clears_previous_media_clock() {
        let mut state = Playback {
            position: Some(99.0),
            duration: Some(100.0),
            ..Default::default()
        };
        state.begin_load();
        assert_eq!(state.generation, 1);
        assert_eq!(state.position, None);
        assert_eq!(state.phase, Phase::Loading);
    }

    #[test]
    fn buffering_pause_and_seek_have_distinct_states() {
        let mut state = Playback {
            phase: Phase::Playing,
            paused: true,
            buffering: true,
            ..Default::default()
        };
        state.reconcile();
        assert_eq!(state.phase, Phase::Buffering);
        state.buffering = false;
        state.reconcile();
        assert_eq!(state.phase, Phase::Paused);
        state.seeking = true;
        state.reconcile();
        assert_eq!(state.phase, Phase::Seeking);
    }

    #[test]
    fn preferences_reject_future_schema() {
        assert!(Preferences::parse(r#"{"schema_version":2}"#).is_err());
        assert_eq!(Preferences::parse("{}").unwrap().volume, 100.0);
        assert!(Preferences::parse("{}").unwrap().recent_media.is_empty());
    }

    #[test]
    fn accessibility_motion_preference_defaults_off_and_round_trips() {
        assert!(!Preferences::parse("{}").unwrap().disable_animations);

        let configured = Preferences {
            disable_animations: true,
            ..Preferences::default()
        };
        let encoded = serde_json::to_string(&configured).unwrap();
        assert!(Preferences::parse(&encoded).unwrap().disable_animations);
    }

    #[test]
    fn recent_media_round_trips_and_rejects_invalid_history() {
        let item = RecentMedia {
            path: r"C:\Фильмы\a.mkv".into(),
            title: "A film".into(),
            position: 87.25,
        };
        let prefs = Preferences {
            recent_media: vec![item.clone()],
            ..Preferences::default()
        };
        let encoded = serde_json::to_string(&prefs).unwrap();
        assert_eq!(Preferences::parse(&encoded).unwrap().recent_media, [item]);
        assert!(
            Preferences::parse(r#"{"recent_media":[{"path":"x","title":"x","position":-1.0}]}"#)
                .is_err()
        );
        assert!(
            Preferences::parse(
                r#"{"recent_media":[{"path":"x","title":"x","position":1.0,"secret":true}]}"#
            )
            .is_err()
        );
    }

    #[test]
    fn remembered_media_moves_to_front_and_bounds_history() {
        let mut prefs = Preferences::default();
        for index in 0..12 {
            prefs.remember_media(RecentMedia {
                path: format!("film-{index}.mkv"),
                title: format!("Film {index}"),
                position: index as f64,
            });
        }
        prefs.remember_media(RecentMedia {
            path: "film-4.mkv".into(),
            title: "Film Four".into(),
            position: 44.0,
        });
        assert_eq!(prefs.recent_media.len(), 10);
        assert_eq!(prefs.recent_media[0].path, "film-4.mkv");
        assert_eq!(prefs.recent_media[0].position, 44.0);
        assert_eq!(
            prefs
                .recent_media
                .iter()
                .filter(|item| item.path == "film-4.mkv")
                .count(),
            1
        );
        assert!(
            !prefs
                .recent_media
                .iter()
                .any(|item| item.path == "film-0.mkv")
        );
    }

    #[test]
    fn amplified_volume_round_trips_and_rejects_invalid_values() {
        for volume in [0.0, 100.0, 150.0, 200.0] {
            assert_eq!(
                Command::Volume(volume).mpv_args().unwrap()[2],
                volume.to_string()
            );
            let json = format!(r#"{{"volume":{volume},"volume_boost":true}}"#);
            assert_eq!(Preferences::parse(&json).unwrap().volume, volume);
        }
        for volume in [-1.0, 200.1, f64::NAN, f64::INFINITY] {
            assert!(Command::Volume(volume).mpv_args().is_err());
        }
        assert!(Preferences::parse(r#"{"volume":201}"#).is_err());
        assert!(!Preferences::default().volume_boost);
        assert_eq!(
            Preferences::parse(r#"{"volume":150}"#).unwrap().volume,
            100.0
        );
    }

    #[test]
    fn invalid_batch_is_rejected_before_planning_any_append() {
        assert!(
            Command::AppendFiles(vec!["first.mkv".into(), "bad\0name".into()])
                .plan(&Playback::default())
                .is_err()
        );
        assert!(
            Command::OpenFiles(vec!["first.mkv".into(), "bad\0name".into()])
                .plan(&Playback::default())
                .is_err()
        );
        assert!(
            Command::OpenFiles(Vec::new())
                .plan(&Playback::default())
                .is_err()
        );
    }

    #[test]
    fn chapter_selection_resumes_and_playlist_ids_resolve_after_reorder() {
        let state = Playback {
            chapters: vec![Chapter {
                time: 2.0,
                title: None,
            }],
            playlist: vec![
                PlaylistEntry {
                    id: 41,
                    filename: "b".into(),
                    title: None,
                    current: true,
                    playing: true,
                },
                PlaylistEntry {
                    id: 40,
                    filename: "a".into(),
                    title: None,
                    current: false,
                    playing: false,
                },
            ],
            ..Default::default()
        };
        let chapter = Command::SelectChapter(0).plan(&state).unwrap();
        assert_eq!(
            chapter.commands,
            vec![vec!["seek", "2", "absolute"], vec!["set", "pause", "no"]]
        );
        let selected = Command::SelectPlaylistId(40).plan(&state).unwrap();
        assert_eq!(selected.commands[1], vec!["playlist-play-index", "1"]);
        assert!(selected.resume_on_load);
        assert!(Command::SelectPlaylistId(99).plan(&state).is_err());
        assert_eq!(
            Command::MovePlaylistId {
                id: 41,
                direction: 1
            }
            .plan(&state)
            .unwrap()
            .commands[0],
            vec!["playlist-move", "0", "2"]
        );
    }

    #[test]
    fn removing_selected_playlist_ids_uses_original_order_and_rejects_stale_or_duplicate_ids() {
        let state = Playback {
            playlist: [10, 20, 30, 40, 50]
                .into_iter()
                .map(|id| PlaylistEntry {
                    id,
                    filename: format!("{id}.mkv"),
                    title: None,
                    current: id == 30,
                    playing: id == 30,
                })
                .collect(),
            ..Default::default()
        };

        assert_eq!(
            Command::RemovePlaylistIds(vec![50, 20])
                .plan(&state)
                .unwrap()
                .commands,
            vec![vec!["playlist-remove", "1"], vec!["playlist-remove", "3"]]
        );
        assert!(
            Command::RemovePlaylistIds(vec![20, 20])
                .plan(&state)
                .is_err()
        );
        assert!(Command::RemovePlaylistIds(vec![99]).plan(&state).is_err());
        assert!(
            Command::RemovePlaylistIds(Vec::new())
                .plan(&state)
                .unwrap()
                .commands
                .is_empty()
        );
    }

    #[test]
    fn playlist_sort_plans_keep_native_entry_identity_and_current_item() {
        let state = Playback {
            playlist: vec![
                PlaylistEntry {
                    id: 41,
                    filename: "D:/media/zeta.mkv".into(),
                    title: None,
                    current: true,
                    playing: true,
                },
                PlaylistEntry {
                    id: 40,
                    filename: "C:/media/alpha.mkv".into(),
                    title: None,
                    current: false,
                    playing: false,
                },
            ],
            ..Default::default()
        };
        assert_eq!(
            Command::SortPlaylist(playlist::PlaylistSort::FilenameAscending)
                .plan(&state)
                .unwrap()
                .commands,
            vec![vec!["playlist-move", "1", "0"]]
        );
        assert!(
            Command::SortPlaylist(playlist::PlaylistSort::PathAscending)
                .plan(&Playback::default())
                .unwrap()
                .commands
                .is_empty()
        );
        assert_eq!(
            Command::TogglePlaylistShuffle
                .plan(&Playback::default())
                .unwrap()
                .commands,
            vec![vec!["playlist-shuffle"]]
        );
        assert_eq!(
            Command::TogglePlaylistShuffle
                .plan(&Playback {
                    playlist_shuffled: true,
                    ..Default::default()
                })
                .unwrap()
                .commands,
            vec![vec!["playlist-unshuffle"]]
        );
    }
}
