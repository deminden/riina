//! Transient feedback derives from confirmed native values, never queued intent.
use riina_core::{Phase, Playback, speed_controls};

#[derive(Debug, PartialEq)]
pub(crate) struct Message {
    pub text: String,
    pub progress: Option<f32>,
}

#[derive(Default)]
pub(crate) struct VideoOsd {
    previous: Option<Playback>,
}

impl VideoOsd {
    pub(crate) fn observe(&mut self, state: &Playback) -> Option<Message> {
        let message = changed(self.previous.as_ref(), state);
        // Catalog caches intentionally ignore clock/settings-only snapshots.
        // Consume every observation here, retaining only feedback fields so a
        // prior edit cannot repeatedly mask a later edit or restart its timer.
        self.previous = Some(Playback {
            generation: state.generation,
            speed: state.speed,
            hardware_decoding: state.hardware_decoding.clone(),
            video_equalizer: state.video_equalizer,
            ..Default::default()
        });
        message
    }
}

pub(crate) fn changed(previous: Option<&Playback>, state: &Playback) -> Option<Message> {
    let previous = previous?;
    if previous.generation != state.generation
        || matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed)
        || state.error.is_some()
    {
        return None;
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
                text: format!("{label}: {value}"),
                progress: Some((value as f32 + 100.0) / 200.0),
            });
        }
    }
    if state.speed != previous.speed {
        return Some(Message {
            text: format!("Speed: {}x", speed_controls::text(state.speed)),
            progress: None,
        });
    }
    if let (Some(old), Some(mode)) = (&previous.hardware_decoding, &state.hardware_decoding)
        && old != mode
    {
        return Some(Message {
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
}
