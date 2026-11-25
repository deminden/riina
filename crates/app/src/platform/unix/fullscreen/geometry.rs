//! Recover when a Wayland compositor acknowledges fullscreen with windowed bounds.
use super::observed_fullscreen;
use crate::PlayerWindow;
use slint::{
    ComponentHandle,
    winit_030::{
        WinitWindowAccessor,
        winit::{
            dpi::PhysicalSize,
            monitor::MonitorHandle,
            raw_window_handle::{HasWindowHandle, RawWindowHandle},
            window::{Fullscreen, Window as NativeWindow},
        },
    },
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

const RETRY_SETTLE: Duration = Duration::from_millis(500);
const ACK_LIMIT: Duration = Duration::from_millis(1500);

#[derive(Default)]
pub(super) struct FullscreenGeometry {
    tried: RefCell<Option<Output>>,
    restarted: Cell<bool>,
    recovery: Cell<Option<(Phase, Instant)>>,
    timer: slint::Timer,
}

#[derive(Clone, PartialEq, Eq)]
struct Output {
    monitor: MonitorHandle,
    bounds: PhysicalSize<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    WaitingForResize,
    Leaving,
    Returning,
}

#[derive(Debug, PartialEq, Eq)]
enum Action {
    Wait,
    Leave,
    Reenter,
    Finish,
}

impl Phase {
    fn next(self, fullscreen: bool, same_small_output: bool, expired: bool) -> Action {
        match self {
            Self::WaitingForResize if !fullscreen || !same_small_output => Action::Finish,
            Self::WaitingForResize if expired => Action::Leave,
            Self::Leaving if !fullscreen => Action::Reenter,
            Self::Returning if fullscreen => Action::Finish,
            Self::Leaving | Self::Returning if expired => Action::Finish,
            _ => Action::Wait,
        }
    }
}

impl FullscreenGeometry {
    pub(super) fn confirmed(&self, window: &slint::Window) -> Option<bool> {
        // A temporary windowed configure in recovery must not commit the user's
        // playback preferences, Music Mode or a plugin window-mode event.
        if matches!(
            self.recovery.get(),
            Some((Phase::Leaving | Phase::Returning, _))
        ) {
            return None;
        }
        observed_fullscreen(window)
    }

    pub(super) fn stop(&self) {
        self.recovery.set(None);
        self.timer.stop();
    }

    pub(super) fn reset(&self) {
        self.stop();
        self.tried.take();
        self.restarted.set(false);
    }

    pub(super) fn needs_repair(&self, window: &slint::Window) -> bool {
        !self.restarted.get()
            && self.recovery.get().is_none()
            && window
                .with_winit_window(|native| self.untried_output(native).is_some())
                .unwrap_or(false)
    }

    pub(super) fn repair(
        self: &Rc<Self>,
        app: &PlayerWindow,
        may_repair: impl Fn(&PlayerWindow) -> bool + 'static,
    ) {
        if self.restarted.get() || self.recovery.get().is_some() {
            return;
        }
        let sent = app.window().with_winit_window(|native| {
            let Some(output) = self.untried_output(native) else {
                return false;
            };
            native.set_fullscreen(Some(Fullscreen::Borderless(Some(output.monitor.clone()))));
            self.tried.replace(Some(output));
            native.request_redraw();
            true
        });
        if sent != Some(true) {
            return;
        }
        self.recovery.set(Some((
            Phase::WaitingForResize,
            Instant::now() + RETRY_SETTLE,
        )));
        let geometry = Rc::downgrade(self);
        let app = app.as_weak();
        self.timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(50),
            move || {
                if let Some(geometry) = geometry.upgrade() {
                    if let Some(app) = app.upgrade()
                        && may_repair(&app)
                    {
                        geometry.tick(&app);
                    } else {
                        geometry.stop();
                    }
                }
            },
        );
    }

    fn tick(&self, app: &PlayerWindow) {
        let Some((phase, deadline)) = self.recovery.get() else {
            return;
        };
        let now = Instant::now();
        let finished = app.window().with_winit_window(|native| {
            let fullscreen = native.fullscreen().is_some();
            let same_small_output = phase == Phase::WaitingForResize
                && small_output(native).as_ref() == self.tried.borrow().as_ref();
            match phase.next(fullscreen, same_small_output, now >= deadline) {
                Action::Wait => None,
                Action::Leave => {
                    // Some desktops suppress a same-size reconfigure. Ask for a
                    // new transition only after the original retry has settled.
                    self.restarted.set(true);
                    self.recovery.set(Some((Phase::Leaving, now + ACK_LIMIT)));
                    native.set_fullscreen(None);
                    None
                }
                Action::Reenter => {
                    self.recovery.set(Some((Phase::Returning, now + ACK_LIMIT)));
                    let monitor = self.tried.borrow().as_ref().and_then(|output| {
                        native
                            .available_monitors()
                            .find(|monitor| *monitor == output.monitor)
                    });
                    native.set_fullscreen(Some(Fullscreen::Borderless(monitor)));
                    None
                }
                Action::Finish => {
                    self.stop();
                    native.request_redraw();
                    Some(fullscreen)
                }
            }
        });
        match finished {
            Some(Some(fullscreen)) => app.invoke_native_fullscreen_observed(fullscreen, true),
            None => self.stop(),
            Some(None) => {}
        }
    }

    fn untried_output(&self, native: &NativeWindow) -> Option<Output> {
        small_output(native).filter(|output| self.tried.borrow().as_ref() != Some(output))
    }
}

fn small_output(native: &NativeWindow) -> Option<Output> {
    if !matches!(
        native.window_handle().ok()?.as_raw(),
        RawWindowHandle::Wayland(_)
    ) || native.fullscreen().is_none()
    {
        return None;
    }
    let monitor = native.current_monitor()?;
    let bounds = monitor.size();
    undersized_fullscreen(native.inner_size(), bounds).then_some(Output { monitor, bounds })
}

fn undersized_fullscreen(surface: PhysicalSize<u32>, output: PhysicalSize<u32>) -> bool {
    // Fractional scaling may change a pixel. Check for a large loss in both axes.
    surface.width > 0
        && surface.height > 0
        && u64::from(surface.width) * 8 <= u64::from(output.width) * 7
        && u64::from(surface.height) * 8 <= u64::from(output.height) * 7
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_substantially_smaller_fullscreen_surface_needs_recovery() {
        let output = PhysicalSize::new(1280, 800);
        assert!(undersized_fullscreen(PhysicalSize::new(320, 180), output));
        assert!(!undersized_fullscreen(PhysicalSize::new(1279, 799), output));
        assert!(!undersized_fullscreen(PhysicalSize::new(800, 1280), output));
        assert!(!undersized_fullscreen(PhysicalSize::new(0, 0), output));
    }

    #[test]
    fn recovery_waits_for_native_confirmation_and_stops_on_timeout() {
        assert_eq!(
            Phase::WaitingForResize.next(true, true, false),
            Action::Wait
        );
        assert_eq!(
            Phase::WaitingForResize.next(true, false, true),
            Action::Finish
        );
        assert_eq!(
            Phase::WaitingForResize.next(false, false, true),
            Action::Finish
        );
        assert_eq!(
            Phase::WaitingForResize.next(true, true, true),
            Action::Leave
        );
        assert_eq!(Phase::Leaving.next(true, false, false), Action::Wait);
        assert_eq!(Phase::Leaving.next(false, false, false), Action::Reenter);
        assert_eq!(Phase::Leaving.next(true, false, true), Action::Finish);
        assert_eq!(Phase::Returning.next(false, false, false), Action::Wait);
        assert_eq!(Phase::Returning.next(true, false, false), Action::Finish);
        assert_eq!(Phase::Returning.next(false, false, true), Action::Finish);
    }
}
