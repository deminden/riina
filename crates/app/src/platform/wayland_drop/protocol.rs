//! Own each seat's data device and separate a hover offer from a completed drop.
use super::{Hover, OpenDrop, Target, activation, blackout, find_target, payload};
use slint::LogicalPosition;
use std::{
    collections::HashMap,
    ffi::OsString,
    path::PathBuf,
    rc::Rc,
    sync::{Mutex, mpsc},
};
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle, WEnum,
    backend::ObjectId,
    delegate_noop, event_created_child,
    protocol::{
        wl_data_device::{self, WlDataDevice},
        wl_data_device_manager::{DndAction, WlDataDeviceManager},
        wl_data_offer::{self, WlDataOffer},
        wl_registry::{self, WlRegistry},
        wl_seat::{self, WlSeat},
    },
};

pub(super) struct State {
    pub(super) activation: activation::State,
    blackout: blackout::Capabilities,
    _registry: WlRegistry,
    manager: Option<WlDataDeviceManager>,
    seats: HashMap<u32, Seat>,
    current: HashMap<ObjectId, Drag>,
    released: HashMap<u64, (Drag, OpenDrop)>,
    next_serial: u64,
    send: mpsc::Sender<payload::Result>,
    results: mpsc::Receiver<payload::Result>,
}

impl State {
    pub(super) fn new(registry: WlRegistry) -> Self {
        let (send, results) = mpsc::channel();
        Self {
            activation: activation::State::default(),
            blackout: blackout::Capabilities::default(),
            _registry: registry,
            manager: None,
            seats: HashMap::new(),
            current: HashMap::new(),
            released: HashMap::new(),
            next_serial: 0,
            send,
            results,
        }
    }

    pub(super) fn deliver_payloads(&mut self) {
        while let Ok(payload) = self.results.try_recv() {
            if let Some((drag, open)) = self.released.remove(&payload.serial) {
                drag.complete(open, payload.files);
            } else if let Some(drag) = self
                .current
                .values_mut()
                .find(|drag| drag.serial == payload.serial)
            {
                let paths = payload.files.iter().map(PathBuf::from).collect::<Vec<_>>();
                drag.files = Some(payload.files);
                if !paths.is_empty() {
                    (drag.target.hover)(Hover::files(paths, drag.position));
                }
            }
        }
    }

    fn leave(&mut self, device: &ObjectId) {
        if let Some(drag) = self.current.remove(device) {
            (drag.target.hover)(Hover::Left);
        }
    }

    fn release(&mut self, device: &ObjectId) {
        let Some(mut drag) = self.current.remove(device) else {
            return;
        };
        let open = (drag.target.capture)(drag.position);
        (drag.target.hover)(Hover::Left);
        if let Some(open) = open {
            if let Some(files) = drag.files.take() {
                drag.complete(open, files);
            } else {
                self.released.insert(drag.serial, (drag, open));
            }
        }
    }
}

struct Seat {
    proxy: WlSeat,
    device: Option<WlDataDevice>,
}

impl Drop for Seat {
    fn drop(&mut self) {
        if let Some(device) = self.device.take() {
            device.release();
        }
        if self.proxy.version() >= 5 {
            self.proxy.release();
        }
    }
}

struct Drag {
    serial: u64,
    offer: WlDataOffer,
    _read: payload::ReadRequest,
    target: Rc<Target>,
    scale: f32,
    position: LogicalPosition,
    files: Option<Vec<OsString>>,
}

impl Drag {
    fn complete(self, open: OpenDrop, files: Vec<OsString>) {
        if files.is_empty() {
            return;
        }
        let action = self
            .offer
            .data::<Mutex<Offer>>()
            .and_then(|data| riina_core::sync::lock(data).action);
        if action == Some(DndAction::Copy) {
            self.offer.finish();
            open(files);
        }
    }
}

impl Drop for Drag {
    fn drop(&mut self) {
        self.offer.destroy();
    }
}

impl Dispatch<WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        queue: &QueueHandle<Self>,
    ) {
        state.blackout.observe(&event);
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } if interface == activation::interface() && version >= 1 => {
                state.activation.bind(name, registry, queue);
            }
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } if interface == WlDataDeviceManager::interface().name
                && version >= 3
                && state.manager.is_none() =>
            {
                let manager: WlDataDeviceManager = registry.bind(name, 3, queue, ());
                for seat in state.seats.values_mut() {
                    seat.device = Some(manager.get_data_device(&seat.proxy, queue, ()));
                }
                state.manager = Some(manager);
            }
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } if interface == WlSeat::interface().name => {
                let proxy: WlSeat = registry.bind(name, version.min(5), queue, ());
                let device = state
                    .manager
                    .as_ref()
                    .map(|manager| manager.get_data_device(&proxy, queue, ()));
                state.seats.insert(name, Seat { proxy, device });
            }
            wl_registry::Event::GlobalRemove { name } => {
                state.activation.remove_global(name);
                if let Some(seat) = state.seats.remove(&name) {
                    state.activation.remove_seat(&seat.proxy.id());
                    if let Some(device) = seat.device.as_ref() {
                        state.leave(&device.id());
                    }
                }
            }
            _ => {}
        }
    }
}

delegate_noop!(State: ignore WlDataDeviceManager);

impl Dispatch<WlSeat, ()> for State {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        queue: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(capabilities),
        } = event
        {
            state.activation.track_input(seat, capabilities, queue);
        }
    }
}

#[derive(Default)]
struct Offer {
    mimes: Vec<String>,
    action: Option<DndAction>,
}

impl Offer {
    fn mime(&self) -> Option<&str> {
        std::iter::once(payload::URI_LIST)
            .chain(payload::TEXT_MIMES.iter().copied())
            .find(|mime| self.mimes.iter().any(|offered| offered == *mime))
    }
}

impl Dispatch<WlDataOffer, Mutex<Offer>> for State {
    fn event(
        _: &mut Self,
        _: &WlDataOffer,
        event: wl_data_offer::Event,
        data: &Mutex<Offer>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let mut data = riina_core::sync::lock(data);
        match event {
            wl_data_offer::Event::Offer { mime_type } => data.mimes.push(mime_type),
            wl_data_offer::Event::Action {
                dnd_action: WEnum::Value(action),
            } => data.action = Some(action),
            _ => {}
        }
    }
}

impl Dispatch<WlDataDevice, ()> for State {
    event_created_child!(State, WlDataDevice, [
        wl_data_device::EVT_DATA_OFFER_OPCODE => (WlDataOffer, Mutex::<Offer>::default())
    ]);

    fn event(
        state: &mut Self,
        device: &WlDataDevice,
        event: wl_data_device::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_device::Event::Enter {
                serial,
                surface,
                x,
                y,
                id: Some(offer),
            } => {
                let device = device.id();
                state.leave(&device);
                let target = find_target(surface.id().as_ptr().addr());
                let mime = offer
                    .data::<Mutex<Offer>>()
                    .and_then(|data| riina_core::sync::lock(data).mime().map(str::to_owned));
                if let (Some((target, scale)), Some(mime)) = (target, mime) {
                    state.next_serial = state.next_serial.wrapping_add(1);
                    let transfer = state.next_serial;
                    offer.accept(serial, Some(mime.clone()));
                    offer.set_actions(DndAction::Copy, DndAction::Copy);
                    if let Ok(read) =
                        payload::ReadRequest::start(&offer, &mime, transfer, state.send.clone())
                    {
                        state.current.insert(
                            device,
                            Drag {
                                serial: transfer,
                                offer,
                                _read: read,
                                target,
                                scale,
                                position: LogicalPosition::new(x as f32 * scale, y as f32 * scale),
                                files: None,
                            },
                        );
                        return;
                    }
                }
                offer.accept(serial, None);
                offer.destroy();
            }
            wl_data_device::Event::Motion { x, y, .. } => {
                if let Some(drag) = state.current.get_mut(&device.id()) {
                    drag.position =
                        LogicalPosition::new(x as f32 * drag.scale, y as f32 * drag.scale);
                    (drag.target.hover)(Hover::Moved(drag.position));
                }
            }
            wl_data_device::Event::Leave => state.leave(&device.id()),
            wl_data_device::Event::Drop => state.release(&device.id()),
            wl_data_device::Event::Selection { id: Some(offer) } => offer.destroy(),
            _ => {}
        }
    }
}
