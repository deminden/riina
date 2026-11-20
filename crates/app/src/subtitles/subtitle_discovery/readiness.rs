//! Completion belongs to a specific scan, including preference changes within a file.
use std::sync::{Mutex, MutexGuard};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Request {
    generation: u64,
    sequence: u64,
}

#[derive(Default)]
struct State {
    sequence: u64,
    active: Option<Request>,
    ready: Option<u64>,
}

#[derive(Default)]
pub(crate) struct Readiness(Mutex<State>);

impl Readiness {
    fn state(&self) -> MutexGuard<'_, State> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(crate) fn begin(&self, generation: u64) -> Request {
        let mut state = self.state();
        state.sequence = state.sequence.wrapping_add(1);
        let request = Request {
            generation,
            sequence: state.sequence,
        };
        state.active = Some(request);
        state.ready = None;
        request
    }

    pub(crate) fn is_current(&self, request: Request) -> bool {
        self.state().active == Some(request)
    }

    pub(crate) fn complete(&self, request: Request) {
        let mut state = self.state();
        if state.active == Some(request) {
            state.ready = Some(request.generation);
            state.active = None;
        }
    }

    pub(crate) fn mark_ready(&self, generation: u64) {
        let mut state = self.state();
        state.active = None;
        state.ready = Some(generation);
    }

    pub(crate) fn is_ready(&self, generation: u64) -> bool {
        self.state().ready == Some(generation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn late_completion_cannot_replace_the_newer_medias_ready_state() {
        let ready = Readiness::default();
        let old = ready.begin(1);
        let current = ready.begin(2);
        ready.complete(current);
        ready.complete(old);
        assert!(ready.is_ready(2));
        assert!(!ready.is_ready(1));
    }

    #[test]
    fn changing_preferences_invalidates_an_older_scan_in_the_same_generation() {
        let ready = Readiness::default();
        let old = ready.begin(1);
        let current = ready.begin(1);
        assert!(!ready.is_current(old));
        assert!(ready.is_current(current));
        ready.complete(old);
        assert!(!ready.is_ready(1));
        ready.complete(current);
        assert!(ready.is_ready(1));
    }

    #[test]
    fn disabling_local_discovery_invalidates_the_running_scan() {
        let ready = Readiness::default();
        let old = ready.begin(1);
        ready.mark_ready(2);
        assert!(!ready.is_current(old));
        ready.complete(old);
        assert!(ready.is_ready(2));
    }
}
