//! Own the process-wide MediaPlayer endpoint and route it to one player window.
mod dispatch;

use super::{
    CommandSender, NativeVideoHost, PlaybackStore, PlayerWindow,
    ownership::{FocusOrder, PlaybackTarget},
};
use crate::{
    platform::{
        MediaEvent, SystemActivity, SystemMediaControls, active_poll::ActivePoll,
        player_input::some_player_window_is_focused, player_thumbnails::Samples,
    },
    player_ui::lifecycle::AutomaticPause,
};
use riina_core::{Phase, sync::lock};
use riina_ui::{PlayerControlActions, PlayerPreferences};
use slint::{ComponentHandle, Weak, winit_030::WinitWindowAccessor};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak as RcWeak},
    time::{Duration, Instant},
};

const MEDIA_UPDATE_INTERVAL: Duration = Duration::from_millis(50);

thread_local! {
    static APPLICATION: RefCell<RcWeak<Application>> = const { RefCell::new(RcWeak::new()) };
}

pub(in crate::platform) fn wake_application() {
    if let Ok(Some(application)) = APPLICATION.try_with(|current| current.borrow().upgrade()) {
        application.wake();
    }
}

struct Route {
    id: u64,
    player: Weak<PlayerWindow>,
    playback: PlaybackStore,
    sender: RefCell<Option<CommandSender>>,
    timeline: RefCell<Option<Samples>>,
    focused: Cell<bool>,
    automatic_pause: RefCell<AutomaticPause>,
}

impl Route {
    fn set_focus(&self, app: &PlayerWindow, focused: bool, another_player_focused: bool) {
        self.focused.set(focused);
        if !app.global::<PlayerPreferences>().get_pause_when_inactive() {
            self.automatic_pause.borrow_mut().cancel();
        } else if focused {
            let resume = self
                .automatic_pause
                .borrow_mut()
                .resume(&lock(&self.playback));
            if resume {
                app.global::<PlayerControlActions>()
                    .invoke_set_paused(false);
            }
        } else if another_player_focused {
            self.pause_for_focus(app);
        }
    }

    fn pause_for_focus(&self, app: &PlayerWindow) {
        let pause = self
            .automatic_pause
            .borrow_mut()
            .pause(&lock(&self.playback));
        if pause {
            app.global::<PlayerControlActions>().invoke_set_paused(true);
        }
    }

    fn observe_activity(&self, app: &PlayerWindow, activity: SystemActivity) {
        let preferences = app.global::<PlayerPreferences>();
        match activity {
            SystemActivity::Inactive
                if self.focused.get() && preferences.get_pause_when_inactive() =>
            {
                self.pause_for_focus(app);
            }
            SystemActivity::Sleep if preferences.get_pause_when_goes_to_sleep() => {
                let playing = {
                    let state = lock(&self.playback);
                    state.phase == Phase::Playing && !state.paused
                };
                if playing {
                    app.global::<PlayerControlActions>().invoke_set_paused(true);
                }
            }
            _ => {}
        }
    }
}

struct Window {
    route: Rc<Route>,
    app: PlayerWindow,
    focused: bool,
}

struct SkipIntervals {
    profile: String,
    expires: Instant,
    forward: Option<f64>,
    backward: Option<f64>,
}

struct Application {
    controls: RefCell<SystemMediaControls>,
    routes: RefCell<Vec<Rc<Route>>>,
    focus_order: RefCell<FocusOrder>,
    selected: Cell<Option<u64>>,
    active: Cell<bool>,
    next_id: Cell<u64>,
    intervals: RefCell<Option<SkipIntervals>>,
    poll: ActivePoll,
}

impl Application {
    fn shared() -> Rc<Self> {
        APPLICATION.with(|current| {
            if let Some(application) = current.borrow().upgrade() {
                return application;
            }
            let application = Rc::new(Self {
                controls: RefCell::new(SystemMediaControls::new(
                    crate::platform::playback_library(),
                )),
                routes: RefCell::default(),
                focus_order: RefCell::default(),
                selected: Cell::default(),
                active: Cell::new(true),
                next_id: Cell::new(1),
                intervals: RefCell::default(),
                poll: ActivePoll::default(),
            });
            *current.borrow_mut() = Rc::downgrade(&application);
            application
        })
    }

    fn wake(self: &Rc<Self>) {
        if self.routes.borrow().is_empty() {
            return;
        }
        let weak = Rc::downgrade(self);
        self.poll.start_if_idle(MEDIA_UPDATE_INTERVAL, move || {
            weak.upgrade()
                .is_some_and(|application| application.tick())
                .then_some(MEDIA_UPDATE_INTERVAL)
        });
    }

    fn tick(&self) -> bool {
        let windows = self
            .routes
            .borrow()
            .iter()
            .filter_map(|route| {
                let app = route.player.upgrade()?;
                route.sender.borrow().as_ref()?;
                let focused = app
                    .window()
                    .with_winit_window(|window| window.has_focus())
                    .unwrap_or(false);
                Some(Window {
                    route: route.clone(),
                    app,
                    focused,
                })
            })
            .collect::<Vec<_>>();
        let selected = self.focus_order.borrow_mut().select(
            windows
                .iter()
                .filter(|window| window.app.get_loaded())
                .map(|window| (window.route.id, window.focused)),
        );
        let owner = windows
            .iter()
            .find(|window| selected == Some(window.route.id));
        let published = self.publish(owner);
        let events = self.controls.borrow().drain_events().collect::<Vec<_>>();
        for event in events {
            match event {
                MediaEvent::Remote(target, action)
                    if Some(target) == published && self.selected.get() == Some(target.window) =>
                {
                    if let Some(owner) = owner {
                        owner.dispatch(target, action);
                    }
                }
                MediaEvent::Remote(..) => {}
                MediaEvent::Activity(activity) => {
                    match activity {
                        SystemActivity::Active => self.active.set(true),
                        SystemActivity::Inactive => self.active.set(false),
                        SystemActivity::Sleep => {}
                    }
                    for window in &windows {
                        if window.route.sender.borrow().is_some() {
                            window.route.observe_activity(&window.app, activity);
                        }
                    }
                }
            }
        }
        if self.active.get() {
            let any_focused = some_player_window_is_focused();
            for window in windows {
                if window.route.sender.borrow().is_some() {
                    window
                        .route
                        .set_focus(&window.app, window.focused, any_focused);
                }
            }
        }
        published.is_some()
    }

    fn publish(&self, owner: Option<&Window>) -> Option<PlaybackTarget> {
        let mut controls = self.controls.borrow_mut();
        let selected = owner.map(|window| window.route.id);
        if self.selected.replace(selected) != selected {
            let timeline = owner.and_then(|window| window.route.timeline.borrow().clone());
            controls.switch_player(timeline.as_ref().map(|samples| &samples.controller));
        }
        let Some(owner) = owner.filter(|window| {
            window
                .app
                .global::<PlayerPreferences>()
                .get_key_binding_use_system_media_controls()
        }) else {
            controls.set_remote_target(None);
            controls.synchronize(false, "", None, None);
            return None;
        };
        let preferences = owner.app.global::<PlayerPreferences>();
        let profile = preferences.get_key_binding_current_profile();
        let (forward, backward) = self.skip_intervals(profile.as_str());
        controls.synchronize(true, profile.as_str(), forward, backward);
        let playback = lock(&owner.route.playback).clone();
        let target = (!matches!(
            playback.phase,
            Phase::Stopped | Phase::Loading | Phase::Failed
        ))
        .then_some(PlaybackTarget {
            window: owner.route.id,
            generation: playback.generation,
        });
        controls.publish_playback(&playback, preferences.get_thumbnail_preview_enabled());
        controls.set_remote_target(target);
        target
    }

    fn skip_intervals(&self, profile: &str) -> (Option<f64>, Option<f64>) {
        let mut saved = self.intervals.borrow_mut();
        let now = Instant::now();
        if let Some(saved) = saved.as_ref()
            && saved.profile == profile
            && now < saved.expires
        {
            return (saved.forward, saved.backward);
        }
        let (forward, backward) =
            crate::key_binding_preferences::system_skip_intervals_for_profile(profile);
        *saved = Some(SkipIntervals {
            profile: profile.to_owned(),
            expires: now + Duration::from_secs(1),
            forward,
            backward,
        });
        (forward, backward)
    }

    fn remove(self: &Rc<Self>, route: u64) {
        self.routes
            .borrow_mut()
            .retain(|registered| registered.id != route);
        self.focus_order.borrow_mut().remove(route);
        if self.selected.get() == Some(route) {
            self.publish(None);
            if !self.routes.borrow().is_empty() {
                self.wake();
            }
        }
        if self.routes.borrow().is_empty() {
            self.poll.stop();
        }
    }
}

impl Window {
    fn dispatch(&self, target: PlaybackTarget, action: crate::platform::RemoteMediaAction) {
        let sender = self.route.sender.borrow().clone();
        if target.generation == lock(&self.route.playback).generation
            && self
                .app
                .global::<PlayerPreferences>()
                .get_key_binding_use_system_media_controls()
            && let Some(sender) = sender
        {
            dispatch::apply(&self.app, action, &self.route.playback, &sender);
        }
    }
}

pub(crate) struct SystemMediaSession {
    application: Rc<Application>,
    route: Rc<Route>,
}

impl SystemMediaSession {
    pub(crate) fn new(app: &PlayerWindow, playback: PlaybackStore) -> Self {
        let application = Application::shared();
        let id = application.next_id.get();
        application.next_id.set(id + 1);
        let route = Rc::new(Route {
            id,
            player: app.as_weak(),
            playback,
            sender: RefCell::default(),
            timeline: RefCell::default(),
            focused: Cell::default(),
            automatic_pause: RefCell::default(),
        });
        application.routes.borrow_mut().push(route.clone());
        app.on_system_media_observation_changed(wake_application);
        Self { application, route }
    }

    pub(crate) fn bind(&self, sender: &CommandSender, _: Option<Weak<NativeVideoHost>>) {
        self.route.sender.replace(Some(sender.clone()));
        self.application.wake();
    }

    pub(super) fn store_timeline_samples(&self, samples: &Samples) {
        self.route.timeline.replace(Some(samples.clone()));
    }

    pub(crate) fn prepare_to_close(&self) {
        self.route.sender.take();
        self.route.timeline.take();
        self.application.remove(self.route.id);
    }
}

impl Drop for SystemMediaSession {
    fn drop(&mut self) {
        self.prepare_to_close();
    }
}
