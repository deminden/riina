//! Validated playback intent and its native command plan.
mod arguments;
mod plan;

use crate::{
    LoopMode, TrackKind, control_preferences, crop, crop_session, hdr_preferences,
    network_preferences, playlist, screenshot, subtitle_color, subtitle_controls, video_controls,
    video_reflection, video_viewport,
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
    AudioDevice(String),
    AdjustVolume {
        delta: f64,
        boost: bool,
    },
    SetVolumeBoost(bool),
    ToggleMute,
    SetMuted(bool),
    Speed(f64),
    MultiplySpeed(f64),
    FrameStep,
    FrameBackStep,
    CaptureFrame,
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
    SetVideoFilterAt {
        index: usize,
        filter: String,
    },
    ToggleAudioFilter(String),
    AddAudioFilter(String),
    RemoveAudioFilter(crop::VideoFilter),
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
    SetHdrEnabled(bool),
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
    pub resume_on_load: bool,
}

impl Command {
    /// Clear queued work that belongs to the previously requested media.
    pub fn replaces_media(&self) -> bool {
        matches!(
            self,
            Self::Load(_)
                | Self::LoadAtPosition { .. }
                | Self::OpenFiles(_)
                | Self::OpenFilesWithSubtitles { .. }
                | Self::Stop
        )
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

    /// Clear the shuffle indicator when the user changes playlist contents or order.
    pub fn resets_playlist_shuffle(&self) -> bool {
        matches!(
            self,
            Self::SortPlaylist(_)
                | Self::MovePlaylistEntry { .. }
                | Self::MovePlaylistId { .. }
                | Self::RemovePlaylistEntry(_)
                | Self::RemovePlaylistId(_)
                | Self::RemovePlaylistIds(_)
                | Self::ClearPlaylist
                | Self::AppendFiles(_)
                | Self::OpenFiles(_)
                | Self::OpenFilesWithSubtitles { .. }
        )
    }
}
