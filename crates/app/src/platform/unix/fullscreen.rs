//! Read acknowledgement from the desktop managing the actual player window.
use slint::winit_030::{
    WinitWindowAccessor,
    winit::{
        raw_window_handle::{HasWindowHandle, RawWindowHandle},
        window::Fullscreen,
    },
};
use std::cell::RefCell;
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
