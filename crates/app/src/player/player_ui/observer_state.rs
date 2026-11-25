//! Snapshot projection state belongs to the UI thread for each player session.
use super::features::PlaybackTimers;
use crate::windows::open_url::ApplicationOpenUrl;
use crate::{screenshot_osd::ScreenshotOsd, seek_osd::SeekOsd, video_osd::VideoOsd};
use riina_core::Playback;
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    collections::BTreeMap,
    rc::{Rc, Weak},
    sync::{
        Arc, Weak as SyncWeak,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

thread_local! {
    static OBSERVERS: RefCell<BTreeMap<u64, Observer>> = const { RefCell::new(BTreeMap::new()) };
}

struct Observer {
    window: slint::Weak<crate::PlayerWindow>,
    state: Weak<RefCell<UiObserverState>>,
    discovery: SyncWeak<AtomicBool>,
}

impl Observer {
    fn cancel_discovery(&self) {
        if let Some(cancelled) = self.discovery.upgrade() {
            cancelled.store(true, Ordering::Relaxed);
        }
    }
}

pub(super) struct UiObserverState {
    pub(super) automatic_music: Rc<crate::automatic_music_mode::Controller>,
    pub(super) media_geometry: crate::window_aspect::MediaGeometry,
    pub(super) displayed: Option<Arc<Playback>>,
    pub(super) volume: Weak<super::volume_preference::VolumePreference>,
    pub(super) opened_media: Option<(u64, i64)>,
    playlist_feedback_seen: Option<u64>,
    pub(super) seek: SeekOsd,
    pub(super) video: Rc<RefCell<VideoOsd>>,
    pub(super) screenshot: ScreenshotOsd,
    pub(super) timers: Option<PlaybackTimers>,
    pub(super) startup_presentation: Option<(i32, crate::startup_arguments::Presentation)>,
    pub(super) open_url: Weak<ApplicationOpenUrl>,
    pub(super) touch_bar: crate::platform::player_touch_bar::Registration,
    power: Option<crate::playback_power::Registration>,
}

impl UiObserverState {
    pub(super) fn new(
        video: VideoOsd,
        media_geometry: crate::window_aspect::MediaGeometry,
        automatic_music: Rc<crate::automatic_music_mode::Controller>,
    ) -> Self {
        Self {
            automatic_music,
            media_geometry,
            displayed: None,
            volume: Weak::new(),
            opened_media: None,
            playlist_feedback_seen: None,
            seek: SeekOsd::default(),
            video: Rc::new(RefCell::new(video)),
            screenshot: ScreenshotOsd::default(),
            timers: None,
            startup_presentation: None,
            open_url: Weak::new(),
            touch_bar: Default::default(),
            power: None,
        }
    }

    pub(super) fn show_playlist_feedback(&mut self, app: &crate::PlayerWindow, state: &Playback) {
        use crate::video_osd::{ActionFeedback, action_message};
        use riina_core::PlaylistChange;

        let Some(feedback) = state.playlist_feedback else {
            return;
        };
        if feedback.generation != state.generation
            || self
                .playlist_feedback_seen
                .is_some_and(|seen| seen >= feedback.sequence)
        {
            return;
        }
        self.playlist_feedback_seen = Some(feedback.sequence);
        let action = match feedback.change {
            PlaylistChange::Added(count) => ActionFeedback::PlaylistAdded(count.get()),
            PlaylistChange::Cleared => ActionFeedback::PlaylistCleared,
        };
        super::show_osd_message(app, action_message(action).text, true);
    }

    pub(super) fn startup_presentation(
        &mut self,
        app: &crate::PlayerWindow,
        state: &Playback,
    ) -> Option<crate::startup_arguments::Presentation> {
        use riina_core::Phase;
        let (request, presentation) = self.startup_presentation?;
        if request != app.get_media_request_generation() || state.phase == Phase::Failed {
            #[cfg(debug_assertions)]
            crate::diagnostics::log_args(
                crate::diagnostics::Level::Debug,
                "launch",
                format_args!(
                    "Discarded {presentation:?} from request {request} in {:?}",
                    state.phase
                ),
            );
            self.startup_presentation = None;
            return None;
        }
        if state.generation != 0
            && matches!(
                state.phase,
                Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Seeking | Phase::Ended
            )
        {
            self.startup_presentation = None;
            return Some(presentation);
        }
        None
    }
}

#[derive(Clone, Copy)]
pub(in crate::player) struct ObserverId(u64);

impl ObserverId {
    pub(in crate::player) fn for_window(app: &crate::PlayerWindow) -> Option<Self> {
        OBSERVERS.with(|observers| {
            observers.borrow().iter().find_map(|(&id, observer)| {
                observer
                    .window
                    .upgrade()
                    .filter(|window| std::ptr::eq(window.window(), app.window()))
                    .map(|_| Self(id))
            })
        })
    }

    pub(in crate::player) fn begin_discovery(self) -> Arc<AtomicBool> {
        let cancelled = Arc::<AtomicBool>::default();
        OBSERVERS.with(|observers| {
            if let Some(observer) = observers.borrow_mut().get_mut(&self.0) {
                observer.discovery = Arc::downgrade(&cancelled);
            }
        });
        cancelled
    }

    pub(in crate::player) fn cancel_discovery(self) {
        OBSERVERS.with(|observers| {
            if let Some(observer) = observers.borrow().get(&self.0) {
                observer.cancel_discovery();
            }
        });
    }

    pub(in crate::player) fn open_url(self) -> Option<Rc<ApplicationOpenUrl>> {
        self.lookup()?.borrow().open_url.upgrade()
    }

    pub(super) fn lookup(self) -> Option<Rc<RefCell<UiObserverState>>> {
        OBSERVERS.with(|observers| {
            observers
                .borrow()
                .get(&self.0)
                .and_then(|observer| observer.state.upgrade())
        })
    }
}

pub(super) struct ObserverRegistration {
    id: ObserverId,
    state: Rc<RefCell<UiObserverState>>,
}

impl ObserverRegistration {
    pub(super) fn new(app: &crate::PlayerWindow, state: UiObserverState) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let state = Rc::new(RefCell::new(state));
        OBSERVERS.with(|observers| {
            observers.borrow_mut().insert(
                id,
                Observer {
                    window: app.as_weak(),
                    state: Rc::downgrade(&state),
                    discovery: SyncWeak::new(),
                },
            );
        });
        Self {
            id: ObserverId(id),
            state,
        }
    }

    pub(super) fn id(&self) -> ObserverId {
        self.id
    }

    pub(super) fn bind_open_url(&self, open_url: &Rc<ApplicationOpenUrl>) {
        self.state.borrow_mut().open_url = Rc::downgrade(open_url);
    }

    pub(super) fn bind_volume(&self, volume: &Rc<super::volume_preference::VolumePreference>) {
        self.state.borrow_mut().volume = Rc::downgrade(volume);
    }

    pub(super) fn bind_playback(
        &self,
        timers: PlaybackTimers,
        power: Option<crate::playback_power::Registration>,
    ) {
        let mut state = self.state.borrow_mut();
        state.timers = Some(timers);
        state.power = power;
    }

    pub(super) fn release_power(&self) {
        self.state.borrow_mut().power.take();
    }

    pub(super) fn bind_touch_bar(
        &self,
        registration: crate::platform::player_touch_bar::Registration,
    ) {
        self.state.borrow_mut().touch_bar = registration;
    }

    pub(super) fn release_touch_bar(&self) {
        let touch_bar = std::mem::take(&mut self.state.borrow_mut().touch_bar);
        touch_bar.retire();
    }

    pub(super) fn queue_startup_presentation(
        &self,
        request: i32,
        presentation: crate::startup_arguments::Presentation,
    ) {
        self.state.borrow_mut().startup_presentation = Some((request, presentation));
    }
}

impl Drop for ObserverRegistration {
    fn drop(&mut self) {
        let _ = OBSERVERS.try_with(|observers| {
            if let Some(observer) = observers.borrow_mut().remove(&self.id.0) {
                observer.cancel_discovery();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::startup_arguments::Presentation;
    use riina_core::Phase;

    fn state(app: &crate::PlayerWindow) -> UiObserverState {
        UiObserverState::new(
            VideoOsd::new(false, false),
            Default::default(),
            crate::automatic_music_mode::Controller::install(app, true),
        )
    }

    #[test]
    fn each_session_owns_its_observations_and_late_delivery_is_ignored() {
        i_slint_backend_testing::init_no_event_loop();
        let first_app = crate::PlayerWindow::new().unwrap();
        let second_app = crate::PlayerWindow::new().unwrap();
        let first = ObserverRegistration::new(&first_app, state(&first_app));
        let second = ObserverRegistration::new(&second_app, state(&second_app));
        let first_id = ObserverId::for_window(&first_app).unwrap();
        let second_id = ObserverId::for_window(&second_app).unwrap();
        let first_scan = first_id.begin_discovery();
        let second_scan = second_id.begin_discovery();
        assert_eq!(crate::media_open::invalidate(&first_app), 1);
        assert!(first_scan.load(Ordering::Relaxed));
        assert!(!second_scan.load(Ordering::Relaxed));
        let replacement_scan = first_id.begin_discovery();
        first_id.lookup().unwrap().borrow_mut().displayed = Some(Arc::default());
        assert!(second.id().lookup().unwrap().borrow().displayed.is_none());
        drop(first);
        assert!(replacement_scan.load(Ordering::Relaxed));
        assert!(!second_scan.load(Ordering::Relaxed));
        assert!(ObserverId::for_window(&first_app).is_none());
        assert!(first_id.lookup().is_none());
        assert!(second.id().lookup().is_some());
    }

    #[test]
    fn startup_presentation_waits_for_playback_and_is_retired_by_a_new_request() {
        i_slint_backend_testing::init_no_event_loop();
        let app = crate::PlayerWindow::new().unwrap();
        let mut observer = state(&app);
        let mut playback = Playback::default();
        app.set_media_request_generation(1);
        observer.startup_presentation = Some((1, Presentation::Music));
        playback.begin_load();
        assert_eq!(observer.startup_presentation(&app, &playback), None);
        playback.phase = Phase::Paused;
        assert_eq!(
            observer.startup_presentation(&app, &playback),
            Some(Presentation::Music)
        );
        assert_eq!(observer.startup_presentation(&app, &playback), None);
        observer.startup_presentation = Some((1, Presentation::PictureInPicture));
        app.set_media_request_generation(2);
        assert_eq!(observer.startup_presentation(&app, &playback), None);
        assert_eq!(observer.startup_presentation, None);
    }
}
