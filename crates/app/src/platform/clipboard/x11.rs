//! Own the X11 selection for copied playlist references while the app runs.
mod selection;

use self::selection::{Atoms, Contents};
use super::uri_list;
use std::{
    cell::RefCell,
    sync::{Arc, mpsc},
    time::Duration,
};
use x11rb::{
    CURRENT_TIME, NONE,
    connection::Connection,
    protocol::{
        Event,
        xproto::{self, ConnectionExt as _, CreateWindowAux, Window, WindowClass},
    },
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

thread_local! {
    static CLIPBOARD: RefCell<Option<Owner>> = const { RefCell::new(None) };
}

pub(super) fn copy_references(uris: String, text: &str) -> Result<()> {
    let owner = Owner::start(Contents::new(uris, text))?;
    CLIPBOARD.set(Some(owner));
    Ok(())
}

pub(super) fn read_owned_references() -> Option<Vec<String>> {
    CLIPBOARD.with_borrow(|provider| {
        let server = &provider.as_ref()?.server;
        server
            .owns_clipboard()
            .ok()?
            .then(|| uri_list::decode(server.contents.uris()))
    })
}

struct Owner {
    server: Arc<Server>,
    saved: mpsc::Receiver<()>,
}

struct Server {
    conn: RustConnection,
    window: Window,
    atoms: Atoms,
    contents: Contents,
}

impl Owner {
    fn start(contents: Contents) -> Result<Self> {
        let (conn, screen) = x11rb::connect(None)?;
        let root = conn
            .setup()
            .roots
            .get(screen)
            .ok_or("Invalid X11 screen")?
            .root;
        let window = conn.generate_id()?;
        conn.create_window(
            0,
            window,
            root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_ONLY,
            0,
            &CreateWindowAux::new().event_mask(
                xproto::EventMask::PROPERTY_CHANGE | xproto::EventMask::STRUCTURE_NOTIFY,
            ),
        )?
        .check()?;
        let atoms = Atoms::new(&conn)?.reply()?;
        let server = Arc::new(Server {
            conn,
            window,
            atoms,
            contents,
        });
        let timestamp = server.timestamp()?;
        let (saved_tx, saved) = mpsc::channel();
        // Start serving requests before publishing the new owner to other apps.
        let worker = server.clone();
        std::thread::Builder::new()
            .name("riina-x11-clipboard".into())
            .spawn(move || worker.serve(timestamp, saved_tx))?;
        let owner = Self { server, saved };
        owner.server.claim(timestamp)?;
        Ok(owner)
    }

    fn save(&self) -> Result<()> {
        let Server {
            conn,
            window,
            atoms,
            ..
        } = &*self.server;
        if !self.server.owns_clipboard()?
            || conn
                .get_selection_owner(atoms.CLIPBOARD_MANAGER)?
                .reply()?
                .owner
                == NONE
        {
            return Ok(());
        }
        conn.convert_selection(
            *window,
            atoms.CLIPBOARD_MANAGER,
            atoms.SAVE_TARGETS,
            atoms.RIINA_PROPERTY,
            CURRENT_TIME,
        )?
        .check()?;
        // A clipboard manager can keep the copied files after Riina exits.
        let _ = self.saved.recv_timeout(Duration::from_millis(250));
        Ok(())
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        let _ = self.save();
        let _ = self.server.conn.destroy_window(self.server.window);
        let _ = self.server.conn.flush();
    }
}

impl Server {
    fn owns_clipboard(&self) -> Result<bool> {
        Ok(self
            .conn
            .get_selection_owner(self.atoms.CLIPBOARD)?
            .reply()?
            .owner
            == self.window)
    }

    fn timestamp(&self) -> Result<u32> {
        self.conn
            .change_property8(
                xproto::PropMode::REPLACE,
                self.window,
                self.atoms.RIINA_PROPERTY,
                xproto::AtomEnum::INTEGER,
                &[],
            )?
            .check()?;
        loop {
            if let Event::PropertyNotify(event) = self.conn.wait_for_event()?
                && event.window == self.window
                && event.atom == self.atoms.RIINA_PROPERTY
            {
                self.conn
                    .delete_property(self.window, event.atom)?
                    .check()?;
                return Ok(event.time);
            }
        }
    }

    fn claim(&self, timestamp: u32) -> Result<()> {
        self.conn
            .set_selection_owner(self.window, self.atoms.CLIPBOARD, timestamp)?
            .check()?;
        if self.owns_clipboard()? {
            Ok(())
        } else {
            Err("Could not take ownership of the X11 clipboard".into())
        }
    }

    fn serve(&self, timestamp: u32, saved: mpsc::Sender<()>) {
        while let Ok(event) = self.conn.wait_for_event() {
            match event {
                Event::SelectionRequest(request) if request.selection == self.atoms.CLIPBOARD => {
                    let _ = selection::reply(self, &self.contents, timestamp, request);
                }
                Event::SelectionNotify(event)
                    if event.selection == self.atoms.CLIPBOARD_MANAGER
                        && event.target == self.atoms.SAVE_TARGETS =>
                {
                    let _ = saved.send(());
                }
                Event::DestroyNotify(event) if event.window == self.window => break,
                _ => {}
            }
        }
    }
}
