//! Opening feedback survives coalescing without overriding a later manual exit.
use riina_core::{Phase, Playback};

#[derive(Default)]
pub(crate) struct OpeningFullscreen {
    ready_generation: Option<u64>,
}
impl OpeningFullscreen {
    pub(crate) fn observe(
        &mut self,
        state: &Playback,
        enabled: bool,
        fullscreen: bool,
        mini_player: bool,
    ) -> bool {
        if !matches!(
            state.phase,
            Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Seeking | Phase::Ended
        ) || self.ready_generation == Some(state.generation)
        {
            return false;
        }
        // Consume opening even when disabled or already fullscreen. A preference
        // edit or ordinary playback update must not become another opening.
        self.ready_generation = Some(state.generation);
        enabled && !fullscreen && !mini_player
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coalesced_opening_enters_once_and_preserves_manual_exit() {
        let mut policy = OpeningFullscreen::default();
        let mut state = Playback::default();
        state.begin_load();
        assert!(!policy.observe(&state, true, false, false));
        state.phase = Phase::Paused;
        assert!(policy.observe(&state, true, false, false));
        assert!(!policy.observe(&state, true, true, false));
        assert!(!policy.observe(&state, true, false, false));
        state.begin_load();
        // The UI may never receive the intermediate Loading snapshot.
        state.phase = Phase::Playing;
        assert!(policy.observe(&state, true, false, false));
    }

    #[test]
    fn disabled_existing_fullscreen_and_music_mode_do_not_replay_opening() {
        for (enabled, fullscreen, mini_player) in [
            (false, false, false),
            (true, true, false),
            (true, false, true),
        ] {
            let mut policy = OpeningFullscreen::default();
            let mut state = Playback::default();
            state.begin_load();
            state.phase = Phase::Failed;
            assert!(!policy.observe(&state, enabled, fullscreen, mini_player));
            state.begin_load();
            state.phase = Phase::Playing;
            assert!(!policy.observe(&state, enabled, fullscreen, mini_player));
            assert!(!policy.observe(&state, true, false, false));
        }
    }
}
