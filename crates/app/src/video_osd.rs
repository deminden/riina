//! Native-value feedback and reference-defined requested aspect feedback.
use riina_core::{Phase, Playback, speed_controls};

#[derive(Debug, PartialEq)]
pub(crate) enum Kind {
    Aspect,
    Deinterlace,
    Speed,
    Equalizer,
    HardwareDecoding,
}

#[derive(Debug, PartialEq)]
pub(crate) struct Message {
    pub kind: Kind,
    pub text: String,
    pub progress: Option<f32>,
}

#[derive(Default)]
pub(crate) struct VideoOsd {
    previous: Option<Playback>,
    disable_speed: bool,
}

impl VideoOsd {
    pub(crate) fn new(disable_speed: bool) -> Self {
        Self {
            disable_speed,
            ..Default::default()
        }
    }

    pub(crate) fn set_disable_speed(&mut self, disabled: bool) {
        self.disable_speed = disabled;
    }

    pub(crate) fn observe(&mut self, state: &Playback) -> Option<Message> {
        // Ignore the suppressed category before choosing a message. Filtering
        // only afterwards would swallow a hardware change coalesced with speed.
        if self.disable_speed
            && let Some(previous) = &mut self.previous
        {
            previous.speed = state.speed;
        }
        let message = changed(self.previous.as_ref(), state);
        // Catalog caches intentionally ignore clock/settings-only snapshots.
        // Consume every observation here, retaining only feedback fields so a
        // prior edit cannot repeatedly mask a later edit or restart its timer.
        self.previous = Some(Playback {
            generation: state.generation,
            speed: state.speed,
            hardware_decoding: state.hardware_decoding.clone(),
            video_equalizer: state.video_equalizer,
            video_aspect: state.video_aspect.clone(),
            aspect_requests: state.aspect_requests,
            deinterlace: state.deinterlace,
            ..Default::default()
        });
        // Consume suppressed changes too: re-enabling feedback must not replay
        // an old edit, and speed suppression must not hide unrelated channels.
        message.filter(|message| !(self.disable_speed && message.kind == Kind::Speed))
    }
}

pub(crate) fn changed(previous: Option<&Playback>, state: &Playback) -> Option<Message> {
    let previous = previous?;
    if previous.generation != state.generation
        || matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed)
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
    None
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
}
