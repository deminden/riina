//! Share one MPRIS endpoint across Linux player windows.
mod actions;
mod broker;
mod interface;
mod projection;

use super::{CommandSender, PlaybackStore, PlayerWindow};
use interface::Request;
use projection::Snapshot;
use riina_core::sync::lock;
use riina_ui::PlayerPreferences;
use slint::{ComponentHandle, Timer, TimerMode, Weak, winit_030::WinitWindowAccessor};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak as RcWeak},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use tokio::sync::watch;

thread_local! {
    static APPLICATION: RefCell<RcWeak<Application>> = const { RefCell::new(RcWeak::new()) };
}

struct Route {
    id: u64,
    app: Weak<PlayerWindow>,
    playback: PlaybackStore,
    sender: Option<CommandSender>,
}

struct Application {
    id: u64,
    routes: RefCell<Vec<Route>>,
    last_focused: Cell<Option<u64>>,
    snapshots: watch::Sender<Snapshot>,
    receiver: RefCell<Option<watch::Receiver<Snapshot>>>,
    timer: Timer,
}

impl Application {
    fn shared() -> Rc<Self> {
        APPLICATION.with(|existing| {
            if let Some(app) = existing.borrow().upgrade() {
                return app;
            }
            let id = next_id();
            let (snapshots, receiver) = watch::channel(Snapshot::default());
            let app = Rc::new(Self {
                id,
                routes: RefCell::default(),
                last_focused: Cell::new(None),
                snapshots,
                receiver: RefCell::new(Some(receiver)),
                timer: Timer::default(),
            });
            *existing.borrow_mut() = Rc::downgrade(&app);
            let weak = Rc::downgrade(&app);
            app.timer
                .start(TimerMode::Repeated, Duration::from_millis(250), move || {
                    if let Some(app) = weak.upgrade() {
                        app.refresh();
                    }
                });
            app
        })
    }

    fn refresh(&self) {
        let routes = self.routes.borrow();
        let selected = routes
            .iter()
            .filter_map(|route| {
                let app = route.app.upgrade()?;
                let prefs = app.global::<PlayerPreferences>();
                if route.sender.is_none()
                    || !app.get_loaded()
                    || !prefs.get_key_binding_use_system_media_controls()
                {
                    return None;
                }
                let focused = app
                    .window()
                    .with_winit_window(|window| window.has_focus())
                    .unwrap_or(false);
                let priority = if focused {
                    2
                } else {
                    u8::from(self.last_focused.get() == Some(route.id))
                };
                Some((priority, route, app))
            })
            .max_by_key(|(priority, _, _)| *priority);
        let snapshot = selected.map_or_else(Snapshot::default, |(priority, route, app)| {
            // Remember observed focus. With no keyboard seat, the newest ready
            // window remains the fallback when desktop controls open media.
            if priority == 2 {
                self.last_focused.set(Some(route.id));
            }
            Snapshot::read(route.id, &lock(&route.playback), app.get_fullscreen())
        });
        if snapshot.media.is_some() {
            self.start_broker();
        }
        self.snapshots.send_if_modified(|current| {
            if *current == snapshot {
                return false;
            }
            *current = snapshot;
            true
        });
    }

    fn start_broker(&self) {
        let Some(receiver) = self.receiver.borrow_mut().take() else {
            return;
        };
        let id = self.id;
        if let Err(error) = broker::start(id, receiver, move |request| {
            let _ = slint::invoke_from_event_loop(move || {
                APPLICATION.with(|current| {
                    let owner = current.borrow().upgrade().filter(|owner| owner.id == id);
                    if let Some(owner) = owner {
                        owner.deliver(request);
                    }
                });
            });
        }) {
            crate::diagnostics::warn!("Could not start Linux system media controls: {error}");
        }
    }

    fn deliver(&self, request: Request) {
        let accepted = self.snapshots.borrow().media.as_ref().is_some_and(|media| {
            media.route == request.route && media.generation == request.generation
        });
        if !accepted {
            return;
        }
        let recipient = self.routes.borrow().iter().find_map(|route| {
            if route.id != request.route {
                return None;
            }
            let app = route.app.upgrade()?;
            if !app
                .global::<PlayerPreferences>()
                .get_key_binding_use_system_media_controls()
                || lock(&route.playback).generation != request.generation
            {
                return None;
            }
            Some((app, route.playback.clone(), route.sender.clone()?))
        });
        if let Some((app, latest, sender)) = recipient {
            actions::apply(&app, &latest, &sender, request.operation);
        }
    }
}

fn next_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

pub(crate) struct SystemMediaSession {
    application: Rc<Application>,
    route: u64,
}

impl SystemMediaSession {
    pub(crate) fn new(app: &PlayerWindow, playback: PlaybackStore) -> Self {
        let application = Application::shared();
        let route = next_id();
        application.routes.borrow_mut().push(Route {
            id: route,
            app: app.as_weak(),
            playback,
            sender: None,
        });
        Self { application, route }
    }

    pub(crate) fn bind(&self, sender: &CommandSender) {
        if let Some(route) = self
            .application
            .routes
            .borrow_mut()
            .iter_mut()
            .find(|route| route.id == self.route)
        {
            route.sender = Some(sender.clone());
        }
        self.application.refresh();
    }

    pub(crate) fn prepare_to_close(&self) {
        self.application
            .routes
            .borrow_mut()
            .retain(|route| route.id != self.route);
        self.application.refresh();
    }
}

impl Drop for SystemMediaSession {
    fn drop(&mut self) {
        self.prepare_to_close();
    }
}
