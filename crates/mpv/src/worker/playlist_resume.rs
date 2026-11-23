//! Ownership of the temporary pause applied for manual playlist navigation.

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) enum PlaylistResume {
    #[default]
    Idle,
    Submitted(u64),
    AwaitingStart,
    Loading(u64),
    Loaded(u64),
}

impl PlaylistResume {
    pub(super) fn command_replied(&mut self, id: u64, failed: bool) {
        if *self == Self::Submitted(id) {
            // mpv replies to synchronous playlist navigation before starting
            // the selected entry. Old file and seek events can arrive first.
            *self = if failed {
                Self::Idle
            } else {
                Self::AwaitingStart
            };
        }
    }

    pub(super) fn file_started(&mut self, generation: u64) {
        match self {
            Self::AwaitingStart => *self = Self::Loading(generation),
            Self::Loading(_) | Self::Loaded(_) => *self = Self::Idle,
            _ => {}
        }
    }

    pub(super) fn file_loaded(&mut self, generation: u64) {
        if *self == Self::Loading(generation) {
            *self = Self::Loaded(generation);
        }
    }

    pub(super) fn file_ended(&mut self, generation: u64, redirects: bool) {
        if *self == Self::Loading(generation) || *self == Self::Loaded(generation) {
            *self = if redirects {
                Self::AwaitingStart
            } else {
                Self::Idle
            };
        }
    }

    pub(super) fn playback_restarted(&mut self, generation: u64) -> bool {
        if *self != Self::Loaded(generation) {
            return false;
        }
        *self = Self::Idle;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resume_waits_for_the_latest_navigation_and_its_own_restart() {
        let mut resume = PlaylistResume::Submitted(12);
        resume.command_replied(11, false);
        resume.file_started(9);
        resume.file_loaded(9);
        assert!(!resume.playback_restarted(9));

        resume.command_replied(12, false);
        resume.file_ended(9, false);
        resume.file_started(10);
        assert!(!resume.playback_restarted(10));
        resume.file_loaded(10);
        assert!(!resume.playback_restarted(9));
        assert!(resume.playback_restarted(10));
        assert!(!resume.playback_restarted(10));
    }

    #[test]
    fn a_redirect_keeps_the_intent_and_a_failed_replacement_retires_it() {
        let mut resume = PlaylistResume::Submitted(20);
        resume.command_replied(20, false);
        resume.file_started(9);
        resume.file_ended(9, true);
        resume.file_started(10);
        resume.file_loaded(10);
        assert!(resume.playback_restarted(10));

        resume = PlaylistResume::Submitted(21);
        resume.command_replied(21, false);
        resume.file_started(11);
        resume.file_ended(11, false);
        resume.file_started(12);
        resume.file_loaded(12);
        assert!(!resume.playback_restarted(12));

        resume = PlaylistResume::Submitted(22);
        resume.command_replied(22, true);
        resume.file_started(13);
        resume.file_loaded(13);
        assert!(!resume.playback_restarted(13));
    }
}
