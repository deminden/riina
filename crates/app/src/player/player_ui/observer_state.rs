//! Snapshot projection state belongs to the UI thread for each player session.
use super::{features::PlaybackTimers, idle_media::Availability};
use crate::player::playlist_edits;
use crate::windows::open_url::ApplicationOpenUrl;
use crate::{seek_osd::SeekOsd, video_osd::VideoOsd};
use riina_core::Playback;
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
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
    playlist_edits: Rc<RefCell<playlist_edits::Queue>>,
    idle_media: Option<Rc<Cell<Availability>>>,
    parked: Rc<Cell<bool>>,
    stopped: Option<Rc<dyn Fn(u32)>>,
    delivered_stop: Option<u32>,
    end: Option<EndOfFile>,
}

struct EndOfFile {
    latest: crate::PlaybackStore,
    commands: riina_mpv::CommandSender,
    superseded_through: u64,
    submitted: Option<(u64, u32)>,
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

    pub(in crate::player) fn supersede_media(self) {
        OBSERVERS.with(|observers| {
            let mut observers = observers.borrow_mut();
            let Some(observer) = observers.get_mut(&self.0) else {
                return;
            };
            if let Some(available) = &observer.idle_media {
                available.set(Availability::Unavailable);
            }
            if let Some(end) = observer.end.as_mut() {
                end.superseded_through = riina_core::sync::lock(&end.latest).generation;
            }
        });
    }

    pub(in crate::player) fn arm_idle_after_stop(self, request: u32) {
        if let Some(available) = self.idle_media() {
            available.set(Availability::AfterStop(request));
        }
    }

    pub(super) fn bind_end(
        self,
        app: &crate::PlayerWindow,
        latest: crate::PlaybackStore,
        commands: riina_mpv::CommandSender,
    ) {
        OBSERVERS.with(|observers| {
            if let Some(observer) = observers.borrow_mut().get_mut(&self.0) {
                observer.end = Some(EndOfFile {
                    latest,
                    commands,
                    superseded_through: 0,
                    submitted: None,
                });
            }
        });
        let weak = app.as_weak();
        app.global::<riina_ui::PlayerDialogState>()
            .on_dialog_closed(move || {
                if let Some(app) = weak.upgrade() {
                    self.dialog_closed(&app);
                }
            });
    }

    pub(super) fn eof_stop_superseded(self, generation: u64, request: u32) -> bool {
        OBSERVERS.with(|observers| {
            observers
                .borrow()
                .get(&self.0)
                .and_then(|observer| observer.end.as_ref())
                .is_some_and(|end| {
                    end.submitted == Some((generation, request))
                        && generation <= end.superseded_through
                })
        })
    }

    pub(super) fn finish_at_end(self, app: &crate::PlayerWindow, playback: &Playback) {
        if !playback.ended_idle()
            || !app.get_player_manager_ready()
            || app
                .global::<riina_ui::PlayerDialogState>()
                .get_dialog_open()
        {
            return;
        }
        let request = app.get_media_request_generation().cast_unsigned();
        let result = OBSERVERS.with(|observers| {
            let mut observers = observers.borrow_mut();
            let observer = observers.get_mut(&self.0)?;
            let end = observer.end.as_mut()?;
            if playback.generation <= end.superseded_through
                || end.submitted == Some((playback.generation, request))
                || observer.playlist_edits.borrow().has_pending_media()
            {
                return None;
            }
            let result = end.commands.stop_at_end(playback.generation, request);
            if result.is_ok() {
                end.submitted = Some((playback.generation, request));
                if let Some(available) = &observer.idle_media {
                    available.set(Availability::AfterStop(request));
                }
            }
            Some(result)
        });
        if let Some(Err(error)) = result {
            crate::diagnostics::report(app, error);
        }
    }

    pub(super) fn idle_media(self) -> Option<Rc<Cell<Availability>>> {
        OBSERVERS.with(|observers| observers.borrow().get(&self.0)?.idle_media.clone())
    }

    pub(in crate::player) fn is_parked(self) -> bool {
        OBSERVERS.with(|observers| {
            observers
                .borrow()
                .get(&self.0)
                .is_some_and(|observer| observer.parked.get())
        })
    }

    pub(super) fn notify_stopped(self, app: &crate::PlayerWindow, playback: &Playback) {
        let request = app.get_media_request_generation().cast_unsigned();
        if !playback.confirmed_stop(request)
            || !app.get_player_manager_ready()
            || app
                .global::<riina_ui::PlayerDialogState>()
                .get_dialog_open()
        {
            return;
        }
        let callback = OBSERVERS.with(|observers| {
            let mut observers = observers.borrow_mut();
            let observer = observers.get_mut(&self.0)?;
            let callback = observer.stopped.clone()?;
            (observer.delivered_stop.replace(request) != Some(request)).then_some(callback)
        });
        if let Some(callback) = callback {
            callback(request);
        }
    }

    fn dialog_closed(self, app: &crate::PlayerWindow) {
        let latest = OBSERVERS.with(|observers| {
            let mut observers = observers.borrow_mut();
            let observer = observers.get_mut(&self.0)?;
            let end = observer.end.as_ref()?;
            // A newly opened dialog can preempt the manager's deferred close.
            observer.delivered_stop = None;
            Some(riina_core::sync::lock(&end.latest).clone())
        });
        if let Some(playback) = latest {
            self.finish_at_end(app, &playback);
            self.notify_stopped(app, &playback);
        }
    }

    pub(super) fn enable_idle_media(self, initial: Availability) {
        OBSERVERS.with(|observers| {
            if let Some(observer) = observers.borrow_mut().get_mut(&self.0) {
                observer.idle_media = Some(Rc::new(Cell::new(initial)));
            }
        });
    }

    pub(in crate::player) fn open_url(self) -> Option<Rc<ApplicationOpenUrl>> {
        self.lookup()?.borrow().open_url.upgrade()
    }

    pub(in crate::player) fn playlist_edits(self) -> Option<Rc<RefCell<playlist_edits::Queue>>> {
        OBSERVERS.with(|observers| {
            observers
                .borrow()
                .get(&self.0)
                .map(|observer| observer.playlist_edits.clone())
        })
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
    pub(super) parked: Rc<Cell<bool>>,
}

impl ObserverRegistration {
    pub(super) fn new(app: &crate::PlayerWindow, state: UiObserverState) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let state = Rc::new(RefCell::new(state));
        let parked = Rc::new(Cell::new(false));
        OBSERVERS.with(|observers| {
            observers.borrow_mut().insert(
                id,
                Observer {
                    window: app.as_weak(),
                    state: Rc::downgrade(&state),
                    discovery: SyncWeak::new(),
                    playlist_edits: Rc::default(),
                    idle_media: None,
                    parked: parked.clone(),
                    stopped: None,
                    delivered_stop: None,
                    end: None,
                },
            );
        });
        Self {
            id: ObserverId(id),
            state,
            parked,
        }
    }

    pub(super) fn id(&self) -> ObserverId {
        self.id
    }

    pub(super) fn bind_stop(&self, callback: impl Fn(u32) + 'static) {
        OBSERVERS.with(|observers| {
            if let Some(observer) = observers.borrow_mut().get_mut(&self.id.0) {
                observer.stopped = Some(Rc::new(callback));
            }
        });
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
    fn idle_eof_is_finalized_once_and_cannot_retire_a_new_media_request() {
        i_slint_backend_testing::init_no_event_loop();
        let app = crate::PlayerWindow::new().unwrap();
        app.set_player_manager_ready(true);
        let observer = ObserverRegistration::new(&app, state(&app));
        let id = observer.id();
        let mut ended = Playback {
            generation: 1,
            phase: Phase::Ended,
            ..Playback::default()
        };
        let latest = crate::PlaybackStore::default();
        let (commands, engine) = riina_mpv::CommandSender::recording(4);
        id.bind_end(&app, latest.clone(), commands);
        id.finish_at_end(&app, &ended);
        assert!(engine.commands.try_recv().is_err());

        ended.idle_active = true;
        *riina_core::sync::lock(&latest) = Arc::new(ended.clone());
        assert_eq!(crate::media_open::invalidate(&app), 1);
        id.finish_at_end(&app, &ended);
        assert!(engine.commands.try_recv().is_err());

        ended.generation = 2;
        *riina_core::sync::lock(&latest) = Arc::new(ended.clone());
        let dialog = app.global::<riina_ui::PlayerDialogState>();
        dialog.set_dialog_open(true);
        id.finish_at_end(&app, &ended);
        assert!(engine.commands.try_recv().is_err());

        dialog.set_dialog_open(false);
        dialog.invoke_dialog_closed();
        assert_eq!(
            engine.commands.try_recv(),
            Ok(riina_core::Command::Stop(Some(1)))
        );
        id.finish_at_end(&app, &ended);
        assert!(engine.commands.try_recv().is_err());

        let stopped = Rc::new(Cell::new(None));
        let observed = stopped.clone();
        observer.bind_stop(move |request| observed.set(Some(request)));
        ended.phase = Phase::Stopped;
        ended.stopped_by = Some(1);
        *riina_core::sync::lock(&latest) = Arc::new(ended.clone());
        dialog.set_dialog_open(true);
        id.notify_stopped(&app, &ended);
        assert_eq!(stopped.get(), None);
        dialog.set_dialog_open(false);
        dialog.invoke_dialog_closed();
        assert_eq!(stopped.get(), Some(1));

        id.supersede_media();
        assert!(id.eof_stop_superseded(2, 1));
        assert!(!id.eof_stop_superseded(2, 2));
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
