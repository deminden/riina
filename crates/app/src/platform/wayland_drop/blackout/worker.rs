//! Draw non-interactive layers on a separate Wayland connection.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{self, Read, Write},
    os::{
        fd::{AsFd, AsRawFd, FromRawFd},
        unix::net::UnixStream,
    },
    sync::mpsc,
};
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle, delegate_noop,
    protocol::{
        wl_buffer::WlBuffer,
        wl_compositor::WlCompositor,
        wl_output::WlOutput,
        wl_region::WlRegion,
        wl_registry::{Event as GlobalEvent, WlRegistry},
        wl_shm::{Format, WlShm},
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
    },
};
use wayland_protocols::wp::viewporter::client::{
    wp_viewport::WpViewport, wp_viewporter::WpViewporter,
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{Layer, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{self, Anchor, KeyboardInteractivity, ZwlrLayerSurfaceV1},
};

pub(super) fn run(
    mut active: u32,
    mut wake: UnixStream,
    changes: mpsc::Receiver<u32>,
) -> Result<(), Box<dyn std::error::Error>> {
    let connection = Connection::connect_to_env()?;
    let mut queue = connection.new_event_queue();
    let _registry = connection.display().get_registry(&queue.handle(), ());
    let mut state = State::default();
    loop {
        if let Some(display) = changes.try_iter().last() {
            active = display;
        }
        queue.dispatch_pending(&mut state)?;
        state.sync(active, &queue.handle());
        queue.flush()?;
        let Some(read) = queue.prepare_read() else {
            continue;
        };
        let mut sockets = [
            libc::pollfd {
                fd: connection.as_fd().as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: wake.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        // SAFETY: Both descriptors stay open and the array has the declared length.
        if unsafe { libc::poll(sockets.as_mut_ptr(), 2, -1) } < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error.into());
        }
        const CLOSED: libc::c_short = libc::POLLHUP | libc::POLLERR | libc::POLLNVAL;
        if sockets[1].revents & CLOSED != 0 {
            return Ok(());
        }
        if sockets[1].revents & libc::POLLIN != 0 && wake.read(&mut [0; 64])? == 0 {
            return Ok(());
        }
        if sockets[0].revents & CLOSED != 0 {
            return Err(io::Error::other("Display connection closed").into());
        }
        if sockets[0].revents & libc::POLLIN != 0 {
            read.read()?;
        }
    }
}

#[derive(Default)]
struct State {
    compositor: Option<(u32, WlCompositor)>,
    shm: Option<(u32, WlShm)>,
    viewporter: Option<(u32, WpViewporter)>,
    shell: Option<(u32, ZwlrLayerShellV1)>,
    outputs: BTreeMap<u32, WlOutput>,
    covers: BTreeMap<u32, Cover>,
    refused: BTreeSet<u32>,
    active_output: Option<u32>,
    black: Option<WlBuffer>,
}

impl State {
    fn registry_event(
        &mut self,
        registry: &WlRegistry,
        event: &GlobalEvent,
        queue: &QueueHandle<Self>,
    ) {
        match event {
            GlobalEvent::Global {
                name,
                interface,
                version,
            } => {
                let (name, version) = (*name, *version);
                if interface == WlCompositor::interface().name && self.compositor.is_none() {
                    self.compositor = Some((name, registry.bind(name, 1, queue, ())));
                } else if interface == WlShm::interface().name && self.shm.is_none() {
                    self.shm = Some((name, registry.bind(name, 1, queue, ())));
                } else if interface == WpViewporter::interface().name && self.viewporter.is_none() {
                    self.viewporter = Some((name, registry.bind(name, 1, queue, ())));
                } else if interface == ZwlrLayerShellV1::interface().name && self.shell.is_none() {
                    self.shell = Some((name, registry.bind(name, version.min(3), queue, ())));
                } else if interface == WlOutput::interface().name {
                    self.outputs
                        .insert(name, registry.bind(name, version.min(3), queue, ()));
                }
            }
            GlobalEvent::GlobalRemove { name } => self.remove_global(*name),
            _ => {}
        }
    }

    fn remove_global(&mut self, name: u32) {
        clear_removed(&mut self.compositor, name);
        clear_removed(&mut self.shm, name);
        clear_removed(&mut self.viewporter, name);
        clear_removed(&mut self.shell, name);
        self.covers.remove(&name);
        self.refused.remove(&name);
        if let Some(output) = self.outputs.remove(&name)
            && output.version() >= 3
        {
            output.release();
        }
    }

    fn sync(&mut self, active: u32, queue: &QueueHandle<Self>) {
        let active = self.outputs.contains_key(&active).then_some(active);
        if active != self.active_output {
            self.active_output = active;
            self.refused.clear();
        }
        let Some(active) = active else {
            self.covers.clear();
            return;
        };
        self.covers
            .retain(|id, _| *id != active && self.outputs.contains_key(id));
        if self.outputs.len() <= 1 {
            return;
        }
        let (Some((_, compositor)), Some((_, shm)), Some((_, viewporter)), Some((_, shell))) =
            (&self.compositor, &self.shm, &self.viewporter, &self.shell)
        else {
            self.covers.clear();
            return;
        };
        if self.black.is_none() {
            self.black = black_buffer(shm, queue);
        }
        if self.black.is_none() {
            return;
        }
        let outputs = &self.outputs;
        let covers = &mut self.covers;
        for (&id, output) in outputs {
            if id != active && !self.refused.contains(&id) {
                covers.entry(id).or_insert_with(|| {
                    Cover::new(id, output, compositor, viewporter, shell, queue)
                });
            }
        }
    }
}

fn clear_removed<T>(item: &mut Option<(u32, T)>, name: u32) {
    if item.as_ref().is_some_and(|(id, _)| *id == name) {
        item.take();
    }
}

impl Drop for State {
    fn drop(&mut self) {
        self.covers.clear();
        if let Some(buffer) = self.black.take() {
            buffer.destroy();
        }
    }
}

struct Cover {
    surface: WlSurface,
    viewport: WpViewport,
    layer: ZwlrLayerSurfaceV1,
}

impl Cover {
    fn new(
        id: u32,
        output: &WlOutput,
        compositor: &WlCompositor,
        viewporter: &WpViewporter,
        shell: &ZwlrLayerShellV1,
        queue: &QueueHandle<State>,
    ) -> Self {
        let surface = compositor.create_surface(queue, ());
        let viewport = viewporter.get_viewport(&surface, queue, ());
        let layer = shell.get_layer_surface(
            &surface,
            Some(output),
            Layer::Overlay,
            "riina-fullscreen-blackout".to_owned(),
            queue,
            id,
        );
        layer.set_anchor(Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right);
        layer.set_size(0, 0);
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        let empty = compositor.create_region(queue, ());
        surface.set_input_region(Some(&empty));
        empty.destroy();
        surface.commit();
        Self {
            surface,
            viewport,
            layer,
        }
    }
}

impl Drop for Cover {
    fn drop(&mut self) {
        self.layer.destroy();
        self.viewport.destroy();
        self.surface.destroy();
    }
}

fn black_buffer(shm: &WlShm, queue: &QueueHandle<State>) -> Option<WlBuffer> {
    // SAFETY: The fixed, NUL-terminated name is borrowed during the call.
    let fd = unsafe { libc::memfd_create(c"riina-black-pixel".as_ptr(), libc::MFD_CLOEXEC) };
    if fd < 0 {
        return None;
    }
    // SAFETY: A successful memfd_create returned a fresh, owned descriptor.
    let mut file = unsafe { File::from_raw_fd(fd) };
    file.write_all(&[0; 4]).ok()?;
    let pool = shm.create_pool(file.as_fd(), 4, queue, ());
    let buffer = pool.create_buffer(0, 1, 1, 4, Format::Xrgb8888, queue, ());
    pool.destroy();
    Some(buffer)
}

impl Dispatch<WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: GlobalEvent,
        _: &(),
        _: &Connection,
        queue: &QueueHandle<Self>,
    ) {
        state.registry_event(registry, &event, queue);
    }
}

impl Dispatch<ZwlrLayerSurfaceV1, u32> for State {
    fn event(
        state: &mut Self,
        layer: &ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        id: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(cover) = state.covers.get(id).filter(|cover| cover.layer == *layer) else {
            return;
        };
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                layer.ack_configure(serial);
                if let (Ok(width), Ok(height)) =
                    (i32::try_from(width.max(1)), i32::try_from(height.max(1)))
                {
                    cover.viewport.set_destination(width, height);
                    cover.surface.attach(state.black.as_ref(), 0, 0);
                    cover.surface.damage(0, 0, width, height);
                    cover.surface.commit();
                }
            }
            zwlr_layer_surface_v1::Event::Closed => {
                state.covers.remove(id);
                state.refused.insert(*id);
            }
            _ => {}
        }
    }
}

delegate_noop!(State: ignore WlCompositor);
delegate_noop!(State: ignore WlShm);
delegate_noop!(State: ignore WlShmPool);
delegate_noop!(State: ignore WlBuffer);
delegate_noop!(State: ignore WlOutput);
delegate_noop!(State: ignore WlSurface);
delegate_noop!(State: ignore WlRegion);
delegate_noop!(State: ignore WpViewporter);
delegate_noop!(State: ignore WpViewport);
delegate_noop!(State: ignore ZwlrLayerShellV1);
