use super::{Result, Server};
use x11rb::{
    NONE,
    connection::RequestConnection,
    protocol::xproto::{
        Atom, AtomEnum, ConnectionExt as _, EventMask, PropMode, SELECTION_NOTIFY_EVENT,
        SelectionNotifyEvent, SelectionRequestEvent, Window,
    },
    wrapper::ConnectionExt as _,
};

x11rb::atom_manager! {
    pub(super) Atoms: AtomCookies {
        CLIPBOARD,
        CLIPBOARD_MANAGER,
        SAVE_TARGETS,
        TARGETS,
        MULTIPLE,
        ATOM_PAIR,
        TIMESTAMP,
        NULL,
        UTF8_STRING,
        TEXT,
        MIME_TEXT: b"text/plain;charset=utf-8",
        MIME_URI_LIST: b"text/uri-list",
        GNOME_FILES: b"x-special/gnome-copied-files",
        RIINA_PROPERTY: b"_RIINA_PLAYLIST_CLIPBOARD",
    }
}

pub(super) struct Contents {
    text: String,
    uris: String,
    desktop_files: String,
}

impl Contents {
    pub(super) fn new(uris: String, text: &str) -> Self {
        // File managers can interpret this action as a request to move the
        // underlying files. Playlist Cut only removes rows from the queue.
        let desktop_files = format!("copy\n{}", uris.replace("\r\n", "\n"));
        Self {
            text: text.to_owned(),
            uris,
            desktop_files,
        }
    }

    pub(super) fn uris(&self) -> &str {
        &self.uris
    }
}

pub(super) fn reply(
    server: &Server,
    contents: &Contents,
    timestamp: u32,
    request: SelectionRequestEvent,
) -> Result<()> {
    let property = if request.property == NONE && request.target != server.atoms.MULTIPLE {
        // Older X11 selection clients omit the destination property.
        request.target
    } else {
        request.property
    };
    let transfer = Transfer {
        server,
        contents,
        timestamp,
        requestor: request.requestor,
    };
    let result = if property == NONE {
        Ok(false)
    } else if request.target == server.atoms.MULTIPLE {
        transfer.write_multiple(property)
    } else {
        transfer.write(request.target, property)
    };
    let property = if result.unwrap_or(false) {
        property
    } else {
        NONE
    };
    server
        .conn
        .send_event(
            false,
            request.requestor,
            EventMask::NO_EVENT,
            SelectionNotifyEvent {
                response_type: SELECTION_NOTIFY_EVENT,
                sequence: 0,
                time: request.time,
                requestor: request.requestor,
                selection: request.selection,
                target: request.target,
                property,
            },
        )?
        .check()?;
    Ok(())
}

struct Transfer<'a> {
    server: &'a Server,
    contents: &'a Contents,
    timestamp: u32,
    requestor: Window,
}

impl Transfer<'_> {
    fn write(&self, target: Atom, property: Atom) -> Result<bool> {
        let Server { conn, atoms, .. } = self.server;
        if target == atoms.TARGETS {
            let formats = [
                atoms.TARGETS,
                atoms.MULTIPLE,
                atoms.TIMESTAMP,
                atoms.SAVE_TARGETS,
                atoms.UTF8_STRING,
                atoms.TEXT,
                atoms.MIME_TEXT,
                atoms.MIME_URI_LIST,
                atoms.GNOME_FILES,
            ];
            conn.change_property32(
                PropMode::REPLACE,
                self.requestor,
                property,
                AtomEnum::ATOM,
                &formats,
            )?
            .check()?;
        } else if target == atoms.TIMESTAMP {
            conn.change_property32(
                PropMode::REPLACE,
                self.requestor,
                property,
                AtomEnum::INTEGER,
                &[self.timestamp],
            )?
            .check()?;
        } else {
            let (kind, bytes) = if target == atoms.MIME_URI_LIST {
                (target, self.contents.uris.as_bytes())
            } else if target == atoms.GNOME_FILES {
                (target, self.contents.desktop_files.as_bytes())
            } else if target == atoms.TEXT {
                (atoms.UTF8_STRING, self.contents.text.as_bytes())
            } else if target == atoms.UTF8_STRING || target == atoms.MIME_TEXT {
                (target, self.contents.text.as_bytes())
            } else if target == atoms.SAVE_TARGETS {
                (atoms.NULL, &[][..])
            } else {
                return Ok(false);
            };
            self.write_bytes(property, kind, bytes)?;
        }
        Ok(true)
    }

    fn write_bytes(&self, property: Atom, kind: Atom, bytes: &[u8]) -> Result<()> {
        let conn = &self.server.conn;
        // Assemble large lists in consecutive requests. Announce the property
        // only after the complete value is available to the requestor.
        let bytes_per_request = conn.maximum_request_bytes().saturating_sub(32).max(1);
        let mut chunks = bytes.chunks(bytes_per_request);
        conn.change_property8(
            PropMode::REPLACE,
            self.requestor,
            property,
            kind,
            chunks.next().unwrap_or_default(),
        )?
        .check()?;
        for chunk in chunks {
            conn.change_property8(PropMode::APPEND, self.requestor, property, kind, chunk)?
                .check()?;
        }
        Ok(())
    }

    fn write_multiple(&self, property: Atom) -> Result<bool> {
        let Server { conn, atoms, .. } = self.server;
        let reply = conn
            .get_property(false, self.requestor, property, atoms.ATOM_PAIR, 0, 256)?
            .reply()?;
        let Some(values) = reply.value32() else {
            return Ok(false);
        };
        let mut pairs = values.collect::<Vec<_>>();
        let (pairs_mut, remainder) = pairs.as_chunks_mut::<2>();
        if reply.type_ != atoms.ATOM_PAIR || reply.bytes_after != 0 || !remainder.is_empty() {
            return Ok(false);
        }
        for pair in pairs_mut {
            if pair[1] == NONE
                || pair[1] == property
                || !self.write(pair[0], pair[1]).unwrap_or(false)
            {
                pair[1] = NONE;
            }
        }
        conn.change_property32(
            PropMode::REPLACE,
            self.requestor,
            property,
            atoms.ATOM_PAIR,
            &pairs,
        )?
        .check()?;
        Ok(true)
    }
}
