//! Route secondary-display covers to the desktop that owns the video window.
mod x11;

use crate::platform::{display_blackout::secondary, wayland_drop::blackout};
use slint::winit_030::{
    WinitWindowAccessor,
    winit::{
        platform::wayland::MonitorHandleExtWayland,
        raw_window_handle::{HasWindowHandle, RawWindowHandle},
    },
};

#[derive(Default)]
pub(in crate::platform) struct Covers {
    x11: x11::Covers,
    wayland: blackout::Request,
}

impl Covers {
    pub(in crate::platform) fn refresh(&mut self, owner: &slint::Window, _: &slint::Window) {
        let updated = owner.with_winit_window(|window| {
            if !window.has_focus() || window.is_minimized() == Some(true) {
                self.clear();
                return;
            }
            match window.window_handle().ok().map(|handle| handle.as_raw()) {
                Some(RawWindowHandle::Wayland(_)) => {
                    let display = window
                        .current_monitor()
                        .filter(|_| window.available_monitors().count() > 1)
                        .map(|monitor| monitor.native_id());
                    self.wayland.set(display);
                }
                Some(RawWindowHandle::Xlib(_) | RawWindowHandle::Xcb(_)) => {
                    self.x11.refresh(secondary(window));
                }
                _ => self.clear(),
            }
        });
        if updated.is_none() {
            self.clear();
        }
    }

    pub(in crate::platform) fn clear(&mut self) {
        self.wayland.set(None);
        self.x11.clear();
    }
}
