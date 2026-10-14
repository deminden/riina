//! Native-value feedback and reference-defined requested aspect feedback.
use riina_core::{Phase, Playback, speed_controls};
use std::collections::VecDeque;

#[derive(Debug, PartialEq)]
pub(crate) enum Kind {
    Aspect,
    AbLoop,
    AudioDelay,
    Crop,
    Deinterlace,
    FileStart,
    Mute,
    PauseResume,
    Speed,
    Equalizer,
    HardwareDecoding,
    LoopMode,
    Rotation,
    SubtitleScale,
    Volume,
    SubtitleDelay,
    SubtitlePosition,
    SubtitleVisibility,
    SecondarySubtitleDelay,
    SecondarySubtitlePosition,
    SecondarySubtitleVisibility,
    Progress,
    Stop,
    Chapter,
    PlaylistAdd,
    PlaylistClear,
    Track,
}

#[derive(Debug, PartialEq)]
pub(crate) struct Message {
    pub kind: Kind,
    pub text: String,
    pub progress: Option<f32>,
}

pub(crate) fn progress_message(position: Option<f64>, duration: Option<f64>) -> Option<Message> {
    let position = position.filter(|value| value.is_finite() && *value >= 0.0)?;
    let duration = duration.filter(|value| value.is_finite() && *value > 0.0)?;
    Some(Message {
        kind: Kind::Progress,
        text: format!(
            "{} / {}",
            precise_time_label(position),
            precise_time_label(duration)
        ),
        progress: Some((position / duration).clamp(0.0, 1.0) as f32),
    })
}

fn precise_time_label(seconds: f64) -> String {
    let total_milliseconds = (seconds * 1000.0).round().min(u64::MAX as f64) as u64;
    let total_seconds = total_milliseconds / 1000;
    let milliseconds = total_milliseconds % 1000;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}.{milliseconds:03}")
    } else {
        format!("{minutes}:{seconds:02}.{milliseconds:03}")
    }
}

pub(crate) enum ActionFeedback {
    Stop,
    Chapter(String),
    PlaylistAdded(usize),
    PlaylistCleared,
}

pub(crate) fn action_message(action: ActionFeedback) -> Message {
    let (kind, text) = match action {
        ActionFeedback::Stop => (Kind::Stop, "Stop".to_owned()),
        ActionFeedback::Chapter(title) => (Kind::Chapter, format!("Chapter: {title}")),
        ActionFeedback::PlaylistAdded(count) => (
            Kind::PlaylistAdd,
            format!("Added {count} Files to Playlist"),
        ),
        ActionFeedback::PlaylistCleared => (Kind::PlaylistClear, "Cleared Playlist".into()),
    };
    Message {
        kind,
        text,
        progress: None,
    }
}

pub(crate) fn file_start_message(title: &str, enabled: bool, disabled: bool) -> Option<Message> {
    (enabled && !disabled && !title.is_empty()).then(|| Message {
        kind: Kind::FileStart,
        text: title.to_owned(),
        progress: None,
    })
}

#[derive(Default)]
pub(crate) struct VideoOsd {
    previous: Option<Playback>,
    disable_speed: bool,
    disable_pause_resume: bool,
    volume_limit: f64,
    pending: VecDeque<Message>,
}

impl VideoOsd {
    pub(crate) fn new(disable_speed: bool, disable_pause_resume: bool) -> Self {
        Self {
            disable_speed,
            disable_pause_resume,
            volume_limit: 100.0,
            ..Default::default()
        }
    }

    pub(crate) fn set_volume_limit(&mut self, limit: f64) {
        if limit.is_finite() && limit > 0.0 {
            self.volume_limit = limit;
        }
    }

    pub(crate) fn set_disable_speed(&mut self, disabled: bool) {
        self.disable_speed = disabled;
    }

    pub(crate) fn set_disable_pause_resume(&mut self, disabled: bool) {
        self.disable_pause_resume = disabled;
    }

    pub(crate) fn observe(&mut self, state: &Playback) -> Option<Message> {
        if self.previous.as_ref().is_some_and(|previous| {
            previous.generation != state.generation
                || matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed)
        }) {
            self.pending.clear();
        }
        // Ignore the suppressed category before choosing a message. Filtering
        // only afterwards would swallow a hardware change coalesced with speed.
        if self.disable_speed
            && let Some(previous) = &mut self.previous
        {
            previous.speed = state.speed;
        }
        if self.disable_pause_resume
            && let Some(previous) = &mut self.previous
        {
            previous.paused = state.paused;
        }
        let mut messages = changed_all(self.previous.as_ref(), state);
        for message in &mut messages {
            if message.kind == Kind::Volume && state.volume.is_finite() {
                message.progress = Some((state.volume / self.volume_limit).clamp(0.0, 1.0) as f32);
            }
        }
        // Catalog caches intentionally ignore clock/settings-only snapshots.
        // Consume every observation here, retaining only feedback fields so a
        // prior edit cannot repeatedly mask a later edit or restart its timer.
        self.previous = Some(Playback {
            generation: state.generation,
            phase: state.phase,
            paused: state.paused,
            volume: state.volume,
            muted: state.muted,
            speed: state.speed,
            hardware_decoding: state.hardware_decoding.clone(),
            video_equalizer: state.video_equalizer,
            video_aspect: state.video_aspect.clone(),
            aspect_requests: state.aspect_requests,
            deinterlace: state.deinterlace,
            video_rotation: state.video_rotation,
            video: state.video.clone(),
            video_filters: state.video_filters.clone(),
            subtitle_scale: state.subtitle_scale,
            loop_file: state.loop_file.clone(),
            loop_playlist: state.loop_playlist.clone(),
            ab_loop_a: state.ab_loop_a,
            ab_loop_b: state.ab_loop_b,
            position: state.position,
            chapters: state.chapters.clone(),
            tracks: state.tracks.clone(),
            audio_delay: state.audio_delay,
            subtitle_delay: state.subtitle_delay,
            subtitle_position: state.subtitle_position,
            subtitle_visible: state.subtitle_visible,
            secondary_subtitle_visible: state.secondary_subtitle_visible,
            secondary_subtitle_delay: state.secondary_subtitle_delay,
            secondary_subtitle_position: state.secondary_subtitle_position,
            ..Default::default()
        });
        // Re-enabling feedback must not replay a suppressed change, and a
        // coalesced native snapshot must retain each independent message.
        let messages_waiting_for_display = !self.pending.is_empty();
        self.pending.extend(
            messages
                .into_iter()
                .filter(|message| !(self.disable_speed && message.kind == Kind::Speed)),
        );
        if messages_waiting_for_display {
            None
        } else {
            self.pending.pop_front()
        }
    }

    pub(crate) fn take_pending(&mut self) -> Option<Message> {
        self.pending.pop_front()
    }

    pub(crate) fn clear_pending(&mut self) {
        self.pending.clear();
    }
}

#[cfg(test)]
pub(crate) fn changed(previous: Option<&Playback>, state: &Playback) -> Option<Message> {
    changed_one(previous, state)
}

fn changed_all(previous: Option<&Playback>, state: &Playback) -> Vec<Message> {
    let Some(previous) = previous else {
        return Vec::new();
    };
    let mut baseline = previous.clone();
    let mut messages = Vec::new();
    while let Some(message) = changed_one(Some(&baseline), state) {
        advance_baseline(&mut baseline, state, &message.kind);
        messages.push(message);
    }
    messages
}

fn advance_baseline(baseline: &mut Playback, state: &Playback, kind: &Kind) {
    match kind {
        Kind::Aspect => baseline.aspect_requests = state.aspect_requests,
        Kind::AbLoop => {
            // Native observations can coalesce both endpoints into one
            // snapshot. Advance one endpoint at a time so changed_all can
            // preserve both feedback messages. A full clear is one action.
            if state.ab_loop_a.is_none() && state.ab_loop_b.is_none() {
                baseline.ab_loop_a = None;
                baseline.ab_loop_b = None;
            } else if baseline.ab_loop_a != state.ab_loop_a {
                baseline.ab_loop_a = state.ab_loop_a;
            } else {
                baseline.ab_loop_b = state.ab_loop_b;
            }
        }
        Kind::AudioDelay => baseline.audio_delay = state.audio_delay,
        Kind::Crop => {
            baseline.video = state.video.clone();
            baseline.video_filters = state.video_filters.clone();
        }
        Kind::Deinterlace => baseline.deinterlace = state.deinterlace,
        Kind::Mute => baseline.muted = state.muted,
        Kind::PauseResume => baseline.paused = state.paused,
        Kind::Speed => baseline.speed = state.speed,
        Kind::Equalizer => baseline.video_equalizer = state.video_equalizer,
        Kind::HardwareDecoding => baseline.hardware_decoding = state.hardware_decoding.clone(),
        Kind::LoopMode => {
            baseline.loop_file = state.loop_file.clone();
            baseline.loop_playlist = state.loop_playlist.clone();
        }
        Kind::Rotation => baseline.video_rotation = state.video_rotation,
        Kind::SubtitleScale => baseline.subtitle_scale = state.subtitle_scale,
        Kind::Volume => baseline.volume = state.volume,
        Kind::SubtitleDelay => baseline.subtitle_delay = state.subtitle_delay,
        Kind::SubtitlePosition => baseline.subtitle_position = state.subtitle_position,
        Kind::SubtitleVisibility => baseline.subtitle_visible = state.subtitle_visible,
        Kind::SecondarySubtitleDelay => {
            baseline.secondary_subtitle_delay = state.secondary_subtitle_delay;
        }
        Kind::SecondarySubtitlePosition => {
            baseline.secondary_subtitle_position = state.secondary_subtitle_position;
        }
        Kind::SecondarySubtitleVisibility => {
            baseline.secondary_subtitle_visible = state.secondary_subtitle_visible;
        }
        Kind::Track => baseline.tracks.clone_from(&state.tracks),
        Kind::FileStart
        | Kind::Progress
        | Kind::Stop
        | Kind::Chapter
        | Kind::PlaylistAdd
        | Kind::PlaylistClear => {}
    }
}

fn changed_one(previous: Option<&Playback>, state: &Playback) -> Option<Message> {
    let previous = previous?;
    if previous.generation != state.generation
        || matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed)
        || !matches!(
            previous.phase,
            Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Seeking | Phase::Ended
        )
    {
        return None;
    }
    // Aspect feedback reports the exact submitted text even for a no-op or a
    // native parse failure. A numeric ratio cannot preserve that distinction.
    if previous.aspect_requests != state.aspect_requests {
        return Some(Message {
            kind: Kind::Aspect,
            text: format!("Aspect Ratio: {}", state.video_aspect.requested_text()),
            progress: None,
        });
    }
    if previous.paused != state.paused {
        return Some(Message {
            kind: Kind::PauseResume,
            text: if state.paused { "Paused" } else { "Resumed" }.into(),
            progress: None,
        });
    }
    if previous.muted != state.muted {
        return Some(Message {
            kind: Kind::Mute,
            text: if state.muted { "Mute" } else { "Unmute" }.into(),
            progress: None,
        });
    }
    if state.volume.is_finite() && previous.volume.is_finite() && previous.volume != state.volume {
        return Some(Message {
            kind: Kind::Volume,
            text: format!("Volume: {:.0}", state.volume),
            progress: Some((state.volume / 100.0).clamp(0.0, 1.0) as f32),
        });
    }
    if state.error.is_some() {
        return None;
    }
    if let Some(enabled) = state.deinterlace
        && previous.deinterlace.unwrap_or(false) != enabled
    {
        return Some(Message {
            kind: Kind::Deinterlace,
            text: format!("Deinterlace: {}", if enabled { "On" } else { "Off" }),
            progress: None,
        });
    }
    // One native snapshot can coalesce several channels. Display one complete
    // message rather than mixing one channel's label with another's progress.
    for (index, label) in ["Brightness", "Contrast", "Saturation", "Gamma", "Hue"]
        .into_iter()
        .enumerate()
        .rev()
    {
        if let (Some(old), Some(value)) = (
            previous.video_equalizer[index],
            state.video_equalizer[index],
        ) && old != value
        {
            return Some(Message {
                kind: Kind::Equalizer,
                text: format!("{label}: {value}"),
                progress: Some((value as f32 + 100.0) / 200.0),
            });
        }
    }
    if state.speed != previous.speed {
        return Some(Message {
            kind: Kind::Speed,
            text: format!("Speed: {}x", speed_controls::text(state.speed)),
            progress: None,
        });
    }
    if let (Some(old), Some(mode)) = (&previous.hardware_decoding, &state.hardware_decoding)
        && old != mode
    {
        return Some(Message {
            kind: Kind::HardwareDecoding,
            text: format!(
                "Hardware Decoding: {}",
                if mode == "no" { "Off" } else { "On" }
            ),
            progress: None,
        });
    }
    if let (Some(old), Some(rotation)) = (previous.video_rotation, state.video_rotation)
        && old != rotation
    {
        return Some(Message {
            kind: Kind::Rotation,
            text: format!("Rotate: {rotation}°"),
            progress: None,
        });
    }
    if let (Some(old), Some(scale)) = (previous.subtitle_scale, state.subtitle_scale)
        && old.is_finite()
        && scale.is_finite()
        && old != scale
        && scale != 0.0
    {
        let display = if scale >= 1.0 { scale } else { -1.0 / scale };
        let display = (display * 100.0).round() / 100.0;
        return Some(Message {
            kind: Kind::SubtitleScale,
            text: format!("Subtitle Scale: {}x", compact_number(display)),
            progress: None,
        });
    }
    if let (Some(old), Some(current)) = (observed_crop_osd(previous), observed_crop_osd(state))
        && old != current
    {
        let label = current.unwrap_or_else(|| "None".into());
        return Some(Message {
            kind: Kind::Crop,
            text: format!("Crop: {label}"),
            progress: None,
        });
    }
    if let (Some(old), Some(current)) = (loop_mode(previous), loop_mode(state))
        && old != current
    {
        return Some(Message {
            kind: Kind::LoopMode,
            text: format!("Loop: {current}"),
            progress: None,
        });
    }
    if previous.ab_loop_a != state.ab_loop_a || previous.ab_loop_b != state.ab_loop_b {
        let both_cleared = state.ab_loop_a.is_none()
            && state.ab_loop_b.is_none()
            && (previous.ab_loop_a.is_some() || previous.ab_loop_b.is_some());
        let (label, point) = if previous.ab_loop_a != state.ab_loop_a {
            ("A", state.ab_loop_a)
        } else {
            ("B", state.ab_loop_b)
        };
        let text = if both_cleared {
            "AB-Loop: Cleared".into()
        } else if let Some(point) = point {
            format!("AB-Loop: {label} {}", riina_core::time_label(Some(point)))
        } else {
            format!("AB-Loop: {label}")
        };
        return Some(Message {
            kind: Kind::AbLoop,
            text,
            progress: None,
        });
    }
    if previous.audio_delay.is_finite()
        && state.audio_delay.is_finite()
        && previous.audio_delay != state.audio_delay
    {
        return Some(delay_message("Audio Delay", state.audio_delay));
    }
    if previous.subtitle_delay.is_finite()
        && state.subtitle_delay.is_finite()
        && previous.subtitle_delay != state.subtitle_delay
    {
        return Some(delay_message("Subtitle Delay", state.subtitle_delay));
    }
    if previous
        .secondary_subtitle_delay
        .is_some_and(f64::is_finite)
        && previous.secondary_subtitle_delay != state.secondary_subtitle_delay
        && let Some(delay) = state.secondary_subtitle_delay
        && delay.is_finite()
    {
        return Some(delay_message("Secondary Subtitle Delay", delay));
    }
    if previous.subtitle_position.is_some_and(f64::is_finite)
        && previous.subtitle_position != state.subtitle_position
        && let Some(position) = state.subtitle_position
        && position.is_finite()
    {
        return Some(position_message("Subtitle Position", position));
    }
    if previous
        .secondary_subtitle_position
        .is_some_and(f64::is_finite)
        && previous.secondary_subtitle_position != state.secondary_subtitle_position
        && let Some(position) = state.secondary_subtitle_position
        && position.is_finite()
    {
        return Some(position_message("Secondary Subtitle Position", position));
    }
    if previous.subtitle_visible != state.subtitle_visible {
        return Some(Message {
            kind: Kind::SubtitleVisibility,
            text: if state.subtitle_visible {
                "Subtitles Visible"
            } else {
                "Subtitles Hidden"
            }
            .into(),
            progress: None,
        });
    }
    if let (Some(old), Some(visible)) = (
        previous.secondary_subtitle_visible,
        state.secondary_subtitle_visible,
    ) && old != visible
    {
        return Some(Message {
            kind: Kind::SecondarySubtitleVisibility,
            text: if visible {
                "Second Subtitles Visible"
            } else {
                "Second Subtitles Hidden"
            }
            .into(),
            progress: None,
        });
    }
    for (kind, secondary, label) in [
        (riina_core::TrackKind::Audio, false, "Audio"),
        (riina_core::TrackKind::Video, false, "Video"),
        (riina_core::TrackKind::Sub, false, "Subtitle"),
        (riina_core::TrackKind::Sub, true, "Second Subtitle"),
    ] {
        let old = selected_track(previous, kind, secondary);
        let current = selected_track(state, kind, secondary);
        if old.map(|track| track.id) != current.map(|track| track.id) {
            let languages = crate::language_names::LanguageNames::current();
            return Some(Message {
                kind: Kind::Track,
                text: format!("{label}: {}", readable_track_title(current, &languages)),
                progress: None,
            });
        }
    }
    None
}

/// Return an observed crop label, keeping unavailable native state distinct
/// from a confirmed absence of Riina's managed crop filter.
fn observed_crop_osd(state: &Playback) -> Option<Option<String>> {
    let index = riina_core::crop::observed_index(state)?;
    if index == 0 {
        return Some(None);
    }
    let custom_index = riina_core::crop::Preset::ALL.len() + 1;
    if index == custom_index {
        let filter = state
            .video_filters
            .as_ref()?
            .iter()
            .find(|filter| filter.label.as_deref() == Some(riina_core::crop::FILTER_LABEL))?;
        let param = |name: &str, position: &str| {
            filter
                .params
                .get(name)
                .or_else(|| filter.params.get(position))
                .and_then(|value| value.parse::<u32>().ok())
        };
        let rectangle = param("x", "@2")
            .zip(param("y", "@3"))
            .zip(param("w", "@0").zip(param("h", "@1")));
        return Some(Some(match rectangle {
            Some(((x, y), (width, height))) => {
                format!("({x}, {y}) ({width}×{height})")
            }
            None => "Custom".into(),
        }));
    }
    let label = riina_core::crop::LABELS.get(index).copied()?;
    Some(Some(label.trim_end_matches('…').to_owned()))
}

fn delay_message(label: &str, value: f64) -> Message {
    let text = if value == 0.0 {
        format!("{label}: No Delay")
    } else {
        format!(
            "{label}: {}s {}",
            compact_number(value.abs()),
            if value > 0.0 { "Later" } else { "Earlier" }
        )
    };
    Message {
        kind: match label {
            "Audio Delay" => Kind::AudioDelay,
            "Subtitle Delay" => Kind::SubtitleDelay,
            _ => Kind::SecondarySubtitleDelay,
        },
        text,
        progress: Some(((value + 10.0).clamp(0.0, 20.0) / 20.0) as f32),
    }
}

fn position_message(label: &str, value: f64) -> Message {
    Message {
        kind: if label == "Subtitle Position" {
            Kind::SubtitlePosition
        } else {
            Kind::SecondarySubtitlePosition
        },
        text: format!("{label}: {}", compact_number(value)),
        progress: Some((value / 100.0) as f32),
    }
}

fn compact_number(value: f64) -> String {
    let mut text = format!("{value:.6}");
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

fn loop_mode(state: &Playback) -> Option<&'static str> {
    let file = state.loop_file.as_deref()?;
    let playlist = state.loop_playlist.as_deref()?;
    if loop_enabled(file) {
        Some("File")
    } else if loop_enabled(playlist) {
        Some("Playlist")
    } else {
        Some("Off")
    }
}

fn loop_enabled(value: &str) -> bool {
    !matches!(value, "no" | "0" | "")
}

fn selected_track(
    state: &Playback,
    kind: riina_core::TrackKind,
    secondary: bool,
) -> Option<&riina_core::Track> {
    state.tracks.iter().find(|track| {
        track.kind == kind
            && if kind == riina_core::TrackKind::Sub {
                if secondary {
                    track.selected && track.main_selection == Some(1)
                } else {
                    track.is_primary_selected()
                }
            } else {
                !secondary && track.selected
            }
    })
}

fn readable_track_title(
    track: Option<&riina_core::Track>,
    languages: &crate::language_names::LanguageNames,
) -> String {
    let Some(track) = track else {
        // IINA represents the no-track choice as the synthetic track #0.
        return "#0 <None>".into();
    };
    let language = track
        .lang
        .as_deref()
        .filter(|language| !language.is_empty() && *language != "und")
        .map(|language| {
            let name = languages.display_name(Some(language));
            if name.is_empty() {
                String::new()
            } else {
                format!("[{name}] ")
            }
        })
        .unwrap_or_default();
    format!("#{} {language}{}", track.id, track.display_name())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn loaded() -> Playback {
        Playback {
            generation: 1,
            phase: Phase::Paused,
            speed: 1.0,
            video_equalizer: [Some(0); 5],
            hardware_decoding: Some("auto".into()),
            ..Default::default()
        }
    }

    #[test]
    fn explicit_progress_feedback_uses_precise_clock_and_bounded_progress() {
        let message = progress_message(Some(3661.2345), Some(3662.0)).unwrap();
        assert_eq!(message.kind, Kind::Progress);
        assert_eq!(message.text, "1:01:01.235 / 1:01:02.000");
        assert_eq!(message.progress, Some((3661.2345_f64 / 3662.0) as f32));

        let beyond_end = progress_message(Some(20.0), Some(10.0)).unwrap();
        assert_eq!(beyond_end.progress, Some(1.0));
        assert_eq!(progress_message(Some(1.0), None), None);
        assert_eq!(progress_message(Some(f64::NAN), Some(2.0)), None);
        assert_eq!(progress_message(Some(0.0), Some(0.0)), None);
    }

    #[test]
    fn accepted_action_feedback_uses_reference_stop_chapter_and_playlist_messages() {
        for (action, kind, text) in [
            (ActionFeedback::Stop, Kind::Stop, "Stop".to_owned()),
            (
                ActionFeedback::Chapter("Opening".into()),
                Kind::Chapter,
                "Chapter: Opening".to_owned(),
            ),
            (
                ActionFeedback::PlaylistAdded(3),
                Kind::PlaylistAdd,
                "Added 3 Files to Playlist".to_owned(),
            ),
            (
                ActionFeedback::PlaylistCleared,
                Kind::PlaylistClear,
                "Cleared Playlist".to_owned(),
            ),
        ] {
            let message = action_message(action);
            assert_eq!(message.kind, kind);
            assert_eq!(message.text, text);
            assert_eq!(message.progress, None);
        }
    }

    #[test]
    fn coalesced_ab_loop_endpoints_each_receive_feedback() {
        let mut feedback = VideoOsd::default();
        let mut state = loaded();
        assert_eq!(feedback.observe(&state), None);

        state.ab_loop_a = Some(3.0);
        state.ab_loop_b = Some(9.0);
        assert_eq!(feedback.observe(&state).unwrap().text, "AB-Loop: A 0:03");
        assert_eq!(feedback.take_pending().unwrap().text, "AB-Loop: B 0:09");
        assert_eq!(feedback.take_pending(), None);
    }

    #[test]
    fn volume_feedback_progress_tracks_the_default_and_boost_caps() {
        let mut feedback = VideoOsd::default();
        let mut state = loaded();
        state.volume = 40.0;
        assert_eq!(feedback.observe(&state), None);

        state.volume = 100.0;
        let default_cap = feedback.observe(&state).unwrap();
        assert_eq!(default_cap.kind, Kind::Volume);
        assert_eq!(default_cap.text, "Volume: 100");
        assert_eq!(default_cap.progress, Some(1.0));

        feedback.set_volume_limit(200.0);
        for (volume, progress) in [(125.0, 0.625), (200.0, 1.0), (240.0, 1.0)] {
            state.volume = volume;
            let message = feedback.observe(&state).unwrap();
            assert_eq!(message.kind, Kind::Volume);
            assert_eq!(message.text, format!("Volume: {volume:.0}"));
            assert_eq!(message.progress, Some(progress));
        }
    }

    #[test]
    fn simultaneous_ab_loop_clear_is_reported_once() {
        let mut feedback = VideoOsd::default();
        let mut state = loaded();
        state.ab_loop_a = Some(3.0);
        state.ab_loop_b = Some(9.0);
        assert_eq!(feedback.observe(&state), None);

        state.ab_loop_a = None;
        state.ab_loop_b = None;
        assert_eq!(feedback.observe(&state).unwrap().text, "AB-Loop: Cleared");
        assert_eq!(feedback.take_pending(), None);
    }

    #[test]
    fn feedback_requires_confirmed_changes_in_the_same_media_generation() {
        let old = loaded();
        let mut state = old.clone();
        assert_eq!(changed(None, &state), None);
        assert_eq!(changed(Some(&old), &state), None);
        state.speed = 1.234567;
        assert_eq!(
            changed(Some(&old), &state),
            Some(Message {
                kind: Kind::Speed,
                text: "Speed: 1.234567x".into(),
                progress: None
            })
        );
        state.generation += 1;
        assert_eq!(changed(Some(&old), &state), None);
        state.generation = old.generation;
        state.error = Some("Rejected command".into());
        assert_eq!(changed(Some(&old), &state), None);
    }

    #[test]
    fn crop_feedback_reports_confirmed_presets_and_custom_rectangle_changes() {
        use riina_core::crop::{FILTER_LABEL, VideoFilter};

        let mut feedback = VideoOsd::default();
        let mut state = loaded();
        state.video =
            Some(serde_json::from_value(serde_json::json!({"w": 320, "h": 180})).unwrap());
        state.video_filters = Some(vec![]);
        assert_eq!(feedback.observe(&state), None);

        state.video_filters = Some(vec![VideoFilter {
            name: "crop".into(),
            label: Some(FILTER_LABEL.into()),
            enabled: Some(true),
            params: [("w".into(), "240".into()), ("h".into(), "180".into())].into(),
        }]);
        assert_eq!(feedback.observe(&state).unwrap().text, "Crop: 4:3");

        state.video_filters = Some(vec![VideoFilter {
            name: "crop".into(),
            label: Some(FILTER_LABEL.into()),
            enabled: Some(true),
            params: [
                ("w".into(), "300".into()),
                ("h".into(), "170".into()),
                ("x".into(), "20".into()),
                ("y".into(), "10".into()),
            ]
            .into(),
        }]);
        assert_eq!(
            feedback.observe(&state).unwrap().text,
            "Crop: (20, 10) (300×170)"
        );

        state.video_filters.as_mut().unwrap()[0]
            .params
            .insert("x".into(), "21".into());
        assert_eq!(
            feedback.observe(&state).unwrap().text,
            "Crop: (21, 10) (300×170)"
        );

        state.video_filters = Some(vec![]);
        assert_eq!(feedback.observe(&state).unwrap().text, "Crop: None");
        assert_eq!(feedback.observe(&state), None);
    }

    #[test]
    fn crop_feedback_does_not_treat_unknown_filter_state_as_no_crop() {
        use riina_core::crop::{FILTER_LABEL, VideoFilter};

        let mut feedback = VideoOsd::default();
        let mut state = loaded();
        state.video =
            Some(serde_json::from_value(serde_json::json!({"w": 320, "h": 180})).unwrap());
        assert_eq!(feedback.observe(&state), None);

        state.video_filters = Some(vec![VideoFilter {
            name: "crop".into(),
            label: Some(FILTER_LABEL.into()),
            enabled: Some(true),
            params: [
                ("w".into(), "240".into()),
                ("h".into(), "180".into()),
                ("x".into(), "0".into()),
                ("y".into(), "0".into()),
            ]
            .into(),
        }]);
        assert_eq!(feedback.observe(&state), None);
    }

    #[test]
    fn feedback_consumes_each_observation_independently_of_catalog_updates() {
        let mut feedback = VideoOsd::default();
        let mut state = loaded();
        assert_eq!(feedback.observe(&state), None);
        state.video_equalizer[0] = Some(-27);
        assert_eq!(feedback.observe(&state).unwrap().text, "Brightness: -27");
        assert_eq!(feedback.observe(&state), None);
        state.speed = 2.0;
        assert_eq!(feedback.observe(&state).unwrap().text, "Speed: 2x");
        state.position = Some(4.0);
        assert_eq!(feedback.observe(&state), None);
    }

    #[test]
    fn pause_resume_messages_follow_confirmed_playback_state_and_can_be_suppressed() {
        let mut feedback = VideoOsd::new(false, false);
        let mut state = loaded();
        assert_eq!(feedback.observe(&state), None);
        state.paused = true;
        assert_eq!(feedback.observe(&state).unwrap().text, "Paused");
        state.paused = false;
        assert_eq!(feedback.observe(&state).unwrap().text, "Resumed");

        feedback.set_disable_pause_resume(true);
        state.paused = true;
        assert_eq!(feedback.observe(&state), None);
        state.video_equalizer[0] = Some(-27);
        assert_eq!(feedback.observe(&state).unwrap().kind, Kind::Equalizer);
        feedback.set_disable_pause_resume(false);
        assert_eq!(feedback.observe(&state), None);
    }

    #[test]
    fn file_start_feedback_obeys_global_and_category_suppression() {
        assert!(file_start_message("clip.mkv", true, false).is_some());
        assert!(file_start_message("clip.mkv", false, false).is_none());
        assert!(file_start_message("clip.mkv", true, true).is_none());
        assert!(file_start_message("", true, false).is_none());
    }

    #[test]
    fn speed_suppression_consumes_speed_but_keeps_other_feedback() {
        let mut feedback = VideoOsd::default();
        let mut state = loaded();
        assert_eq!(feedback.observe(&state), None);
        feedback.set_disable_speed(true);
        state.speed = 2.0;
        assert_eq!(feedback.observe(&state), None);
        state.video_equalizer[0] = Some(-27);
        assert_eq!(feedback.observe(&state).unwrap().kind, Kind::Equalizer);
        state.hardware_decoding = Some("no".into());
        state.speed = 3.0;
        assert_eq!(
            feedback.observe(&state).unwrap().text,
            "Hardware Decoding: Off"
        );
        feedback.set_disable_speed(false);
        assert_eq!(feedback.observe(&state), None);
        state.speed = 1.0;
        assert_eq!(feedback.observe(&state).unwrap().kind, Kind::Speed);
    }
    #[test]
    fn equalizer_progress_and_hardware_policy_keep_distinct_semantics() {
        let old = loaded();
        let mut state = old.clone();
        for (value, progress) in [(-100, 0.0), (0, 0.5), (100, 1.0)] {
            let mut previous = old.clone();
            previous.video_equalizer[0] = Some(1);
            state.video_equalizer[0] = Some(value);
            assert_eq!(
                changed(Some(&previous), &state),
                Some(Message {
                    kind: Kind::Equalizer,
                    text: format!("Brightness: {value}"),
                    progress: Some(progress)
                })
            );
        }
        state = old.clone();
        state.hardware_decoder = Some("no".into());
        assert_eq!(changed(Some(&old), &state), None);
        state.hardware_decoding = Some("no".into());
        assert_eq!(
            changed(Some(&old), &state).unwrap().text,
            "Hardware Decoding: Off"
        );
        state.hardware_decoding = Some("auto-copy".into());
        assert_eq!(
            changed(Some(&old), &state).unwrap().text,
            "Hardware Decoding: On"
        );
    }
    #[test]
    fn aspect_feedback_retains_raw_text_repeats_and_native_failures() {
        use riina_core::video_controls::AspectRatio;
        let mut feedback = VideoOsd::default();
        let mut state = loaded();
        assert_eq!(feedback.observe(&state), None);
        for text in ["8:6", "8:6", "Default", "invalid", "١٦:٩"] {
            state.video_aspect = AspectRatio::from_text(text);
            state.aspect_requests += 1;
            state.error = Some("Native parse failure".into());
            assert_eq!(
                feedback.observe(&state),
                Some(Message {
                    kind: Kind::Aspect,
                    text: format!("Aspect Ratio: {text}"),
                    progress: None,
                })
            );
            assert_eq!(feedback.observe(&state), None);
        }
        state.generation += 1;
        state.aspect_requests += 1;
        assert_eq!(feedback.observe(&state), None);
        assert_eq!(feedback.observe(&state), None);
    }

    #[test]
    fn deinterlace_feedback_uses_confirmed_changes_not_filter_activity() {
        let mut feedback = VideoOsd::default();
        let mut state = loaded();
        state.deinterlace = Some(false);
        assert_eq!(feedback.observe(&state), None);
        for (enabled, label) in [(true, "On"), (false, "Off")] {
            state.deinterlace = Some(enabled);
            assert_eq!(
                feedback.observe(&state),
                Some(Message {
                    kind: Kind::Deinterlace,
                    text: format!("Deinterlace: {label}"),
                    progress: None,
                })
            );
            assert_eq!(feedback.observe(&state), None);
            state.deinterlace_active = Some(!enabled);
            assert_eq!(feedback.observe(&state), None);
        }
        state.deinterlace = None;
        assert_eq!(feedback.observe(&state), None);
        state.deinterlace = Some(true);
        state.error = Some("Native command failure".into());
        assert_eq!(feedback.observe(&state), None);
        state.error = None;
        assert_eq!(feedback.observe(&state), None);
        state.generation += 1;
        state.deinterlace = Some(false);
        assert_eq!(feedback.observe(&state), None);
    }

    #[test]
    fn delay_feedback_preserves_lane_direction_and_progress() {
        let old = loaded();
        let mut state = old.clone();
        state.audio_delay = -0.5;
        let audio = changed(Some(&old), &state).unwrap();
        assert_eq!(audio.kind, Kind::AudioDelay);
        assert_eq!(audio.text, "Audio Delay: 0.5s Earlier");
        assert_eq!(audio.progress, Some(0.475));

        let mut old = loaded();
        old.subtitle_delay = 0.0;
        let mut state = old.clone();
        state.subtitle_delay = 1.25;
        let primary = changed(Some(&old), &state).unwrap();
        assert_eq!(primary.kind, Kind::SubtitleDelay);
        assert_eq!(primary.text, "Subtitle Delay: 1.25s Later");
        assert_eq!(primary.progress, Some(0.5625));

        old.secondary_subtitle_delay = Some(0.0);
        state = old.clone();
        state.secondary_subtitle_delay = Some(-2.0);
        let secondary = changed(Some(&old), &state).unwrap();
        assert_eq!(secondary.kind, Kind::SecondarySubtitleDelay);
        assert_eq!(secondary.text, "Secondary Subtitle Delay: 2s Earlier");
        assert_eq!(secondary.progress, Some(0.4));
    }

    #[test]
    fn subtitle_position_feedback_keeps_primary_and_secondary_lanes_distinct() {
        let mut old = loaded();
        old.subtitle_position = Some(50.0);
        let mut state = old.clone();
        state.subtitle_position = Some(75.0);
        let primary = changed(Some(&old), &state).unwrap();
        assert_eq!(primary.kind, Kind::SubtitlePosition);
        assert_eq!(primary.text, "Subtitle Position: 75");
        assert_eq!(primary.progress, Some(0.75));

        old.secondary_subtitle_position = Some(30.0);
        state = old.clone();
        state.secondary_subtitle_position = Some(65.0);
        let secondary = changed(Some(&old), &state).unwrap();
        assert_eq!(secondary.kind, Kind::SecondarySubtitlePosition);
        assert_eq!(secondary.text, "Secondary Subtitle Position: 65");
        assert_eq!(secondary.progress, Some(0.65));
    }

    #[test]
    fn subtitle_visibility_feedback_identifies_the_changed_lane() {
        let old = loaded();
        let mut state = old.clone();
        state.subtitle_visible = true;
        let primary = changed(Some(&old), &state).unwrap();
        assert_eq!(primary.kind, Kind::SubtitleVisibility);
        assert_eq!(primary.text, "Subtitles Visible");

        let mut old = loaded();
        old.secondary_subtitle_visible = Some(false);
        let mut state = old.clone();
        state.secondary_subtitle_visible = Some(true);
        let secondary = changed(Some(&old), &state).unwrap();
        assert_eq!(secondary.kind, Kind::SecondarySubtitleVisibility);
        assert_eq!(secondary.text, "Second Subtitles Visible");
    }

    #[test]
    fn track_feedback_uses_selected_track_identity_and_empty_choice() {
        fn audio_track(id: i64, title: &str) -> riina_core::Track {
            riina_core::Track {
                id,
                kind: riina_core::TrackKind::Audio,
                title: Some(title.into()),
                lang: None,
                codec: None,
                selected: true,
                main_selection: None,
                external: false,
                albumart: false,
                external_filename: None,
                is_default: false,
                forced: false,
                channels: None,
                channel_layout: None,
                sample_rate: None,
                width: None,
                height: None,
                frame_rate: None,
            }
        }

        let mut old = loaded();
        old.tracks = vec![audio_track(1, "Main")];
        let mut state = old.clone();
        state.tracks = vec![audio_track(2, "Director Commentary")];
        let selected = changed(Some(&old), &state).unwrap();
        assert_eq!(selected.kind, Kind::Track);
        assert_eq!(selected.text, "Audio: #2 Director Commentary");

        state.tracks.clear();
        let disabled = changed(Some(&old), &state).unwrap();
        assert_eq!(disabled.kind, Kind::Track);
        assert_eq!(disabled.text, "Audio: #0 <None>");
    }

    #[test]
    fn rotation_subtitle_scale_and_loop_feedback_use_observed_values() {
        let mut old = loaded();
        old.video_rotation = Some(0);
        let mut state = old.clone();
        state.video_rotation = Some(90);
        let rotation = changed(Some(&old), &state).unwrap();
        assert_eq!(rotation.kind, Kind::Rotation);
        assert_eq!(rotation.text, "Rotate: 90°");

        old.video_rotation = state.video_rotation;
        old.subtitle_scale = Some(1.0);
        state = old.clone();
        state.subtitle_scale = Some(0.5);
        let scale = changed(Some(&old), &state).unwrap();
        assert_eq!(scale.kind, Kind::SubtitleScale);
        assert_eq!(scale.text, "Subtitle Scale: -2x");

        old.subtitle_scale = state.subtitle_scale;
        old.loop_file = Some("no".into());
        old.loop_playlist = Some("no".into());
        state = old.clone();
        state.loop_playlist = Some("inf".into());
        let looping = changed(Some(&old), &state).unwrap();
        assert_eq!(looping.kind, Kind::LoopMode);
        assert_eq!(looping.text, "Loop: Playlist");
    }
}
