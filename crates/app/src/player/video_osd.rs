//! Native-value feedback and reference-defined requested aspect feedback.
mod observations;

use observations::changed_all;
use riina_core::{Phase, Playback};
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
        if disabled {
            self.pending.retain(|message| message.kind != Kind::Speed);
        }
    }

    pub(crate) fn set_disable_pause_resume(&mut self, disabled: bool) {
        self.disable_pause_resume = disabled;
        if disabled {
            self.pending
                .retain(|message| message.kind != Kind::PauseResume);
        }
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
fn changed(previous: Option<&Playback>, state: &Playback) -> Option<Message> {
    observations::changed_one(previous, state)
}

#[cfg(test)]
mod tests;
