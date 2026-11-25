//! Use XShape to keep override-redirect covers out of pointer hit testing.
use crate::platform::display_blackout::Bounds;
use std::{
    cell::OnceCell,
    collections::{BTreeMap, BTreeSet, btree_map::Entry},
};
use x11rb::{
    COPY_DEPTH_FROM_PARENT, COPY_FROM_PARENT,
    connection::Connection,
    protocol::{
        shape::{ConnectionExt as _, SK, SO},
        xproto::{ClipOrdering, ConnectionExt as _, CreateWindowAux, Window, WindowClass},
    },
    rust_connection::RustConnection,
};

#[derive(Default)]
pub(super) struct Covers {
    server: OnceCell<Option<Server>>,
    windows: BTreeMap<Bounds, Window>,
}

impl Covers {
    pub(super) fn refresh(&mut self, regions: BTreeSet<Bounds>) {
        if regions.is_empty() {
            self.clear();
            return;
        }
        let Some(server) = self.server.get_or_init(Server::connect) else {
            return;
        };
        self.windows.retain(|region, window| {
            if regions.contains(region) {
                true
            } else {
                let _ = server.connection.destroy_window(*window);
                false
            }
        });
        for region in regions {
            if let Entry::Vacant(slot) = self.windows.entry(region)
                && let Some(window) = server.create(region)
            {
                slot.insert(window);
            }
        }
        let _ = server.connection.flush();
    }

    pub(super) fn clear(&mut self) {
        if self.windows.is_empty() {
            return;
        }
        if let Some(Some(server)) = self.server.get() {
            for (_, window) in std::mem::take(&mut self.windows) {
                let _ = server.connection.destroy_window(window);
            }
            let _ = server.connection.flush();
        }
    }
}

struct Server {
    connection: RustConnection,
    root: Window,
    black: u32,
}

impl Server {
    fn connect() -> Option<Self> {
        let (connection, screen) = RustConnection::connect(None).ok()?;
        let version = connection.shape_query_version().ok()?.reply().ok()?;
        if (version.major_version, version.minor_version) < (1, 1) {
            return None;
        }
        let screen = connection.setup().roots.get(screen)?;
        let (root, black) = (screen.root, screen.black_pixel);
        Some(Self {
            connection,
            root,
            black,
        })
    }

    fn create(&self, region: Bounds) -> Option<Window> {
        let window = self.connection.generate_id().ok()?;
        self.connection
            .create_window(
                COPY_DEPTH_FROM_PARENT,
                window,
                self.root,
                i16::try_from(region.x).ok()?,
                i16::try_from(region.y).ok()?,
                u16::try_from(region.width).ok()?,
                u16::try_from(region.height).ok()?,
                0,
                WindowClass::INPUT_OUTPUT,
                COPY_FROM_PARENT,
                &CreateWindowAux::new()
                    .background_pixel(self.black)
                    .override_redirect(1),
            )
            .ok()?
            .check()
            .ok()?;
        if self
            .connection
            .shape_rectangles(
                SO::SET,
                SK::INPUT,
                ClipOrdering::UNSORTED,
                window,
                0,
                0,
                &[],
            )
            .ok()
            .and_then(|request| request.check().ok())
            .is_some()
            && self
                .connection
                .map_window(window)
                .ok()
                .and_then(|request| request.check().ok())
                .is_some()
        {
            Some(window)
        } else {
            let _ = self.connection.destroy_window(window);
            None
        }
    }
}
