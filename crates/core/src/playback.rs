//! Observed playback state; the backend remains the only playback clock.
use crate::{
    CapturedFrame, Chapter, LoopMode, PlaylistEntry, Track, TrackKind, audio_preferences, crop,
    music_mode, subtitle_color, video_controls,
};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::Arc, time::Instant};

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
    /// File-opening policy is intent; `paused` remains native observed state.
    pub pause_when_open: bool,
    pub generation: u64,
    pub phase: Phase,
    /// Version string from the loaded libmpv runtime, when that property is available.
    pub mpv_version: Option<String>,
    /// FFmpeg version reported by the loaded libmpv runtime, when available.
    pub ffmpeg_version: Option<String>,
    /// Observed libmpv filename-hashing policy for its watch-later files.
    pub ignore_path_in_watch_later_config: Option<bool>,
    /// Observed save-on-stop/quit policy; Stop writes watch-later only when enabled.
    pub save_position_on_quit: Option<bool>,
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
    /// Currently selected libmpv output device ID, independently observed so
    /// menus reflect runtime changes and backend fallback behavior.
    pub audio_device: Option<String>,
    pub audio_devices: Vec<audio_preferences::OutputDevice>,
    pub hardware_decoder: Option<String>,
    // Configured policy is separate from hwdec-current: unsupported codecs can
    // use software while the user's hardware-decoding switch remains enabled.
    pub hardware_decoding: Option<String>,
    pub video: Option<VideoParameters>,
    pub video_target: Option<VideoParameters>,
    pub video_output: Option<VideoParameters>,
    pub video_filters: Option<Vec<crop::VideoFilter>>,
    pub audio_filters: Option<Vec<crop::VideoFilter>>,
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
    /// Last pre-transition image retained while the next video initializes.
    pub transition_frame: Option<Arc<CapturedFrame>>,
    pub transition_frame_generation: Option<u64>,
    pub transition_frame_revision: u64,
    pub transition_frame_captured_at: Option<Instant>,
    /// Monotonic start time for the incoming side of a native video handoff.
    pub incoming_transition_started_at: Option<Instant>,
    /// First decoded frame from the incoming video, used to bridge native-surface startup.
    pub incoming_transition_frame: Option<Arc<CapturedFrame>>,
    pub incoming_transition_frame_generation: Option<u64>,
    /// Changes for every capture reply, including blank frames that must be retried.
    pub incoming_transition_frame_revision: u64,
    pub incoming_transition_frame_position: Option<f64>,
    /// Monotonic capture time used to keep the native-output handoff cover up
    /// for a real settling interval regardless of playback speed.
    pub incoming_transition_frame_captured_at: Option<Instant>,
    /// Latest optional video sample used only as the D3D11 material backdrop.
    pub material_frame: Option<Arc<CapturedFrame>>,
    pub material_frame_generation: Option<u64>,
    pub material_frame_position: Option<f64>,
    pub material_frame_captured_at: Option<Instant>,
    pub material_frame_revision: u64,
    /// Properties requested by active player-scoped plugin event listeners.
    pub observed_mpv_properties: BTreeMap<String, serde_json::Value>,
    /// Recent native mpv event names, retained so coalesced UI snapshots do not
    /// drop event callbacks before the plugin worker can receive them.
    pub mpv_events: Vec<MpvEventRecord>,
    /// Monotonic sequence for native mpv event records in `mpv_events`.
    pub mpv_event_sequence: u64,
    pub video_equalizer: [Option<i64>; 5],
    /// Decoded gain curve for Riina's labeled native audio equalizer filter.
    pub audio_equalizer: Option<[f64; 10]>,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub error: Option<String>,
    pub tracks: Vec<Track>,
    /// Whether mpv reported the incoming file's track list, including an empty list.
    pub track_list_observed: bool,
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

pub const MAX_RETAINED_MPV_EVENTS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MpvEventRecord {
    pub sequence: u64,
    pub name: String,
}

impl Playback {
    pub fn record_mpv_event(&mut self, name: String) {
        self.mpv_event_sequence = self.mpv_event_sequence.wrapping_add(1).max(1);
        self.mpv_events.push(MpvEventRecord {
            sequence: self.mpv_event_sequence,
            name,
        });
        if self.mpv_events.len() > MAX_RETAINED_MPV_EVENTS {
            self.mpv_events.remove(0);
        }
    }
}

#[cfg(test)]
mod mpv_event_record_tests {
    use super::*;

    #[test]
    fn native_mpv_event_history_is_bounded_and_sequenced() {
        let mut playback = Playback::default();
        for index in 0..=MAX_RETAINED_MPV_EVENTS {
            playback.record_mpv_event(format!("event-{index}"));
        }
        assert_eq!(playback.mpv_events.len(), MAX_RETAINED_MPV_EVENTS);
        assert_eq!(playback.mpv_events.first().unwrap().sequence, 2);
        assert_eq!(playback.mpv_events.last().unwrap().sequence, 257);
        assert_eq!(playback.mpv_events.last().unwrap().name, "event-256");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubtitleStyleAvailability {
    pub text_controls: bool,
    pub ass_warning: bool,
}

impl Playback {
    /// Use post-filter output to compute displayed geometry and aspect ratio.
    pub fn display_video_parameters(&self) -> Option<&VideoParameters> {
        self.video_output
            .as_ref()
            .or(self.video.as_ref())
            .or(self.video_target.as_ref())
    }

    /// Base natural window size on decoded source dimensions. In native VO
    /// mode output dimensions can describe the current target window.
    pub fn window_size_video_parameters(&self) -> Option<&VideoParameters> {
        self.video
            .as_ref()
            .or(self.video_output.as_ref())
            .or(self.video_target.as_ref())
    }

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
        let outgoing_generation = self.generation;
        self.generation += 1;
        self.incoming_transition_started_at = Some(Instant::now());
        self.phase = Phase::Loading;
        self.position = None;
        self.duration = None;
        self.seekable = false;
        self.ab_loop_a = None;
        self.ab_loop_b = None;
        self.error = None;
        // Track IDs belong to one file; never offer a stale selector during replacement.
        self.tracks.clear();
        self.track_list_observed = false;
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
        if self.transition_frame_generation != Some(outgoing_generation) {
            self.transition_frame = None;
            self.transition_frame_generation = None;
            self.transition_frame_captured_at = None;
        }
        self.incoming_transition_frame = None;
        self.incoming_transition_frame_generation = None;
        self.incoming_transition_frame_position = None;
        self.incoming_transition_frame_captured_at = None;
        self.material_frame = None;
        self.material_frame_generation = None;
        self.material_frame_position = None;
        self.material_frame_captured_at = None;
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

/// Approximate current-frame flags from mpv, separate from source parameters.
/// They can lag presentation; pixel validation must not rely on these alone.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct VideoFrameInfo {
    pub interlaced: Option<bool>,
    #[serde(rename = "tff")]
    pub top_field_first: Option<bool>,
}

/// Effective source parameters, not the renderer's output/display color space.
/// mpv can publish only container dimensions before a decoded frame exists.
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

#[cfg(test)]
mod display_video_parameters_tests {
    use super::{Playback, VideoParameters};

    fn video(width: u32, height: u32) -> VideoParameters {
        VideoParameters {
            pixelformat: None,
            w: width,
            h: height,
            dw: None,
            dh: None,
            rotate: None,
            colormatrix: None,
            colorlevels: None,
            primaries: None,
            gamma: None,
            max_luma: None,
            min_luma: None,
            signal_peak: None,
        }
    }

    #[test]
    fn natural_window_size_uses_source_dimensions_before_vo_output() {
        let playback = Playback {
            video: Some(video(320, 180)),
            video_target: Some(video(1920, 1080)),
            video_output: Some(video(1920, 1080)),
            ..Playback::default()
        };
        let natural = playback.window_size_video_parameters().unwrap();
        assert_eq!((natural.w, natural.h), (320, 180));
    }

    #[test]
    fn display_geometry_uses_post_filter_output_and_falls_back_to_source_then_target() {
        let source = Playback {
            video: Some(video(320, 180)),
            video_target: Some(video(1920, 1080)),
            video_output: Some(video(640, 360)),
            ..Playback::default()
        };
        let displayed = source.display_video_parameters().unwrap();
        assert_eq!((displayed.w, displayed.h), (640, 360));

        let target = Playback {
            video_target: Some(video(1920, 1080)),
            ..Playback::default()
        };
        let displayed = target.display_video_parameters().unwrap();
        assert_eq!((displayed.w, displayed.h), (1920, 1080));
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
