//! Ask the compositor to activate an existing player after a menu action.
mod input;

use super::protocol;
use crate::PlayerWindow;
use slint::{
    ComponentHandle,
    winit_030::{
        WinitWindowAccessor,
        winit::raw_window_handle::{HasWindowHandle, RawWindowHandle},
    },
};
use std::{cell::RefCell, collections::HashMap};
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle,
    backend::ObjectId,
    delegate_noop,
    protocol::{
        wl_registry::WlRegistry,
        wl_seat::{Capability, WlSeat},
        wl_surface::WlSurface,
    },
};
use wayland_protocols::xdg::activation::v1::client::{
    xdg_activation_token_v1::{self, XdgActivationTokenV1},
    xdg_activation_v1::XdgActivationV1,
};

type Activate = Box<dyn FnOnce(&Connection, &XdgActivationV1, String)>;

struct Request {
    origin: slint::Weak<PlayerWindow>,
    target: Activate,
}

thread_local! {
    static REQUEST: RefCell<Option<Request>> = const { RefCell::new(None) };
}

pub(crate) fn request_focus<T: ComponentHandle + 'static>(
    origin: &PlayerWindow,
    target: slint::Weak<T>,
) -> bool {
    let Some(true) = origin.window().with_winit_window(|native| {
        native
            .window_handle()
            .is_ok_and(|handle| matches!(handle.as_raw(), RawWindowHandle::Wayland(_)))
    }) else {
        return false;
    };
    REQUEST.replace(Some(Request {
        origin: origin.as_weak(),
        target: Box::new(move |connection, activation, token| {
            if let Some(target) = target.upgrade()
                && target.window().is_visible()
            {
                with_surface(target.window(), connection, |surface| {
                    activation.activate(token, surface);
                });
            }
        }),
    }));
    true
}

pub(super) fn clear_request() {
    REQUEST.take();
}

pub(super) fn interface() -> &'static str {
    XdgActivationV1::interface().name
}

fn with_surface<T>(
    window: &slint::Window,
    connection: &Connection,
    run: impl FnOnce(&WlSurface) -> T,
) -> Option<T> {
    window.with_winit_window(|native| {
        let RawWindowHandle::Wayland(handle) = native.window_handle().ok()?.as_raw() else {
            return None;
        };
        // SAFETY: Winit keeps this wl_surface live for the duration of the borrow.
        // The guest connection uses the same Wayland display as the borrowed window.
        let id =
            unsafe { ObjectId::from_ptr(WlSurface::interface(), handle.surface.as_ptr().cast()) }
                .ok()?;
        let surface = WlSurface::from_id(connection, id).ok()?;
        Some(run(&surface))
    })?
}

#[derive(Default)]
pub(super) struct State {
    global: Option<(u32, XdgActivationV1)>,
    seats: HashMap<ObjectId, input::Seat>,
    sequence: u64,
    pending: Option<(u64, Activate)>,
}

impl State {
    pub(super) fn bind(
        &mut self,
        name: u32,
        registry: &WlRegistry,
        queue: &QueueHandle<protocol::State>,
    ) {
        if self.global.is_none() {
            self.global = Some((name, registry.bind(name, 1, queue, ())));
        }
    }

    pub(super) fn remove_global(&mut self, name: u32) {
        if self.global.as_ref().is_some_and(|(id, _)| *id == name)
            && let Some((_, activation)) = self.global.take()
        {
            activation.destroy();
            self.pending = None;
        }
    }

    pub(super) fn track_input(
        &mut self,
        seat: &WlSeat,
        capabilities: Capability,
        queue: &QueueHandle<protocol::State>,
    ) {
        self.seats
            .entry(seat.id())
            .or_insert_with(|| input::Seat::new(seat.clone()))
            .update(capabilities, queue);
    }

    pub(super) fn remove_seat(&mut self, id: &ObjectId) {
        self.seats.remove(id);
    }

    pub(super) fn activate_requested(
        &mut self,
        connection: &Connection,
        queue: &QueueHandle<protocol::State>,
    ) {
        let Some(request) = REQUEST.take() else {
            return;
        };
        self.pending = None;
        let (Some(origin), Some((_, activation))) = (request.origin.upgrade(), &self.global) else {
            return;
        };
        self.sequence = self.sequence.wrapping_add(1);
        let submitted = with_surface(origin.window(), connection, |source| {
            let token = activation.get_activation_token(queue, self.sequence);
            token.set_surface(source);
            let source_id = source.id().protocol_id();
            if let Some((seat, event)) = self
                .seats
                .values()
                .filter_map(|seat| seat.latest_for(source_id).map(|event| (&seat.proxy, event)))
                .max_by_key(|(_, event)| event.time)
            {
                token.set_serial(event.serial, seat);
            }
            token.commit();
        });
        if submitted.is_some() {
            self.pending = Some((self.sequence, request.target));
        }
    }

    fn complete(&mut self, id: u64, token: String, connection: &Connection) {
        if REQUEST.with(|request| request.borrow().is_some()) {
            return;
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|(expected, _)| *expected == id)
            && let Some((_, target)) = self.pending.take()
            && let Some((_, activation)) = &self.global
        {
            target(connection, activation, token);
        }
    }
}

impl Drop for State {
    fn drop(&mut self) {
        if let Some((_, activation)) = self.global.take() {
            activation.destroy();
        }
    }
}

delegate_noop!(protocol::State: ignore XdgActivationV1);

impl Dispatch<XdgActivationTokenV1, u64> for protocol::State {
    fn event(
        state: &mut Self,
        request: &XdgActivationTokenV1,
        event: xdg_activation_token_v1::Event,
        id: &u64,
        connection: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_activation_token_v1::Event::Done { token } = event {
            request.destroy();
            state.activation.complete(*id, token, connection);
        }
    }
}
