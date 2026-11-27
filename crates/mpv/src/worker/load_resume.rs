//! Match a temporary manual-open pause to the new file's initial playback restart.
use riina_core::LoadResume;

#[derive(Default)]
pub(super) struct Resume {
    pending: Option<(LoadResume, Stage)>,
}

#[derive(PartialEq, Eq)]
enum Stage {
    Submitted(u64),
    AwaitingStart(Option<i64>),
    AwaitingFallback,
    Loading(u64),
    Loaded(u64),
}

impl Resume {
    pub(super) fn submit(&mut self, id: u64, policy: LoadResume) {
        self.pending = Some((policy, Stage::Submitted(id)));
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }

    pub(super) fn awaits_reply(&self, id: u64) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|(_, stage)| *stage == Stage::Submitted(id))
    }

    pub(super) fn replied(&mut self, result: Result<Option<i64>, ()>) {
        if let Some((policy, _)) = self.pending.take()
            && let Ok(entry) = result
        {
            // A loadfile reply identifies its entry. Playlist navigation may
            // return no ID and instead owns the start directly after its reply.
            self.pending = Some((policy, Stage::AwaitingStart(entry)));
        }
    }

    pub(super) fn file_started(&mut self, generation: u64, entry: Option<i64>) {
        match self.pending {
            Some((policy, Stage::AwaitingStart(expected)))
                if expected.is_none() || expected == entry =>
            {
                self.pending = Some((policy, Stage::Loading(generation)));
            }
            Some((policy, Stage::AwaitingFallback)) => {
                self.pending = Some((policy, Stage::Loading(generation)));
            }
            Some((_, Stage::Loading(_) | Stage::Loaded(_))) => self.cancel(),
            _ => {}
        }
    }

    pub(super) fn file_loaded(&mut self, generation: u64) {
        if let Some((policy, Stage::Loading(loading))) = self.pending
            && loading == generation
        {
            self.pending = Some((policy, Stage::Loaded(generation)));
        }
    }

    pub(super) fn file_ended(&mut self, generation: u64, continues: bool) {
        if let Some((policy, Stage::Loading(current) | Stage::Loaded(current))) = self.pending
            && current == generation
        {
            self.pending = continues.then_some((policy, Stage::AwaitingFallback));
        }
    }

    pub(super) fn became_idle(&mut self) {
        if matches!(self.pending, Some((_, Stage::AwaitingFallback))) {
            self.cancel();
        }
    }

    pub(super) fn playback_restarted(&mut self, generation: u64, pause_when_open: bool) -> bool {
        if let Some((policy, Stage::Loaded(loaded))) = self.pending
            && loaded == generation
        {
            self.cancel();
            return policy == LoadResume::Play || !pause_when_open;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resume_waits_for_the_selected_entry_and_its_own_restart() {
        let mut resume = Resume::default();
        resume.submit(11, LoadResume::FollowPreference);
        resume.submit(12, LoadResume::FollowPreference);
        assert!(!resume.awaits_reply(11));
        assert!(resume.awaits_reply(12));
        resume.replied(Ok(Some(42)));

        resume.file_started(9, Some(41));
        resume.file_loaded(9);
        assert!(!resume.playback_restarted(9, false));
        resume.file_ended(9, false);
        resume.file_started(10, Some(42));
        assert!(!resume.playback_restarted(10, false));
        resume.file_loaded(10);
        assert!(!resume.playback_restarted(9, false));
        assert!(resume.playback_restarted(10, false));
        assert!(!resume.playback_restarted(10, false));
    }

    #[test]
    fn redirect_keeps_the_override_and_a_failed_or_cancelled_load_retires_it() {
        let mut resume = Resume::default();
        resume.submit(20, LoadResume::Play);
        resume.replied(Ok(Some(51)));
        resume.file_started(9, Some(51));
        resume.file_ended(9, true);
        resume.file_started(10, Some(52));
        resume.file_loaded(10);
        assert!(resume.playback_restarted(10, true));

        resume.submit(21, LoadResume::FollowPreference);
        resume.replied(Ok(None));
        resume.file_started(11, None);
        resume.file_loaded(11);
        assert!(!resume.playback_restarted(11, true));
        assert!(!resume.playback_restarted(11, false));

        resume.submit(22, LoadResume::Play);
        resume.replied(Err(()));
        resume.file_started(12, None);
        resume.file_loaded(12);
        assert!(!resume.playback_restarted(12, false));

        resume.submit(23, LoadResume::Play);
        resume.replied(Ok(None));
        resume.file_started(13, None);
        resume.cancel();
        resume.file_loaded(13);
        assert!(!resume.playback_restarted(13, false));

        resume.submit(24, LoadResume::Play);
        resume.replied(Ok(None));
        resume.file_started(14, None);
        resume.file_ended(14, true);
        resume.became_idle();
        resume.file_started(15, None);
        resume.file_loaded(15);
        assert!(!resume.playback_restarted(15, false));
    }
}
