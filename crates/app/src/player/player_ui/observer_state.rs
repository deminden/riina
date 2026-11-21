//! Snapshot projection state belongs to the UI thread for each player session.
use crate::{screenshot_osd::ScreenshotOsd, seek_osd::SeekOsd, video_osd::VideoOsd};
use riina_core::Playback;
use std::{
    cell::RefCell,
    collections::BTreeMap,
    rc::{Rc, Weak},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

thread_local! {
    static OBSERVERS: RefCell<BTreeMap<u64, Weak<RefCell<UiObserverState>>>> =
        const { RefCell::new(BTreeMap::new()) };
}

pub(super) struct UiObserverState {
    pub(super) media_geometry: crate::window_aspect::MediaGeometry,
    pub(super) displayed: Option<Arc<Playback>>,
    pub(super) seek: SeekOsd,
    pub(super) video: Rc<RefCell<VideoOsd>>,
    pub(super) screenshot: ScreenshotOsd,
}

impl UiObserverState {
    pub(super) fn new(
        video: VideoOsd,
        media_geometry: crate::window_aspect::MediaGeometry,
    ) -> Self {
        Self {
            media_geometry,
            displayed: None,
            seek: SeekOsd::default(),
            video: Rc::new(RefCell::new(video)),
            screenshot: ScreenshotOsd::default(),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct ObserverId(u64);

impl ObserverId {
    pub(super) fn lookup(self) -> Option<Rc<RefCell<UiObserverState>>> {
        OBSERVERS.with(|observers| observers.borrow().get(&self.0).and_then(Weak::upgrade))
    }
}

pub(super) struct ObserverRegistration {
    id: ObserverId,
    _state: Rc<RefCell<UiObserverState>>,
}

impl ObserverRegistration {
    pub(super) fn new(state: UiObserverState) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let state = Rc::new(RefCell::new(state));
        OBSERVERS.with(|observers| {
            observers.borrow_mut().insert(id, Rc::downgrade(&state));
        });
        Self {
            id: ObserverId(id),
            _state: state,
        }
    }

    pub(super) fn id(&self) -> ObserverId {
        self.id
    }
}

impl Drop for ObserverRegistration {
    fn drop(&mut self) {
        let _ = OBSERVERS.try_with(|observers| observers.borrow_mut().remove(&self.id.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_session_owns_its_observations_and_late_delivery_is_ignored() {
        let state = || UiObserverState::new(VideoOsd::new(false, false), Default::default());
        let first = ObserverRegistration::new(state());
        let second = ObserverRegistration::new(state());
        let first_id = first.id();
        first_id.lookup().unwrap().borrow_mut().displayed = Some(Arc::default());
        assert!(second.id().lookup().unwrap().borrow().displayed.is_none());
        drop(first);
        assert!(first_id.lookup().is_none());
        assert!(second.id().lookup().is_some());
    }
}
