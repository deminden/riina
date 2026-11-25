//! Confirm window mode changes before applying playback or compact-window effects.
use crate::automatic_music_mode;
use crate::platform::fullscreen_window::{self, Completion, FullscreenWindow};
use crate::{NativeVideoHost, PlayerWindow};
use riina_ui::{
    PlayerControlActions, PlayerCropState, PlayerMenuActions, PlayerMusicModeState,
    PlayerPreferences,
};
use slint::ComponentHandle;
use std::{
    cell::Cell,
    rc::Rc,
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
    window: FullscreenWindow,
    music: Rc<automatic_music_mode::Controller>,
    pending: Cell<Option<Pending>>,
    desired: Cell<Option<Mode>>,
    timer: slint::Timer,
    retired: Cell<bool>,
}

pub(super) fn install(
    app: &PlayerWindow,
    host: Option<slint::Weak<NativeVideoHost>>,
    music: &Rc<automatic_music_mode::Controller>,
) -> Rc<Controller> {
    let controller = Rc::new(Controller {
        app: app.as_weak(),
        window: FullscreenWindow::new(app, host),
        music: music.clone(),
        pending: Cell::new(None),
        desired: Cell::new(None),
        timer: slint::Timer::default(),
        retired: Cell::new(false),
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
    controller
}

impl Controller {
    pub(super) fn retire(&self) {
        if self.retired.replace(true) {
            return;
        }
        self.timer.stop();
        self.pending.set(None);
        self.desired.set(None);
        self.window.retire();
    }

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
        if self.retired.get() {
            return;
        }
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
                self.music.toggle_manually(app);
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
        let awaits_confirmation = self.window.awaits_confirmation();
        if awaits_confirmation {
            self.expect(app, fullscreen);
        }
        match self.window.request(app, fullscreen) {
            Some(Completion::Immediate) => self.observe(app, fullscreen, true),
            Some(Completion::Desktop) => {}
            None if awaits_confirmation => self.abandon(app),
            None => self.desired.set(None),
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
            self.timer.start(
                slint::TimerMode::Repeated,
                fullscreen_window::CONFIRMATION_INTERVAL,
                move || {
                    if let Some(controller) = weak.upgrade() {
                        controller.tick();
                    }
                },
            );
        }
    }

    fn tick(self: &Rc<Self>) {
        let Some(app) = self.app.upgrade() else {
            self.timer.stop();
            return;
        };
        if let Some(fullscreen) = self.window.confirmed(app.window()) {
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
        if self.retired.get() {
            return;
        }
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
        self.window
            .geometry_observed(app.window(), fullscreen, self.allows_repair(app));
    }

    fn allows_repair(&self, app: &PlayerWindow) -> bool {
        !self.retired.get()
            && app.get_fullscreen()
            && self.pending.get().is_none_or(|pending| pending.fullscreen)
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

pub(super) fn probe_after_window_event(
    controller: Rc<Controller>,
) -> impl Fn(&slint::winit_030::winit::event::WindowEvent) + 'static {
    let weak = Rc::downgrade(&controller);
    let may_repair = move |app: &PlayerWindow| {
        weak.upgrade()
            .is_some_and(|controller| controller.allows_repair(app))
    };
    move |event| {
        controller
            .window
            .window_event(event, &controller.app, &may_repair);
    }
}
