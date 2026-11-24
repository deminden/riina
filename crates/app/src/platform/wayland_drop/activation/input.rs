//! Keep the latest compositor input serial for each seat and focused surface.
use super::protocol;
use std::time::Instant;
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle,
    backend::ObjectId,
    protocol::{
        wl_keyboard::{self, WlKeyboard},
        wl_pointer::{self, WlPointer},
        wl_seat::{Capability, WlSeat},
    },
};

#[derive(Clone, Copy)]
pub(super) struct Serial {
    pub(super) serial: u32,
    pub(super) time: Instant,
}

struct Input<P> {
    proxy: P,
    surface: Option<u32>,
    last: Option<Serial>,
}

impl<P> Input<P> {
    fn new(proxy: P) -> Self {
        Self {
            proxy,
            surface: None,
            last: None,
        }
    }

    fn enter(&mut self, surface: u32, serial: u32) {
        self.surface = Some(surface);
        self.record(serial);
    }

    fn leave(&mut self) {
        self.surface = None;
        self.last = None;
    }

    fn record(&mut self, serial: u32) {
        self.last = Some(Serial {
            serial,
            time: Instant::now(),
        });
    }

    fn latest_for(&self, surface: u32) -> Option<Serial> {
        self.last.filter(|_| self.surface == Some(surface))
    }
}

pub(super) struct Seat {
    pub(super) proxy: WlSeat,
    pointer: Option<Input<WlPointer>>,
    keyboard: Option<Input<WlKeyboard>>,
}

impl Seat {
    pub(super) fn new(proxy: WlSeat) -> Self {
        Self {
            proxy,
            pointer: None,
            keyboard: None,
        }
    }

    pub(super) fn update(
        &mut self,
        capabilities: Capability,
        queue: &QueueHandle<protocol::State>,
    ) {
        if capabilities.contains(Capability::Pointer) {
            self.pointer
                .get_or_insert_with(|| Input::new(self.proxy.get_pointer(queue, self.proxy.id())));
        } else {
            self.release_pointer();
        }
        if capabilities.contains(Capability::Keyboard) {
            self.keyboard
                .get_or_insert_with(|| Input::new(self.proxy.get_keyboard(queue, self.proxy.id())));
        } else {
            self.release_keyboard();
        }
    }

    pub(super) fn latest_for(&self, surface: u32) -> Option<Serial> {
        [
            self.pointer
                .as_ref()
                .and_then(|input| input.latest_for(surface)),
            self.keyboard
                .as_ref()
                .and_then(|input| input.latest_for(surface)),
        ]
        .into_iter()
        .flatten()
        .max_by_key(|event| event.time)
    }

    fn release_pointer(&mut self) {
        if let Some(pointer) = self.pointer.take()
            && pointer.proxy.version() >= 3
        {
            pointer.proxy.release();
        }
    }

    fn release_keyboard(&mut self) {
        if let Some(keyboard) = self.keyboard.take()
            && keyboard.proxy.version() >= 3
        {
            keyboard.proxy.release();
        }
    }
}

impl Drop for Seat {
    fn drop(&mut self) {
        self.release_pointer();
        self.release_keyboard();
    }
}

impl Dispatch<WlPointer, ObjectId> for protocol::State {
    fn event(
        state: &mut Self,
        _: &WlPointer,
        event: wl_pointer::Event,
        id: &ObjectId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(input) = state
            .activation
            .seats
            .get_mut(id)
            .and_then(|seat| seat.pointer.as_mut())
        else {
            return;
        };
        match event {
            wl_pointer::Event::Enter {
                surface, serial, ..
            } => input.enter(surface.id().protocol_id(), serial),
            wl_pointer::Event::Leave { .. } => input.leave(),
            wl_pointer::Event::Button { serial, .. } => input.record(serial),
            _ => {}
        }
    }
}

impl Dispatch<WlKeyboard, ObjectId> for protocol::State {
    fn event(
        state: &mut Self,
        _: &WlKeyboard,
        event: wl_keyboard::Event,
        id: &ObjectId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(input) = state
            .activation
            .seats
            .get_mut(id)
            .and_then(|seat| seat.keyboard.as_mut())
        else {
            return;
        };
        match event {
            wl_keyboard::Event::Enter {
                surface, serial, ..
            } => input.enter(surface.id().protocol_id(), serial),
            wl_keyboard::Event::Leave { .. } => input.leave(),
            wl_keyboard::Event::Key { serial, .. } => input.record(serial),
            _ => {}
        }
    }
}
