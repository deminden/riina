//! Validated playback intent and its native command plan.
mod arguments;
mod media_link;
pub use media_link::MediaLink;
mod audio;
mod capture;
mod error;
pub use error::CommandError;
mod filters;
mod network;
mod plan;
mod plugin_mpv;
pub use plugin_mpv::{
    MAX_PLUGIN_MPV_ARGUMENT_BYTES, MAX_PLUGIN_MPV_ARGUMENTS, PluginMpvCommand,
    validate_plugin_mpv_arguments,
};
mod playback;
mod playlist;
mod screenshot_plan;
mod seek;
mod subtitles;
mod tracks;
mod video;

use crate::{
    LoopMode, TrackKind, audio_preferences::ReplayGainOptions, control_preferences, crop,
    crop_session, hdr_preferences, network_preferences, screenshot, subtitle_color,
    subtitle_controls, video_controls, video_reflection, video_viewport,
};

fn cmd<const N: usize>(parts: [&str; N]) -> Vec<String> {
    parts.into_iter().map(str::to_owned).collect()
}

fn set(name: &str, value: impl Into<String>) -> Vec<String> {
    vec!["set".to_owned(), name.to_owned(), value.into()]
}

fn flag(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Load(String),
    LoadAtPosition {
        path: String,
        position: f64,
    },
    OpenFiles(Vec<String>),
    /// Open an explicit playlist without discovering siblings of its entries.
    OpenPlaylist(Vec<String>),
    OpenShuffledPlaylist(Vec<String>),
    OpenMediaLink(MediaLink),
    OpenFilesWithSubtitles {
        paths: Vec<String>,
        subtitles: Vec<String>,
    },
    SetPauseWhenOpen(bool),
    SetPlaybackEndPolicy {
        keep_open_on_file_end: bool,
        playlist_auto_play_next: bool,
    },
    SetResumeLastPosition(bool),
    SetPaused(bool),
    TogglePause,
    /// A validated mpv input.conf action from the active key binding profile.
    MpvInputAction(String),
    MpvScriptKeys {
        owner: crate::key_bindings::MpvInputOwner,
        keys: Vec<String>,
    },
    PluginMpv(PluginMpvCommand),
    /// The optional request identifies the application stop that can release a player.
    Stop(Option<u32>),
    Seek {
        seconds: f64,
        absolute: bool,
    },
    /// A position on the visible timeline, expressed as a fraction of its range.
    SeekTimeline {
        fraction: f64,
        exact: bool,
    },
    SeekWithType {
        seconds: f64,
        seek_type: control_preferences::SeekType,
    },
    Volume(f64),
    RestoreVolume(f64),
    AudioDevice(String),
    SetReplayGain(ReplayGainOptions),
    AdjustVolume {
        delta: f64,
        boost: bool,
    },
    SetVolumeBoost(bool),
    ToggleMute,
    SetMuted(bool),
    Speed(f64),
    /// A player-arrow gesture can change rate and resume as one media intent.
    ControllerSpeed {
        speed: f64,
        resume: bool,
    },
    MultiplySpeed(f64),
    FrameStep,
    FrameBackStep,
    CaptureFrame,
    /// Capture a frame retained by the isolated, paused thumbnail decoder.
    CaptureThumbnailFrame,
    CaptureTransitionFrame,
    CaptureIncomingTransitionFrame,
    /// Sample the live video without presenting a second render surface.
    CaptureMaterialFrame,
    /// Observe an arbitrary, validated libmpv property for plugin events.
    ObserveMpvProperty(String),
    Screenshot {
        directory: String,
        content: screenshot::Content,
        encoding: screenshot::Encoding,
    },
    ScreenshotBinding {
        directory: String,
        default_content: screenshot::Content,
        encoding: screenshot::Encoding,
        binding: screenshot::Binding,
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
    RemoveDelogo,
    ToggleVideoFilter(String),
    AddVideoFilter(String),
    RemoveVideoFilter(crop::VideoFilter),
    RemoveVideoFilterAt(usize),
    SetVideoFilterAt {
        index: usize,
        filter: String,
    },
    ToggleAudioFilter(String),
    AddAudioFilter(String),
    RemoveAudioFilter(crop::VideoFilter),
    RemoveAudioFilterAt(usize),
    SetAudioFilterAt {
        index: usize,
        filter: String,
    },
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
    SetIccProfileAuto(bool),
    SetHdrEnabledWithIcc {
        enabled: bool,
        load_icc_profile: bool,
    },
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
    NetworkCache(network_preferences::Cache),
    NetworkOnlineVideos(network_preferences::OnlineVideos),
    VideoEqualizer {
        option: video_controls::Equalizer,
        value: i64,
    },
    PlaylistNext,
    PlaylistPrevious,
    TogglePlaylistShuffle,
    SetPlaylistShuffle(bool),
    SortPlaylist(crate::playlist::PlaylistSort),
    SelectTrack {
        kind: TrackKind,
        id: Option<i64>,
    },
    CycleTrack(TrackKind),
    SelectSecondarySubtitle(Option<i64>),
    SelectChapter(usize),
    ActivateChapter {
        index: usize,
        start: f64,
        notify: bool,
    },
    AdjustChapter(i32),
    SelectPlaylistEntry(usize),
    SelectPlaylistId(i64),
    RemovePlaylistId(i64),
    RemovePlaylistIds(Vec<i64>),
    RemoveDeletedPlaylistEntries(Vec<crate::PlaylistFileReference>),
    QueuePlaylistIdsNext(Vec<i64>),
    MovePlaylistIdsBefore {
        ids: Vec<i64>,
        before_id: Option<i64>,
    },
    MovePlaylistId {
        id: i64,
        direction: i32,
    },
    MovePlaylistIdBefore {
        id: i64,
        before_id: i64,
    },
    AppendFiles(Vec<String>),
    InsertPlaylistTrack {
        path: String,
        after_id: Option<i64>,
        make_current: bool,
    },
    InsertPlaylistFiles {
        paths: Vec<String>,
        index: usize,
    },
    InsertPlaylistFilesBefore {
        paths: Vec<String>,
        before_id: Option<i64>,
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
    AddSubtitlesForMedia {
        generation: u64,
        paths: Vec<String>,
    },
    AddSubtitlesWithoutSelection(Vec<String>),
    AddMatchedSubtitles {
        generation: u64,
        paths: Vec<String>,
        select_first: bool,
    },
    AddAudio(String),
    AddVideo(String),
    AudioDelay(f64),
    AudioEqualizer([f64; 10]),
    AdjustAudioDelay(f64),
    SubtitleDelay(f64),
    SecondarySubtitleDelay(f64),
    SecondarySubtitlePosition(f64),
    SubtitlePosition(f64),
    SubtitleScale(f64),
    MultiplySubtitleScale(f64),
    SubtitleFontSize(f64),
    SubtitleFont(String),
    SubtitleOutlineSize(f64),
    SubtitleBorderStyle(subtitle_controls::BorderStyle),
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
    ToggleLoopMode(LoopMode),
    SetLoopMode(LoopMode),
    CycleAbLoop,
    SetAbLoop {
        a: Option<f64>,
        b: Option<f64>,
    },
    SetAbLoopPoint {
        point: AbLoopPoint,
        value: Option<f64>,
    },
    /// Reserved lifecycle intent; public senders reject it. Drop player owners to close.
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbLoopPoint {
    A,
    B,
}

pub struct CommandPlan {
    pub commands: Vec<Vec<String>>,
    /// Resume after mpv initializes the manually selected source.
    pub resume_on_load: Option<LoadResume>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadResume {
    FollowPreference,
    Play,
}

impl Command {
    /// Clear queued work that belongs to the previously requested media.
    pub fn replaces_media(&self) -> bool {
        matches!(
            self,
            Self::Load(_)
                | Self::LoadAtPosition { .. }
                | Self::OpenFiles(_)
                | Self::OpenPlaylist(_)
                | Self::OpenShuffledPlaylist(_)
                | Self::OpenMediaLink(MediaLink { append: false, .. })
                | Self::OpenFilesWithSubtitles { .. }
                | Self::Stop(_)
        ) || matches!(self, Self::PluginMpv(command) if command.name() == "stop")
    }

    /// Reset navigation feedback when an intent may create its own decoder seek.
    pub fn restarts_decoder(&self) -> bool {
        self.replaces_media()
            || matches!(
                self,
                Self::HardwareDecoding(_)
                    | Self::VideoAspect(_)
                    | Self::VideoRotation(_)
                    | Self::Deinterlace(_)
                    | Self::ToggleDeinterlace
                    | Self::VideoCrop(_)
                    | Self::VideoCropRectangle(_)
                    | Self::VideoReflection { .. }
                    | Self::ToggleVideoReflection(_)
                    | Self::EnterCrop(_)
                    | Self::ExitCrop { .. }
            )
    }
}
