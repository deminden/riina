//! Keep blackout output proxies separate from the toolkit's display connection.
mod worker;

use std::{
    cell::Cell,
    io::{self, Write},
    os::unix::net::UnixStream,
    sync::mpsc,
};
use wayland_client::protocol::wl_registry::Event;

const REQUIRED_GLOBALS: [&str; 4] = [
    "wl_compositor",
    "wl_shm",
    "wp_viewporter",
    "zwlr_layer_shell_v1",
];

thread_local! {
    static SUPPORTED: Cell<Option<bool>> = const { Cell::new(None) };
}

pub(super) fn wayland_started() {
    SUPPORTED.set(Some(false));
}

pub(in crate::platform) fn available() -> bool {
    SUPPORTED.get().unwrap_or(true)
}

#[derive(Default)]
pub(super) struct Capabilities([Option<u32>; REQUIRED_GLOBALS.len()]);

impl Capabilities {
    pub(super) fn observe(&mut self, event: &Event) {
        match event {
            Event::Global {
                name, interface, ..
            } => {
                if let Some(index) = REQUIRED_GLOBALS
                    .iter()
                    .position(|required| *required == interface)
                {
                    self.0[index] = Some(*name);
                }
            }
            Event::GlobalRemove { name } => {
                for global in &mut self.0 {
                    if *global == Some(*name) {
                        *global = None;
                    }
                }
            }
            _ => return,
        }
        SUPPORTED.set(Some(self.0.iter().all(Option::is_some)));
    }
}

#[derive(Default)]
pub(in crate::platform) struct Request {
    display: Option<u32>,
    worker: Option<Worker>,
}

impl Request {
    pub(in crate::platform) fn set(&mut self, display: Option<u32>) {
        if display == self.display {
            return;
        }
        self.display = display;
        let Some(display) = display else {
            self.worker.take();
            return;
        };
        if let Some(worker) = &mut self.worker {
            if !worker.move_to(display) {
                self.worker.take();
            }
        } else {
            match Worker::start(display) {
                Ok(worker) => self.worker = Some(worker),
                Err(error) => {
                    crate::diagnostics::warn!("Could not start display blackout: {error}")
                }
            }
        }
    }
}

struct Worker {
    send: mpsc::Sender<u32>,
    wake: UnixStream,
}

impl Worker {
    fn start(display: u32) -> io::Result<Self> {
        let (send, receive) = mpsc::channel();
        let (wake, reader) = UnixStream::pair()?;
        wake.set_nonblocking(true)?;
        crate::background::spawn_named("wayland-display-blackout", move || {
            if let Err(error) = worker::run(display, reader, receive) {
                crate::diagnostics::warn!("Wayland display blackout ended: {error}");
            }
        })?;
        Ok(Self { send, wake })
    }

    fn move_to(&mut self, display: u32) -> bool {
        self.send.send(display).is_ok()
            && match self.wake.write_all(&[1]) {
                Ok(()) => true,
                Err(error) => error.kind() == io::ErrorKind::WouldBlock,
            }
    }
}
