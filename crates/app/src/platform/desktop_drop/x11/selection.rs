//! Read a bounded selection using one requestor window for each offer.
use super::{Result, protocol::Atoms};
use std::{
    ffi::OsString,
    sync::Arc,
    time::{Duration, Instant},
};
use x11rb::{
    NONE,
    connection::Connection,
    protocol::{
        Event,
        xproto::{
            Atom, AtomEnum, ConnectionExt as _, CreateWindowAux, EventMask, GetPropertyReply,
            Property, Window, WindowClass,
        },
    },
    rust_connection::RustConnection,
};

const LIMIT: usize = 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(4);

#[derive(Clone, Copy)]
pub(super) struct Mime {
    atom: Atom,
    plain_paths: bool,
}

impl Mime {
    pub(super) fn choose(atoms: &Atoms, types: &[Atom]) -> Option<Self> {
        [
            atoms.URI_LIST,
            atoms.UTF8_TEXT,
            atoms.UTF8_STRING,
            atoms.TEXT,
        ]
        .into_iter()
        .find(|atom| types.contains(atom))
        .map(|atom| Self {
            atom,
            plain_paths: atom != atoms.URI_LIST,
        })
    }

    fn decode(self, bytes: &[u8]) -> Vec<OsString> {
        std::str::from_utf8(bytes).map_or_else(
            |_| Vec::new(),
            |text| super::super::parse_references(text, self.plain_paths),
        )
    }
}

pub(super) struct Transfer {
    conn: Arc<RustConnection>,
    window: Window,
    mime: Mime,
    deadline: Instant,
    incremental: Option<Vec<u8>>,
}

impl Transfer {
    pub(super) fn start(
        conn: Arc<RustConnection>,
        root: Window,
        atoms: &Atoms,
        mime: Mime,
        timestamp: u32,
    ) -> Result<Self> {
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
            &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )?
        .check()?;
        let result = Self {
            conn,
            window,
            mime,
            deadline: Instant::now() + TIMEOUT,
            incremental: None,
        };
        result
            .conn
            .convert_selection(
                window,
                atoms.XdndSelection,
                mime.atom,
                atoms.RIINA_DROP,
                timestamp,
            )?
            .check()?;
        Ok(result)
    }

    pub(super) fn deadline(&self) -> Instant {
        self.deadline
    }

    pub(super) fn receive(
        &mut self,
        atoms: &Atoms,
        event: &Event,
    ) -> Result<Option<Vec<OsString>>> {
        match *event {
            Event::SelectionNotify(reply)
                if reply.requestor == self.window
                    && reply.selection == atoms.XdndSelection
                    && reply.target == self.mime.atom =>
            {
                if reply.property != atoms.RIINA_DROP || self.incremental.is_some() {
                    return Ok(Some(Vec::new()));
                }
                let data = self.property(atoms)?;
                if data.type_ != atoms.INCR {
                    return Ok(Some(self.mime.decode(as_bytes(&data)?)));
                }
                let size = data
                    .value32()
                    .and_then(|mut size| size.next())
                    .ok_or("Invalid incremental X11 selection")?;
                if data.value_len != 1 || size as usize > LIMIT {
                    return Err("The X11 drop exceeds the payload limit".into());
                }
                self.incremental = Some(Vec::new());
            }
            Event::PropertyNotify(event)
                if event.window == self.window
                    && event.atom == atoms.RIINA_DROP
                    && event.state == Property::NEW_VALUE
                    && self.incremental.is_some() =>
            {
                let data = self.property(atoms)?;
                let block = as_bytes(&data)?;
                if block.is_empty() {
                    return Ok(self
                        .incremental
                        .take()
                        .map(|bytes| self.mime.decode(&bytes)));
                }
                if let Some(bytes) = self.incremental.as_mut() {
                    if bytes.len() + block.len() > LIMIT {
                        return Err("The X11 drop exceeds the payload limit".into());
                    }
                    bytes.extend_from_slice(block);
                }
            }
            _ => {}
        }
        Ok(None)
    }

    fn property(&self, atoms: &Atoms) -> Result<GetPropertyReply> {
        let data = self
            .conn
            .get_property(
                true,
                self.window,
                atoms.RIINA_DROP,
                AtomEnum::ANY,
                0,
                (LIMIT / 4) as u32,
            )?
            .reply()?;
        if data.bytes_after != 0 || data.type_ == NONE {
            return Err("The X11 selection is missing or too large".into());
        }
        Ok(data)
    }
}

impl Drop for Transfer {
    fn drop(&mut self) {
        // This XID cannot deliver an obsolete conversion to the next drag.
        let _ = self.conn.destroy_window(self.window);
    }
}

fn as_bytes(data: &GetPropertyReply) -> Result<&[u8]> {
    if data.format != 8 {
        return Err("The X11 selection is not a byte stream".into());
    }
    Ok(&data.value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_lists_and_plain_text_share_the_desktop_reference_rules() {
        let bytes = b"# comment\r\nfile:///tmp/film%20%CE%B1.mkv\nhttps://example.test/media\n/tmp/raw.mkv\n";
        assert_eq!(
            Mime {
                atom: 1,
                plain_paths: false
            }
            .decode(bytes),
            ["/tmp/film α.mkv", "https://example.test/media"]
        );
        assert_eq!(
            Mime {
                atom: 1,
                plain_paths: true
            }
            .decode(bytes),
            [
                "/tmp/film α.mkv",
                "https://example.test/media",
                "/tmp/raw.mkv"
            ]
        );
        assert!(
            Mime {
                atom: 1,
                plain_paths: true
            }
            .decode(b"\xff")
            .is_empty()
        );
    }
}
