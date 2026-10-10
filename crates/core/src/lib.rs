//! Player intent and observed state. The backend is the only playback clock.

pub mod crop;
pub mod crop_drag;
pub mod crop_geometry;
pub mod crop_session;
pub mod file_selection;
pub mod screenshot;
pub mod speed_controls;
pub mod subtitle_color;
pub mod subtitle_controls;
mod track_labels;
pub mod video_controls;
pub mod video_reflection;
pub mod video_viewport;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Load(String),
    OpenFiles(Vec<String>),
    TogglePause,
    Stop,
    Seek {
        seconds: f64,
        absolute: bool,
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
    VideoEqualizer {
        option: video_controls::Equalizer,
        value: i64,
    },
    PlaylistNext,
    PlaylistPrevious,
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
    MovePlaylistId {
        id: i64,
        direction: i32,
    },
    AppendFiles(Vec<String>),
    RemovePlaylistEntry(usize),
    MovePlaylistEntry {
        from: usize,
        to: usize,
    },
    ClearPlaylist,
    AddSubtitle(String),
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
    Shutdown,
}

impl Command {
    pub fn mpv_args(&self) -> Result<Vec<String>, String> {
        let args: Vec<String> = match self {
            Self::Load(path) if !path.is_empty() && !path.contains('\0') => {
                vec!["loadfile".into(), path.clone(), "replace".into()]
            }
            Self::Load(_) => return Err("The media path is empty or contains a NUL byte".into()),
            Self::TogglePause => vec!["cycle".into(), "pause".into()],
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
            Self::AddSubtitle(path) | Self::AddAudio(path) if valid_path(path) => vec![
                if matches!(self, Self::AddSubtitle(_)) {
                    "sub-add"
                } else {
                    "audio-add"
                }
                .into(),
                path.clone(),
            ],
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
            | Self::CycleLoop
            | Self::SetAbLoop { .. }
            | Self::SelectPlaylistId(_)
            | Self::RemovePlaylistId(_)
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
        let commands = match self {
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
                    commands.insert(0, vec!["set".into(), "pause".into(), "no".into()]);
                }
                commands
            }
            Self::Load(_) => vec![
                vec!["set".into(), "pause".into(), "no".into()],
                self.mpv_args()?,
            ],
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

#[derive(Debug, Clone, Default)]
pub struct Playback {
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
    pub video_equalizer: [Option<i64>; 5],
    pub title: String,
    pub error: Option<String>,
    pub tracks: Vec<Track>,
    pub chapters: Vec<Chapter>,
    pub playlist: Vec<PlaylistEntry>,
    pub audio_delay: f64,
    pub subtitle_delay: f64,
    pub subtitle_scale: Option<f64>,
    pub subtitle_font_size: Option<f64>,
    pub subtitle_font: Option<String>,
    pub subtitle_outline_size: Option<f64>,
    pub subtitle_text_color: Option<subtitle_color::SubtitleColor>,
    pub subtitle_border_color: Option<subtitle_color::SubtitleColor>,
    pub subtitle_shadow_color: Option<subtitle_color::SubtitleColor>,
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
        self.video_target = None;
        self.video_output = None;
        self.deinterlace_active = None;
        self.video_frame_info = None;
        self.screenshot_file = None;
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
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    pub schema_version: u32,
    pub volume: f64,
    pub volume_boost: bool,
    pub hardware_decoder: video_controls::HardwareDecoder,
    pub dark: bool,
    pub compact: bool,
    pub enable_osd: bool,
    pub disable_osd_speed: bool,
    pub disable_osd_seek: bool,
    pub osd_timeout_secs: f64,
    pub screenshot: screenshot::Settings,
}

impl Default for Preferences {
    fn default() -> Self {
        // IINA Preference.defaultPreference: softVolume=100, themeMaterial=dark.
        Self {
            schema_version: 1,
            volume: 100.0,
            volume_boost: false,
            hardware_decoder: video_controls::HardwareDecoder::Auto,
            dark: true,
            compact: false,
            enable_osd: true,
            disable_osd_speed: false,
            disable_osd_seek: false,
            osd_timeout_secs: 1.0,
            screenshot: screenshot::Settings::default(),
        }
    }
}

impl Preferences {
    pub fn volume_limit(&self) -> f64 {
        if self.volume_boost { 200.0 } else { 100.0 }
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let mut prefs: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if prefs.schema_version != 1
            || !prefs.volume.is_finite()
            || !(0.0..=200.0).contains(&prefs.volume)
        {
            return Err("Unsupported preference version or invalid volume".into());
        }
        // Old settings keep their default cap unless boosting was explicitly enabled.
        prefs.volume = prefs.volume.min(prefs.volume_limit());
        prefs.screenshot.validate()?;
        // Persisted values must be representable by the event-loop timer. Do not
        // silently turn an invalid duration into an immediate or endless OSD.
        let timeout = std::time::Duration::try_from_secs_f64(prefs.osd_timeout_secs)
            .map_err(|_| "Invalid on-screen display timeout")?;
        if timeout.as_millis() > u64::MAX as u128 {
            return Err("On-screen display timeout exceeds timer capacity".into());
        }
        Ok(prefs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn legacy_preferences_keep_osd_defaults_and_customization_round_trips() {
        let old = Preferences::parse(
            r#"{"schema_version":1,"volume":37,"volume_boost":false,"dark":true,"compact":false}"#,
        )
        .unwrap();
        assert!(old.enable_osd);
        assert!(!old.disable_osd_speed);
        assert!(!old.disable_osd_seek);
        assert_eq!(old.osd_timeout_secs, 1.0);
        let configured = Preferences {
            enable_osd: false,
            disable_osd_speed: true,
            disable_osd_seek: true,
            osd_timeout_secs: 0.125,
            ..old
        };
        let saved = serde_json::to_string(&configured).unwrap();
        let restored = Preferences::parse(&saved).unwrap();
        assert!(!restored.enable_osd);
        assert!(restored.disable_osd_speed);
        assert!(restored.disable_osd_seek);
        assert_eq!(restored.osd_timeout_secs, 0.125);
        assert_eq!(restored.volume, 37.0);
        for timeout in ["-1", "1e300"] {
            assert!(Preferences::parse(&format!("{{\"osd_timeout_secs\":{timeout}}}")).is_err());
        }
        assert_eq!(
            Preferences::parse(r#"{"osd_timeout_secs":0}"#)
                .unwrap()
                .osd_timeout_secs,
            0.0
        );
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
    fn unicode_media_stays_one_argument() {
        let path = r"C:\Фильмы\a b.mkv";
        assert_eq!(Command::Load(path.into()).mpv_args().unwrap()[1], path);
        assert!(Command::Load("a\0b".into()).mpv_args().is_err());
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
}
