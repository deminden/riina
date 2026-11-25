//! Blackout surfaces follow one confirmed, focused fullscreen player.
use crate::{NativeVideoHost, PlayerWindow};
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

#[cfg(target_os = "macos")]
use super::macos::display_blackout::Covers;
#[cfg(target_os = "linux")]
use super::unix::display_blackout::Covers;
#[cfg(target_os = "windows")]
use super::windows::display_blackout::Covers;

pub(crate) fn available() -> bool {
    #[cfg(target_os = "linux")]
    return super::wayland_drop::blackout::available();
    #[cfg(not(target_os = "linux"))]
    true
}

pub(crate) fn install(
    app: &PlayerWindow,
    host: Option<slint::Weak<NativeVideoHost>>,
) -> Rc<Session> {
    let session = Rc::new(Session {
        app: app.as_weak(),
        host,
        timer: slint::Timer::default(),
        covers: RefCell::default(),
        retired: Cell::new(false),
    });
    let target = session.clone();
    app.on_darken_secondary_displays_changed(move |active| target.sync(active));
    session
}

pub(crate) struct Session {
    app: slint::Weak<PlayerWindow>,
    host: Option<slint::Weak<NativeVideoHost>>,
    timer: slint::Timer,
    covers: RefCell<Covers>,
    retired: Cell<bool>,
}

impl Session {
    pub(crate) fn retire(&self) {
        self.retired.set(true);
        self.timer.stop();
        self.covers.borrow_mut().clear();
    }

    fn sync(self: &Rc<Self>, active: bool) {
        if self.retired.get() {
            return;
        }
        if active && available() {
            self.refresh();
            if !self.timer.running() {
                let weak = Rc::downgrade(self);
                self.timer.start(
                    slint::TimerMode::Repeated,
                    Duration::from_millis(200),
                    move || {
                        if let Some(session) = weak.upgrade() {
                            session.refresh();
                        }
                    },
                );
            }
        } else {
            self.timer.stop();
            self.covers.borrow_mut().clear();
        }
    }

    fn refresh(&self) {
        let Some(app) = self.app.upgrade() else {
            self.timer.stop();
            self.covers.borrow_mut().clear();
            return;
        };
        let enabled = app.get_fullscreen()
            && app
                .global::<riina_ui::PlayerPreferences>()
                .get_black_out_other_monitors();
        let host = self.host.as_ref().and_then(slint::Weak::upgrade);
        let owner = match (&self.host, &host) {
            (None, _) => Some(app.window()),
            (Some(_), Some(host)) => Some(host.window()),
            (Some(_), None) => None,
        };
        let mut covers = self.covers.borrow_mut();
        if let Some(owner) = owner.filter(|owner| enabled && owner.is_visible()) {
            covers.refresh(owner, app.window());
        } else {
            covers.clear();
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
mod monitor_bounds {
    use slint::winit_030::winit::{monitor::MonitorHandle, window::Window};
    use std::collections::BTreeSet;

    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub(in crate::platform) struct Bounds {
        pub(in crate::platform) x: i32,
        pub(in crate::platform) y: i32,
        pub(in crate::platform) width: u32,
        pub(in crate::platform) height: u32,
    }

    impl Bounds {
        fn new(monitor: MonitorHandle) -> Self {
            let position = monitor.position();
            let size = monitor.size();
            Self {
                x: position.x,
                y: position.y,
                width: size.width,
                height: size.height,
            }
        }
    }

    pub(in crate::platform) fn secondary(window: &Window) -> BTreeSet<Bounds> {
        let Some(owner) = window.current_monitor().map(Bounds::new) else {
            return BTreeSet::new();
        };
        window
            .available_monitors()
            .map(Bounds::new)
            .filter(|bounds| bounds != &owner && bounds.width > 0 && bounds.height > 0)
            .collect()
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(super) use monitor_bounds::{Bounds, secondary};
