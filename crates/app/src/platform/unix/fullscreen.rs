//! Read acknowledgement from the desktop managing the actual player window.
use crate::{PlayerWindow, background::CoalescedUiCallback};
use slint::{
    ComponentHandle,
    winit_030::{
        WinitWindowAccessor,
        winit::{
            dpi::PhysicalSize,
            event::WindowEvent,
            monitor::MonitorHandle,
            raw_window_handle::{HasWindowHandle, RawWindowHandle},
            window::{Fullscreen, Window as NativeWindow},
        },
    },
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};
use x11rb::{
    protocol::xproto::{Atom, AtomEnum, ClientMessageEvent, ConnectionExt, EventMask, Window},
    rust_connection::RustConnection,
};

pub(crate) fn observed_fullscreen(window: &slint::Window) -> Option<bool> {
    match surface(window)? {
        Surface::Wayland { fullscreen } => Some(fullscreen),
        Surface::X11(window) => with_x11(|client| client.is_fullscreen(window)),
    }
}

pub(crate) fn request_fullscreen(window: &slint::Window, fullscreen: bool) -> Option<()> {
    match surface(window)? {
        // A desktop-initiated X11 transition never changes Winit's requested
        // state. Address the WM directly so Escape can leave that fullscreen too.
        Surface::X11(window) => with_x11(|client| client.request(window, fullscreen)),
        Surface::Wayland { .. } => window.with_winit_window(|native| {
            native.set_fullscreen(fullscreen.then_some(Fullscreen::Borderless(None)));
        }),
    }
}

#[derive(Default)]
pub(in crate::platform) struct Observation {
    geometry: Rc<RefCell<FullscreenGeometry>>,
    repair_on_redraw: Cell<bool>,
    deliver: CoalescedUiCallback,
}

impl Observation {
    pub(in crate::platform) fn geometry_observed(
        &self,
        window: &slint::Window,
        fullscreen: bool,
        allow_repair: bool,
    ) {
        if !fullscreen {
            self.geometry.borrow_mut().retried = None;
        }
        let needs_repair = allow_repair && self.geometry.borrow().needs_repair(window);
        if !self.repair_on_redraw.replace(needs_repair) && needs_repair {
            window.request_redraw();
        }
    }

    pub(in crate::platform) fn window_event(
        &self,
        event: &WindowEvent,
        app: &slint::Weak<PlayerWindow>,
        may_repair: &(impl Fn(&PlayerWindow) -> bool + Clone + 'static),
    ) {
        if matches!(event, WindowEvent::RedrawRequested) && self.repair_on_redraw.replace(false) {
            let geometry = Rc::downgrade(&self.geometry);
            let app = app.clone();
            let may_repair = may_repair.clone();
            // Slint still has to present this frame after the Winit filter returns.
            slint::Timer::single_shot(Duration::ZERO, move || {
                if let (Some(app), Some(geometry)) = (app.upgrade(), geometry.upgrade())
                    && may_repair(&app)
                {
                    geometry.borrow_mut().repair(app.window());
                }
            });
        }
        if matches!(
            event,
            WindowEvent::Resized(_)
                | WindowEvent::ScaleFactorChanged { .. }
                | WindowEvent::Occluded(false)
                | WindowEvent::Moved(_)
                | WindowEvent::Focused(_)
        ) {
            self.deliver.post(|| {
                let app = app.clone();
                move || {
                    if let Some(app) = app.upgrade()
                        && let Some(active) = observed_fullscreen(app.window())
                    {
                        app.invoke_native_fullscreen_observed(active, true);
                    }
                }
            });
        }
    }
}

#[derive(Default)]
struct FullscreenGeometry {
    retried: Option<(MonitorHandle, PhysicalSize<u32>)>,
}

impl FullscreenGeometry {
    fn needs_repair(&self, window: &slint::Window) -> bool {
        window
            .with_winit_window(|native| self.undersized_output(native).is_some())
            .unwrap_or(false)
    }

    fn repair(&mut self, window: &slint::Window) {
        window.with_winit_window(|native| {
            let Some((monitor, bounds)) = self.undersized_output(native) else {
                return;
            };
            // A late windowed buffer can shrink a floating fullscreen container.
            // Reapply the native request once per output mode, without leaving fullscreen.
            self.retried = Some((monitor.clone(), bounds));
            native.set_fullscreen(Some(Fullscreen::Borderless(Some(monitor))));
            native.request_redraw();
        });
    }

    fn undersized_output(
        &self,
        native: &NativeWindow,
    ) -> Option<(MonitorHandle, PhysicalSize<u32>)> {
        if !matches!(
            native.window_handle().ok()?.as_raw(),
            RawWindowHandle::Wayland(_)
        ) || native.fullscreen().is_none()
        {
            return None;
        }
        let monitor = native.current_monitor()?;
        let bounds = monitor.size();
        if self
            .retried
            .as_ref()
            .is_some_and(|(display, size)| *display == monitor && *size == bounds)
            || !undersized_fullscreen(native.inner_size(), bounds)
        {
            return None;
        }
        Some((monitor, bounds))
    }
}

fn undersized_fullscreen(surface: PhysicalSize<u32>, output: PhysicalSize<u32>) -> bool {
    // Check both axes. Fractional scaling can change a pixel and rotated or
    // otherwise constrained outputs need not report matching dimensions.
    surface.width > 0
        && surface.height > 0
        && u64::from(surface.width) * 8 <= u64::from(output.width) * 7
        && u64::from(surface.height) * 8 <= u64::from(output.height) * 7
}

enum Surface {
    X11(Window),
    Wayland { fullscreen: bool },
}

fn surface(window: &slint::Window) -> Option<Surface> {
    window
        .with_winit_window(|native| match native.window_handle().ok()?.as_raw() {
            RawWindowHandle::Wayland(_) => Some(Surface::Wayland {
                fullscreen: native.fullscreen().is_some(),
            }),
            RawWindowHandle::Xlib(handle) => Some(Surface::X11(u32::try_from(handle.window).ok()?)),
            RawWindowHandle::Xcb(handle) => Some(Surface::X11(handle.window.get())),
            _ => None,
        })
        .flatten()
}

struct X11 {
    connection: RustConnection,
    state: Atom,
    fullscreen: Atom,
}

impl X11 {
    fn connect() -> Option<Self> {
        let (connection, _) = RustConnection::connect(None).ok()?;
        let state = connection.intern_atom(false, b"_NET_WM_STATE").ok()?;
        let fullscreen = connection
            .intern_atom(false, b"_NET_WM_STATE_FULLSCREEN")
            .ok()?;
        let (state, fullscreen) = (state.reply().ok()?.atom, fullscreen.reply().ok()?.atom);
        Some(Self {
            connection,
            state,
            fullscreen,
        })
    }

    fn is_fullscreen(&self, window: Window) -> Option<bool> {
        let reply = self
            .connection
            .get_property(false, window, self.state, AtomEnum::ATOM, 0, 128)
            .ok()?
            .reply()
            .ok()?;
        if reply.type_ == u32::from(AtomEnum::NONE) {
            return Some(false);
        }
        Some(reply.value32()?.any(|atom| atom == self.fullscreen))
    }

    fn request(&self, window: Window, fullscreen: bool) -> Option<()> {
        let root = self
            .connection
            .get_geometry(window)
            .ok()?
            .reply()
            .ok()?
            .root;
        let message = ClientMessageEvent::new(
            32,
            window,
            self.state,
            [u32::from(fullscreen), self.fullscreen, 0, 1, 0],
        );
        self.connection
            .send_event(
                false,
                root,
                EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                message,
            )
            .ok()?
            .check()
            .ok()
    }
}

fn with_x11<T>(request: impl FnOnce(&X11) -> Option<T>) -> Option<T> {
    thread_local! {
        static CONNECTION: RefCell<Option<X11>> = const { RefCell::new(None) };
    }
    // Winit's X11 fullscreen accessor can return its own request before the
    // window manager has seen it. EWMH stores the acknowledged state on the XID.
    CONNECTION.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = X11::connect();
        }
        let result = request(slot.as_ref()?);
        if result.is_none() {
            *slot = None;
        }
        result
    })
}

#[cfg(test)]
mod tests {
    use super::{PhysicalSize, undersized_fullscreen};

    #[test]
    fn only_a_substantially_smaller_fullscreen_surface_needs_recovery() {
        let output = PhysicalSize::new(1280, 800);
        assert!(undersized_fullscreen(PhysicalSize::new(320, 180), output));
        assert!(!undersized_fullscreen(PhysicalSize::new(1279, 799), output));
        assert!(!undersized_fullscreen(PhysicalSize::new(800, 1280), output));
        assert!(!undersized_fullscreen(PhysicalSize::new(0, 0), output));
    }
}
