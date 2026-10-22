//! Turn confirmed native changes into ordered feedback.
use super::{Kind, Message};
use riina_core::{Phase, Playback, speed_controls};

pub(super) fn changed_all(previous: Option<&Playback>, state: &Playback) -> Vec<Message> {
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

pub(super) fn changed_one(previous: Option<&Playback>, state: &Playback) -> Option<Message> {
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
