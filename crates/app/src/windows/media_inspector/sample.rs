//! Invalidate outstanding reads when the inspected media or view changes.
use riina_ui::InspectorTab;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

pub(super) const INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, PartialEq)]
pub(super) struct Subject {
    pub(super) target: usize,
    pub(super) generation: u64,
    pub(super) tab: InspectorTab,
    pub(super) track: Option<usize>,
    pub(super) catalog: u64,
    pub(super) watches: u64,
    pub(super) ready: bool,
}

#[derive(Default)]
pub(super) struct Sampler {
    subject: Option<Subject>,
    revision: u64,
    last_request: Option<Instant>,
    pending: Option<Request>,
}

#[derive(Clone)]
pub(super) struct Request {
    revision: u64,
    cancelled: Arc<AtomicBool>,
}

impl Request {
    pub(super) fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

impl Sampler {
    pub(super) fn select(&mut self, subject: Option<Subject>) -> bool {
        if self.subject == subject {
            return false;
        }
        self.subject = subject;
        self.revision = self.revision.wrapping_add(1);
        self.last_request = None;
        if let Some(request) = &self.pending {
            request.cancelled.store(true, Ordering::Release);
        }
        true
    }

    pub(super) fn request(&mut self, now: Instant) -> Option<Request> {
        if !self.subject.is_some_and(|subject| subject.ready)
            || self.pending.is_some()
            || self
                .last_request
                .is_some_and(|last| now.duration_since(last) < INTERVAL)
        {
            return None;
        }
        let request = Request {
            revision: self.revision,
            cancelled: Arc::default(),
        };
        self.last_request = Some(now);
        self.pending = Some(request.clone());
        Some(request)
    }

    pub(super) fn complete(&mut self, request: &Request) -> Option<Subject> {
        if !self
            .pending
            .as_ref()
            .is_some_and(|pending| Arc::ptr_eq(&pending.cancelled, &request.cancelled))
        {
            return None;
        }
        self.pending = None;
        self.subject.filter(|_| self.revision == request.revision)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_do_not_cross_tabs_loads_or_closed_windows() {
        let mut sample = Sampler::default();
        let now = Instant::now();
        let general = Subject {
            target: 1,
            generation: 5,
            tab: InspectorTab::General,
            track: None,
            catalog: 0,
            watches: 0,
            ready: true,
        };
        assert!(sample.select(Some(general)));
        let first = sample.request(now).unwrap();
        assert!(sample.select(Some(Subject {
            tab: InspectorTab::Status,
            ..general
        })));
        assert!(sample.select(Some(general)));
        assert!(first.cancelled());
        assert!(sample.request(now).is_none());
        assert!(sample.complete(&first).is_none());
        let next = sample.request(now).unwrap();
        assert!(sample.complete(&next).is_some());
        assert!(sample.request(now).is_none());
        let later = sample.request(now + INTERVAL).unwrap();
        let replacement = Subject {
            generation: 6,
            ..general
        };
        assert!(sample.select(Some(replacement)));
        assert!(later.cancelled());
        assert!(sample.complete(&later).is_none());
        let closing = sample.request(now + INTERVAL).unwrap();
        assert!(sample.select(None));
        assert!(closing.cancelled());
        assert!(sample.complete(&closing).is_none());
        assert!(sample.request(now + INTERVAL).is_none());
    }
}
