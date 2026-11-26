//! Negotiate copy drops on the X server without blocking the player event loop.
use super::{Notice, Point, Result, Ui, selection};
use std::{
    ffi::OsString,
    os::fd::AsRawFd,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
use x11rb::{
    NONE, atom_manager,
    connection::Connection,
    protocol::{
        Event,
        xproto::{
            Atom, AtomEnum, ClientMessageEvent, ConnectionExt as _, CreateWindowAux, EventMask,
            PropMode, Window, WindowClass,
        },
    },
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
};

const VERSION: u32 = 5;

atom_manager! {
    pub(super) Atoms: AtomCookies {
        XdndAware,
        XdndProxy,
        XdndEnter,
        XdndPosition,
        XdndStatus,
        XdndLeave,
        XdndDrop,
        XdndFinished,
        XdndSelection,
        XdndTypeList,
        XdndActionCopy,
        RIINA_DROP: b"_RIINA_XDND_DATA",
        INCR,
        URI_LIST: b"text/uri-list",
        UTF8_TEXT: b"text/plain;charset=utf-8",
        UTF8_STRING,
        TEXT: b"text/plain",
    }
}

pub(super) struct Proxy(Arc<Server>);

struct Server {
    conn: Arc<RustConnection>,
    atoms: Atoms,
    root: Window,
    target: Window,
    proxy: Window,
    closed: AtomicBool,
}

impl Proxy {
    pub(super) fn start(target: Window, id: u64) -> Result<Self> {
        let (conn, _) = x11rb::connect(None)?;
        let root = conn.get_geometry(target)?.reply()?.root;
        let atoms = Atoms::new(&conn)?.reply()?;
        let existing = conn
            .get_property(false, target, atoms.XdndProxy, AtomEnum::ANY, 0, 1)?
            .reply()?;
        if existing.type_ != NONE {
            return Err("The window already has an Xdnd proxy".into());
        }
        let proxy = conn.generate_id()?;
        conn.create_window(
            0,
            proxy,
            root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_ONLY,
            0,
            &CreateWindowAux::new().event_mask(EventMask::STRUCTURE_NOTIFY),
        )?
        .check()?;
        conn.change_property32(
            PropMode::REPLACE,
            proxy,
            atoms.XdndAware,
            AtomEnum::ATOM,
            &[VERSION],
        )?
        .check()?;
        conn.change_property32(
            PropMode::REPLACE,
            proxy,
            atoms.XdndProxy,
            AtomEnum::WINDOW,
            &[proxy],
        )?
        .check()?;
        conn.change_property32(
            PropMode::REPLACE,
            target,
            atoms.XdndProxy,
            AtomEnum::WINDOW,
            &[proxy],
        )?
        .check()?;
        let owner = Self(Arc::new(Server {
            conn: Arc::new(conn),
            atoms,
            root,
            target,
            proxy,
            closed: AtomicBool::new(false),
        }));
        let server = owner.0.clone();
        crate::background::spawn_named("riina-x11-drop", move || {
            let mut worker = Worker {
                server: &server,
                ui: Ui::new(id),
                current: None,
                serial: 0,
            };
            if let Err(error) = worker.run()
                && !server.closed.load(Ordering::Acquire)
            {
                crate::diagnostics::warn!("X11 drop receiver stopped: {error}");
            }
            worker.ui.update(Notice::Left);
            server.close();
        })?;
        Ok(owner)
    }
}

impl Drop for Proxy {
    fn drop(&mut self) {
        self.0.close();
    }
}

impl Server {
    fn close(&self) {
        if self.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        if self
            .conn
            .get_property(
                false,
                self.target,
                self.atoms.XdndProxy,
                AtomEnum::WINDOW,
                0,
                1,
            )
            .ok()
            .and_then(|request| request.reply().ok())
            .and_then(|reply| reply.value32()?.next())
            == Some(self.proxy)
        {
            let _ = self.conn.delete_property(self.target, self.atoms.XdndProxy);
        }
        let _ = self.conn.destroy_window(self.proxy);
        let _ = self.conn.flush();
    }

    fn message(&self, source: Window, atom: Atom, data: [u32; 5]) -> Result<()> {
        self.conn.send_event(
            false,
            source,
            EventMask::NO_EVENT,
            ClientMessageEvent::new(32, source, atom, data),
        )?;
        Ok(())
    }

    fn position(&self, packed: u32) -> Result<Option<Point>> {
        let [a, b, c, d] = packed.to_be_bytes();
        let x = i16::from_be_bytes([a, b]);
        let y = i16::from_be_bytes([c, d]);
        let pos = self
            .conn
            .translate_coordinates(self.root, self.target, x, y)?
            .reply()?;
        Ok(pos.same_screen.then_some((pos.dst_x, pos.dst_y)))
    }
}

struct Worker<'a> {
    server: &'a Server,
    ui: Ui,
    current: Option<Drag>,
    serial: u64,
}

struct Drag {
    id: u64,
    source: Window,
    version: u32,
    mime: Option<selection::Mime>,
    position: Option<Point>,
    released: bool,
    data: Data,
}

enum Data {
    Unrequested,
    Pending(selection::Transfer),
    Ready(Vec<OsString>),
}

impl Worker<'_> {
    fn run(&mut self) -> Result<()> {
        while !self.server.closed.load(Ordering::Acquire) {
            let Some(event) = self.next_event()? else {
                self.resolve(Vec::new())?;
                self.server.conn.flush()?;
                continue;
            };
            match event {
                Event::DestroyNotify(event) if event.window == self.server.proxy => break,
                Event::ClientMessage(event)
                    if event.format == 32
                        && (event.window == self.server.target
                            || event.window == self.server.proxy) =>
                {
                    self.client_message(event);
                }
                event => self.selection_event(&event),
            }
            self.server.conn.flush()?;
        }
        Ok(())
    }

    fn next_event(&self) -> Result<Option<Event>> {
        let deadline = self.current.as_ref().and_then(|drag| match &drag.data {
            Data::Pending(transfer) => Some(transfer.deadline()),
            _ => None,
        });
        let Some(deadline) = deadline else {
            return self
                .server
                .conn
                .wait_for_event()
                .map(Some)
                .map_err(Into::into);
        };
        loop {
            if Instant::now() >= deadline {
                return Ok(None);
            }
            if let Some(event) = self.server.conn.poll_for_event()? {
                return Ok(Some(event));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            let ms = i32::try_from(remaining.as_millis().max(1)).unwrap_or(i32::MAX);
            let mut fd = libc::pollfd {
                fd: self.server.conn.stream().as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: the connection owns its socket and poll borrows one stack record.
            if unsafe { libc::poll(&raw mut fd, 1, ms) } < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() != std::io::ErrorKind::Interrupted {
                    return Err(error.into());
                }
            }
        }
    }

    fn client_message(&mut self, event: ClientMessageEvent) {
        let data = event.data.as_data32();
        let atoms = &self.server.atoms;
        let result = if event.type_ == atoms.XdndEnter {
            self.enter(data)
        } else if self
            .current
            .as_ref()
            .is_some_and(|drag| drag.source == data[0])
        {
            match event.type_ {
                kind if kind == atoms.XdndPosition => self.position(data[2], data[3]),
                kind if kind == atoms.XdndLeave => self.retire(),
                kind if kind == atoms.XdndDrop => self.release(),
                _ => Ok(()),
            }
        } else {
            Ok(())
        };
        if result.is_err() {
            let _ = self.retire();
        }
    }

    fn enter(&mut self, data: [u32; 5]) -> Result<()> {
        self.retire()?;
        let source = data[0];
        let version = data[1] >> 24;
        if source == NONE || version < 3 {
            return Ok(());
        }
        let atoms = &self.server.atoms;
        let types = if data[1] & 1 != 0 {
            let reply = self
                .server
                .conn
                .get_property(false, source, atoms.XdndTypeList, AtomEnum::ATOM, 0, 256)?
                .reply()?;
            reply.value32().into_iter().flatten().collect()
        } else {
            data[2..].to_vec()
        };
        self.serial = self.serial.wrapping_add(1);
        self.current = Some(Drag {
            id: self.serial,
            source,
            version,
            mime: selection::Mime::choose(atoms, &types),
            position: None,
            released: false,
            data: Data::Unrequested,
        });
        Ok(())
    }

    fn position(&mut self, packed: u32, time: u32) -> Result<()> {
        let Some(drag) = self.current.as_mut().filter(|drag| !drag.released) else {
            return Ok(());
        };
        drag.position = self.server.position(packed)?;
        if let (Some(mime), Some(point)) = (drag.mime, drag.position) {
            match &drag.data {
                Data::Unrequested => {
                    drag.data = Data::Pending(selection::Transfer::start(
                        self.server.conn.clone(),
                        self.server.root,
                        &self.server.atoms,
                        mime,
                        time,
                    )?);
                }
                Data::Ready(files) if !files.is_empty() => self.ui.update(Notice::Moved(point)),
                _ => {}
            }
        }
        let accept = drag.mime.is_some()
            && drag.position.is_some()
            && !matches!(&drag.data, Data::Ready(files) if files.is_empty());
        // Keep requesting motion so the insertion marker can follow the pointer.
        self.server.message(
            drag.source,
            self.server.atoms.XdndStatus,
            [
                self.server.target,
                2 | u32::from(accept),
                0,
                0,
                if accept {
                    self.server.atoms.XdndActionCopy
                } else {
                    NONE
                },
            ],
        )
    }

    fn release(&mut self) -> Result<()> {
        let Some(drag) = self.current.as_mut().filter(|drag| !drag.released) else {
            return Ok(());
        };
        drag.released = true;
        if let Some(point) = drag.position {
            self.ui.update(Notice::Release(drag.id, point));
        }
        if !matches!(drag.data, Data::Pending(_)) {
            self.complete()?;
        }
        Ok(())
    }

    fn selection_event(&mut self, event: &Event) {
        let Some(Drag {
            data: Data::Pending(transfer),
            ..
        }) = self.current.as_mut()
        else {
            return;
        };
        match transfer.receive(&self.server.atoms, event) {
            Ok(Some(files)) => {
                let _ = self.resolve(files);
            }
            Ok(None) => {}
            Err(_) => {
                let _ = self.resolve(Vec::new());
            }
        }
    }

    fn resolve(&mut self, files: Vec<OsString>) -> Result<()> {
        if let Some(drag) = self.current.as_mut() {
            if !drag.released
                && !files.is_empty()
                && let Some(position) = drag.position
            {
                self.ui.update(Notice::Hover(
                    position,
                    files.iter().map(PathBuf::from).collect(),
                ));
            }
            drag.data = Data::Ready(files);
            if drag.released {
                self.complete()?;
            }
        }
        Ok(())
    }

    fn complete(&mut self) -> Result<()> {
        if let Some(drag) = self.current.take() {
            let files = match drag.data {
                Data::Ready(files) if drag.position.is_some() => files,
                _ => Vec::new(),
            };
            let accepted = !files.is_empty();
            self.ui.update(Notice::Finished(drag.id, files));
            self.reply_finished(drag.source, drag.version, accepted)?;
        }
        Ok(())
    }

    fn retire(&mut self) -> Result<()> {
        if let Some(drag) = self.current.take() {
            self.ui.update(Notice::Left);
            if drag.released {
                self.reply_finished(drag.source, drag.version, false)?;
            }
        }
        Ok(())
    }

    fn reply_finished(&self, source: Window, version: u32, accepted: bool) -> Result<()> {
        let accepted = version >= 5 && accepted;
        self.server.message(
            source,
            self.server.atoms.XdndFinished,
            [
                self.server.target,
                u32::from(accepted),
                if accepted {
                    self.server.atoms.XdndActionCopy
                } else {
                    NONE
                },
                0,
                0,
            ],
        )
    }
}
