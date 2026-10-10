//! Player intent and observed state. The backend is the only playback clock.

pub mod file_selection;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Load(String),
    OpenFiles(Vec<String>),
    TogglePause,
    Stop,
    Seek { seconds: f64, absolute: bool },
    Volume(f64),
    ToggleMute,
    Speed(f64),
    FrameStep,
    PlaylistNext,
    PlaylistPrevious,
    SelectTrack { kind: TrackKind, id: Option<i64> },
    SelectChapter(usize),
    SelectPlaylistEntry(usize),
    SelectPlaylistId(i64),
    RemovePlaylistId(i64),
    MovePlaylistId { id: i64, direction: i32 },
    AppendFiles(Vec<String>),
    RemovePlaylistEntry(usize),
    MovePlaylistEntry { from: usize, to: usize },
    ClearPlaylist,
    AddSubtitle(String),
    AddAudio(String),
    AudioDelay(f64),
    SubtitleDelay(f64),
    ToggleSubtitleVisibility,
    CycleLoop,
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
            Self::Speed(value) if value.is_finite() && (0.01..=100.0).contains(value) => {
                vec!["set".into(), "speed".into(), value.to_string()]
            }
            Self::ToggleMute => vec!["cycle".into(), "mute".into()],
            Self::FrameStep => vec!["frame-step".into()],
            Self::PlaylistNext => vec!["playlist-next".into(), "weak".into()],
            Self::PlaylistPrevious => vec!["playlist-prev".into(), "weak".into()],
            Self::SelectTrack { kind, id } if id.is_none_or(|id| id > 0) => vec![
                "set".into(),
                kind.property().into(),
                id.map_or_else(|| "no".into(), |id| id.to_string()),
            ],
            Self::SelectChapter(index) => vec!["set".into(), "chapter".into(), index.to_string()],
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
            Self::OpenFiles(_)
            | Self::AppendFiles(_)
            | Self::CycleLoop
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
}

impl Track {
    pub fn display_name(&self) -> String {
        // MPVTrack.readableString excludes the language in the sidebar; the UI
        // displays it separately. Preserve title and codec/channel/rate metadata.
        let mut info = self.codec.iter().cloned().collect::<Vec<_>>();
        if self.kind == TrackKind::Audio {
            if let Some(channels) = self.channels {
                info.push(format!("{channels}ch"));
            }
            if let Some(rate) = self.sample_rate {
                info.push(format!("{}kHz", rate as f64 / 1000.0));
            }
        }
        if self.is_default {
            info.push("Default".into());
        }
        let title = self.title.as_deref().unwrap_or("No Title");
        if info.is_empty() {
            title.into()
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
    pub seekable: bool,
    pub volume: f64,
    pub muted: bool,
    pub speed: f64,
    pub audio_output: Option<String>,
    pub hardware_decoder: Option<String>,
    pub video: Option<VideoParameters>,
    pub video_target: Option<VideoParameters>,
    pub title: String,
    pub error: Option<String>,
    pub tracks: Vec<Track>,
    pub chapters: Vec<Chapter>,
    pub playlist: Vec<PlaylistEntry>,
    pub audio_delay: f64,
    pub subtitle_delay: f64,
    pub subtitle_visible: bool,
    pub subtitle_text: Option<String>,
    pub loop_file: Option<String>,
    pub loop_playlist: Option<String>,
}

impl Playback {
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
        self.error = None;
        // Track IDs belong to one file; never offer a stale selector during replacement.
        self.tracks.clear();
        self.chapters.clear();
        self.subtitle_text = None;
        self.video = None;
        self.video_target = None;
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
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct VideoParameters {
    pub pixelformat: Option<String>,
    pub w: u32,
    pub h: u32,
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
    pub dark: bool,
    pub compact: bool,
    pub enable_osd: bool,
    pub osd_timeout_secs: f64,
}

impl Default for Preferences {
    fn default() -> Self {
        // IINA Preference.defaultPreference: softVolume=100, themeMaterial=dark.
        Self {
            schema_version: 1,
            volume: 100.0,
            volume_boost: false,
            dark: true,
            compact: false,
            enable_osd: true,
            osd_timeout_secs: 1.0,
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
    fn legacy_preferences_keep_osd_defaults_and_customization_round_trips() {
        let old = Preferences::parse(
            r#"{"schema_version":1,"volume":37,"volume_boost":false,"dark":true,"compact":false}"#,
        )
        .unwrap();
        assert!(old.enable_osd);
        assert_eq!(old.osd_timeout_secs, 1.0);
        let configured = Preferences {
            enable_osd: false,
            osd_timeout_secs: 0.125,
            ..old
        };
        let saved = serde_json::to_string(&configured).unwrap();
        let restored = Preferences::parse(&saved).unwrap();
        assert!(!restored.enable_osd);
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
    }

    #[test]
    fn unknown_time_is_not_zero() {
        assert_eq!(time_label(None), "--:--");
        assert_eq!(time_label(Some(3661.9)), "1:01:01");
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
