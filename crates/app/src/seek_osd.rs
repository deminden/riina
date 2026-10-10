use riina_core::{Phase, Playback, time_label};

#[derive(Default)]
pub(crate) struct SeekOsd {
    consumed: Option<(u64, u64)>,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Change {
    Hide,
    Show,
    Keep,
}

impl SeekOsd {
    #[cfg(test)]
    pub(crate) fn observe(&mut self, state: &Playback) -> Change {
        self.observe_with_suppression(state, false)
    }

    pub(crate) fn observe_with_suppression(
        &mut self,
        state: &Playback,
        suppress_message: bool,
    ) -> Change {
        let key = (state.generation, state.completed_navigation_seeks);
        if self.consumed.is_none_or(|old| old.0 != key.0)
            || matches!(state.phase, Phase::Stopped | Phase::Loading | Phase::Failed)
            || state.error.is_some()
        {
            self.consumed = Some(key);
            return Change::Hide;
        }
        // A completion and its clock can arrive in separate snapshots. Do not
        // consume the notification until a usable clock exists and seeking ends.
        // Another seek may already be in flight after an earlier completion.
        if self.consumed != Some(key)
            && !state.seeking
            && state
                .position
                .is_some_and(|value| value.is_finite() && value >= 0.0)
            && state
                .duration
                .is_some_and(|value| value.is_finite() && value > 0.0)
        {
            self.consumed = Some(key);
            if suppress_message {
                Change::Hide
            } else {
                Change::Show
            }
        } else {
            Change::Keep
        }
    }
}

pub(crate) fn clock(seconds: Option<f64>) -> String {
    let label = time_label(seconds);
    if label.len() == 4 {
        format!("0{label}")
    } else {
        label
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loaded() -> Playback {
        Playback {
            generation: 1,
            phase: Phase::Paused,
            position: Some(2.0),
            duration: Some(5.0),
            ..Default::default()
        }
    }

    #[test]
    fn coalesced_completions_wait_for_current_seek_and_clock() {
        let mut osd = SeekOsd::default();
        let mut state = loaded();
        assert_eq!(osd.observe(&state), Change::Hide);
        state.completed_navigation_seeks = 3;
        state.seeking = true;
        assert_eq!(osd.observe(&state), Change::Keep);
        state.seeking = false;
        state.position = None;
        assert_eq!(osd.observe(&state), Change::Keep);
        state.position = Some(2.0);
        assert_eq!(osd.observe(&state), Change::Show);
        state.position = Some(2.1);
        assert_eq!(osd.observe(&state), Change::Keep);
    }

    #[test]
    fn suppressed_seek_feedback_is_consumed_only_after_completion() {
        let mut osd = SeekOsd::default();
        let mut state = loaded();
        assert_eq!(osd.observe_with_suppression(&state, false), Change::Hide);

        state.completed_navigation_seeks = 1;
        state.seeking = true;
        assert_eq!(osd.observe_with_suppression(&state, true), Change::Keep);
        state.seeking = false;
        assert_eq!(osd.observe_with_suppression(&state, true), Change::Hide);
        assert_eq!(osd.observe_with_suppression(&state, false), Change::Keep);

        state.completed_navigation_seeks = 2;
        assert_eq!(osd.observe_with_suppression(&state, false), Change::Show);
    }

    #[test]
    fn decoder_refresh_completions_do_not_create_navigation_feedback() {
        let mut osd = SeekOsd::default();
        let mut state = loaded();
        osd.observe(&state);
        state.completed_seeks += 1;
        assert_eq!(osd.observe(&state), Change::Keep);
        state.completed_navigation_seeks += 1;
        assert_eq!(osd.observe(&state), Change::Show);
    }

    #[test]
    fn replacement_and_failure_discard_previous_notifications() {
        let mut osd = SeekOsd::default();
        let mut state = loaded();
        osd.observe(&state);
        state.completed_navigation_seeks = 1;
        assert_eq!(osd.observe(&state), Change::Show);
        state.begin_load();
        assert_eq!(osd.observe(&state), Change::Hide);
        state.phase = Phase::Paused;
        state.position = Some(0.0);
        state.duration = Some(5.0);
        assert_eq!(osd.observe(&state), Change::Keep);
        state.completed_navigation_seeks += 1;
        state.error = Some("seek failed".into());
        assert_eq!(osd.observe(&state), Change::Hide);
        state.error = None;
        assert_eq!(osd.observe(&state), Change::Keep);
        state.phase = Phase::Stopped;
        assert_eq!(osd.observe(&state), Change::Hide);
    }

    #[test]
    fn reference_clock_keeps_unknown_time_distinct() {
        assert_eq!(clock(Some(2.0)), "00:02");
        assert_eq!(clock(Some(5.0)), "00:05");
        assert_eq!(clock(Some(600.0)), "10:00");
        assert_eq!(clock(None), "--:--");
    }
}
