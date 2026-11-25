//! Read acknowledgement from the desktop managing the actual player window.
mod geometry;

use crate::PlayerWindow;
use geometry::FullscreenGeometry;
use slint::{
    ComponentHandle,
    winit_030::{
        WinitWindowAccessor,
        winit::{
            event::WindowEvent,
            raw_window_handle::{HasWindowHandle, RawWindowHandle},
            window::Fullscreen,
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

fn request_fullscreen(window: &slint::Window, fullscreen: bool) -> Option<()> {
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
    geometry: Rc<FullscreenGeometry>,
    repair_on_redraw: Cell<bool>,
    confirm: slint::Timer,
}

impl Observation {
    pub(in crate::platform) fn request(&self, window: &slint::Window, active: bool) -> bool {
        self.geometry.stop();
        self.repair_on_redraw.set(false);
        request_fullscreen(window, active).is_some()
    }

    pub(in crate::platform) fn confirmed(&self, window: &slint::Window) -> Option<bool> {
        self.geometry.confirmed(window)
    }

    pub(in crate::platform) fn geometry_observed(
        &self,
        window: &slint::Window,
        fullscreen: bool,
        allow_repair: bool,
    ) {
        if !fullscreen {
            self.geometry.reset();
        }
        let needs_repair = allow_repair && self.geometry.needs_repair(window);
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
                    geometry.repair(&app, may_repair);
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
        ) && !self.confirm.running()
        {
            let app = app.clone();
            let geometry = Rc::downgrade(&self.geometry);
            self.confirm
                .start(slint::TimerMode::SingleShot, Duration::ZERO, move || {
                    if let Some(app) = app.upgrade()
                        && let Some(geometry) = geometry.upgrade()
                        && let Some(active) = geometry.confirmed(app.window())
                    {
                        app.invoke_native_fullscreen_observed(active, true);
                    }
                });
        }
    }
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
