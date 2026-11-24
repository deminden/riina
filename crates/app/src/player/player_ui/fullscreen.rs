//! Confirm window mode changes before applying playback or compact-window effects.
use super::music_mode;
use crate::{NativeVideoHost, PlayerWindow};
use riina_core::music_mode::State;
use riina_ui::{
    PlayerControlActions, PlayerCropState, PlayerMenuActions, PlayerMusicModeState,
    PlayerPreferences,
};
use slint::ComponentHandle;
use std::{
    cell::Cell,
    rc::Rc,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Windowed,
    Fullscreen,
    Music,
}

impl Mode {
    fn current(app: &PlayerWindow) -> Self {
        if app.get_fullscreen() {
            Self::Fullscreen
        } else if app.global::<PlayerMusicModeState>().get_music_mode() {
            Self::Music
        } else {
            Self::Windowed
        }
    }

    fn fullscreen(active: bool) -> Self {
        if active {
            Self::Fullscreen
        } else {
            Self::Windowed
        }
    }
}

#[derive(Clone, Copy)]
struct Pending {
    fullscreen: bool,
    expires: Instant,
}

pub(super) struct Controller {
    app: slint::Weak<PlayerWindow>,
    host: Option<slint::Weak<NativeVideoHost>>,
    music: Arc<Mutex<State>>,
    pending: Cell<Option<Pending>>,
    desired: Cell<Option<Mode>>,
    timer: slint::Timer,
    #[cfg(target_os = "linux")]
    geometry: std::cell::RefCell<crate::platform::FullscreenGeometry>,
    #[cfg(target_os = "linux")]
    repair_on_redraw: Cell<bool>,
    #[cfg(target_os = "macos")]
    _notifications: crate::platform::FullscreenNotifications,
}

pub(super) fn install(
    app: &PlayerWindow,
    host: Option<slint::Weak<NativeVideoHost>>,
    music: &Arc<Mutex<State>>,
) -> Rc<Controller> {
    let controller = Rc::new(Controller {
        app: app.as_weak(),
        host,
        music: music.clone(),
        pending: Cell::new(None),
        desired: Cell::new(None),
        timer: slint::Timer::default(),
        #[cfg(target_os = "linux")]
        geometry: Default::default(),
        #[cfg(target_os = "linux")]
        repair_on_redraw: Cell::new(false),
        #[cfg(target_os = "macos")]
        _notifications: crate::platform::FullscreenNotifications::new(app),
    });
    let target = controller.clone();
    let menu = app.global::<PlayerMenuActions>();
    menu.on_toggle_fullscreen(move || {
        if let Some(app) = target.app.upgrade() {
            let fullscreen = target.next_mode(&app) != Mode::Fullscreen;
            target.request_fullscreen(&app, fullscreen);
        }
    });
    let target = controller.clone();
    menu.on_request_fullscreen(move |fullscreen| {
        if let Some(app) = target.app.upgrade() {
            target.request_fullscreen(&app, fullscreen);
        }
    });
    let target = controller.clone();
    menu.on_toggle_music_mode(move || {
        if let Some(app) = target.app.upgrade() {
            let mode = if target.next_mode(&app) == Mode::Music {
                Mode::Windowed
            } else {
                Mode::Music
            };
            target.request(&app, mode);
        }
    });
    let target = controller.clone();
    app.on_native_fullscreen_observed(move |fullscreen, finished| {
        if let Some(app) = target.app.upgrade() {
            target.observe(&app, fullscreen, finished);
        }
    });
    let music = music.clone();
    app.on_auto_switch_music_mode_changed(move |enabled| {
        riina_core::sync::lock(&music).set_auto_switch(enabled);
    });
    controller
}

impl Controller {
    fn next_mode(&self, app: &PlayerWindow) -> Mode {
        self.desired
            .get()
            .or_else(|| {
                self.pending
                    .get()
                    .map(|pending| Mode::fullscreen(pending.fullscreen))
            })
            .unwrap_or_else(|| Mode::current(app))
    }

    fn request_fullscreen(self: &Rc<Self>, app: &PlayerWindow, fullscreen: bool) {
        if Mode::current(app) != Mode::Music {
            self.request(app, Mode::fullscreen(fullscreen));
        }
    }

    fn request(self: &Rc<Self>, app: &PlayerWindow, mode: Mode) {
        app.global::<PlayerControlActions>().invoke_activity();
        self.desired.set(Some(mode));
        self.drive(app);
    }

    fn drive(self: &Rc<Self>, app: &PlayerWindow) {
        if self.pending.get().is_some() {
            return;
        }
        let Some(desired) = self.desired.take() else {
            return;
        };
        match (Mode::current(app), desired) {
            (current, desired) if current == desired => {}
            (Mode::Windowed, Mode::Music) | (Mode::Music, Mode::Windowed) => {
                music_mode::toggle_manually(app, &self.music);
            }
            (Mode::Fullscreen, Mode::Music | Mode::Windowed) => {
                if desired == Mode::Music {
                    self.desired.set(Some(desired));
                }
                self.begin(app, false);
            }
            (Mode::Windowed, Mode::Fullscreen) => self.begin(app, true),
            _ => {}
        }
    }

    fn begin(self: &Rc<Self>, app: &PlayerWindow, fullscreen: bool) {
        if let Some(host) = &self.host {
            if host
                .upgrade()
                .is_some_and(|host| host.invoke_set_fullscreen_window(fullscreen))
            {
                self.observe(app, fullscreen, true);
            } else {
                self.desired.set(None);
            }
        } else if cfg!(target_os = "windows") {
            app.window().set_fullscreen(fullscreen);
            self.observe(app, fullscreen, true);
        } else {
            self.expect(app, fullscreen);
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            if crate::platform::request_fullscreen(app.window(), fullscreen).is_none() {
                self.abandon(app);
            }
        }
    }

    fn expect(self: &Rc<Self>, app: &PlayerWindow, fullscreen: bool) {
        self.pending.set(Some(Pending {
            fullscreen,
            expires: Instant::now() + Duration::from_secs(5),
        }));
        app.set_entering_fullscreen(fullscreen);
        if !self.timer.running() {
            let weak = Rc::downgrade(self);
            #[cfg(target_os = "linux")]
            let interval = Duration::from_millis(50);
            #[cfg(not(target_os = "linux"))]
            let interval = Duration::from_millis(100);
            self.timer
                .start(slint::TimerMode::Repeated, interval, move || {
                    if let Some(controller) = weak.upgrade() {
                        controller.tick();
                    }
                });
        }
    }

    fn tick(self: &Rc<Self>) {
        let Some(app) = self.app.upgrade() else {
            self.timer.stop();
            return;
        };
        #[cfg(target_os = "linux")]
        if let Some(fullscreen) = crate::platform::observed_fullscreen(app.window()) {
            self.observe(&app, fullscreen, true);
        }
        if self
            .pending
            .get()
            .is_some_and(|pending| Instant::now() >= pending.expires)
        {
            self.abandon(&app);
        }
    }

    fn observe(self: &Rc<Self>, app: &PlayerWindow, fullscreen: bool, finished: bool) {
        if !finished {
            self.expect(app, fullscreen);
            return;
        }
        if self
            .pending
            .get()
            .is_some_and(|pending| pending.fullscreen == fullscreen)
        {
            self.clear_pending(app);
        }
        if app.get_fullscreen() != fullscreen {
            publish(app, fullscreen);
        }
        self.drive(app);
        #[cfg(target_os = "linux")]
        self.watch_geometry(app, fullscreen);
    }

    #[cfg(target_os = "linux")]
    fn watch_geometry(&self, app: &PlayerWindow, fullscreen: bool) {
        if !fullscreen {
            self.geometry.borrow_mut().reset();
        }
        let needs_repair =
            self.allows_repair(app) && self.geometry.borrow().needs_repair(app.window());
        if !self.repair_on_redraw.replace(needs_repair) && needs_repair {
            app.window().request_redraw();
        }
    }

    #[cfg(target_os = "linux")]
    fn allows_repair(&self, app: &PlayerWindow) -> bool {
        app.get_fullscreen() && self.pending.get().is_none_or(|pending| pending.fullscreen)
    }

    #[cfg(target_os = "linux")]
    fn redrawing(self: &Rc<Self>) {
        if self.repair_on_redraw.replace(false) {
            // Slint draws inline after the Winit filter returns. Let it present
            // the undersized frame before asking the compositor to arrange again.
            let weak = Rc::downgrade(self);
            slint::Timer::single_shot(Duration::ZERO, move || {
                if let Some(controller) = weak.upgrade()
                    && let Some(app) = controller.app.upgrade()
                    && controller.allows_repair(&app)
                {
                    controller.geometry.borrow_mut().repair(app.window());
                }
            });
        }
    }

    fn clear_pending(&self, app: &PlayerWindow) {
        self.pending.set(None);
        self.timer.stop();
        app.set_entering_fullscreen(false);
    }

    fn abandon(&self, app: &PlayerWindow) {
        self.clear_pending(app);
        self.desired.set(None);
        app.set_status("The desktop did not complete the fullscreen change".into());
    }
}

fn publish(app: &PlayerWindow, fullscreen: bool) {
    let crop = app.global::<PlayerCropState>();
    let was_editing = crop.get_crop_editing();
    if was_editing {
        crop.invoke_finish_custom_crop(false);
    }
    app.set_fullscreen(fullscreen);
    app.window().request_redraw();
    app.invoke_window_mode_confirmed(riina_ui::PlayerWindowMode::Fullscreen, fullscreen);
    let preferences = app.global::<PlayerPreferences>();
    let playback_policy = if fullscreen {
        preferences.get_play_when_entering_fullscreen()
    } else {
        preferences.get_pause_when_leaving_fullscreen()
    };
    // Leaving the crop editor can restore the pre-edit pause state through the
    // engine queue. Apply the window policy after it even if the snapshot is paused.
    if app.get_loaded()
        && (app.get_paused() == fullscreen || (was_editing && !crop.get_crop_editing()))
        && playback_policy
    {
        app.global::<PlayerControlActions>()
            .invoke_set_paused(!fullscreen);
    }
    if fullscreen && app.get_picture_in_picture_active() {
        app.global::<PlayerMenuActions>()
            .invoke_toggle_picture_in_picture();
    }
}

#[cfg(target_os = "linux")]
pub(super) fn probe_after_window_event(
    controller: Rc<Controller>,
) -> impl Fn(&slint::winit_030::winit::event::WindowEvent) + 'static {
    use slint::winit_030::winit::event::WindowEvent;
    let deliver = crate::background::CoalescedUiCallback::default();
    move |event| {
        if matches!(event, WindowEvent::RedrawRequested) {
            controller.redrawing();
        }
        if matches!(
            event,
            WindowEvent::Resized(_)
                | WindowEvent::ScaleFactorChanged { .. }
                | WindowEvent::Occluded(false)
                | WindowEvent::Moved(_)
                | WindowEvent::Focused(_)
        ) {
            deliver.post(|| {
                let weak = controller.app.clone();
                move || {
                    if let Some(app) = weak.upgrade()
                        && let Some(active) = crate::platform::observed_fullscreen(app.window())
                    {
                        app.invoke_native_fullscreen_observed(active, true);
                    }
                }
            });
        }
    }
}
