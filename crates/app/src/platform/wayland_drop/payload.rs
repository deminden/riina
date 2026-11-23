//! Read only the negotiated drag MIME type, off the UI thread.
use std::{
    ffi::OsString,
    io::{self, Read},
    os::{fd::AsFd, unix::net::UnixStream},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
    },
    time::{Duration, Instant},
};
use wayland_client::protocol::wl_data_offer::WlDataOffer;

const MAX_PAYLOAD: usize = 1024 * 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(4);
pub(super) const URI_LIST: &str = "text/uri-list";
pub(super) const TEXT_MIMES: &[&str] = &["text/plain;charset=utf-8", "UTF8_STRING", "text/plain"];

pub(super) struct Result {
    pub(super) serial: u64,
    pub(super) files: Vec<OsString>,
}

pub(super) struct ReadRequest(Arc<AtomicBool>);

impl ReadRequest {
    pub(super) fn start(
        offer: &WlDataOffer,
        mime: &str,
        serial: u64,
        send: Sender<Result>,
    ) -> io::Result<Self> {
        let (reader, writer) = UnixStream::pair()?;
        reader.set_read_timeout(Some(Duration::from_millis(100)))?;
        let cancelled = Arc::new(AtomicBool::new(false));
        let task_cancelled = cancelled.clone();
        let allow_plain_paths = mime != URI_LIST;
        crate::background::spawn_named("wayland-drop-payload", move || {
            let files = read_payload(reader, &task_cancelled)
                .map(|bytes| parse(&bytes, allow_plain_paths))
                .unwrap_or_default();
            if !task_cancelled.load(Ordering::Relaxed)
                && send.send(Result { serial, files }).is_ok()
            {
                let _ = slint::invoke_from_event_loop(|| {});
            }
        })?;
        offer.receive(mime.to_owned(), writer.as_fd());
        Ok(Self(cancelled))
    }
}

impl Drop for ReadRequest {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

fn read_payload(mut reader: UnixStream, cancelled: &AtomicBool) -> io::Result<Vec<u8>> {
    let mut payload = Vec::new();
    let mut block = [0; 8192];
    let deadline = Instant::now() + READ_TIMEOUT;
    while !cancelled.load(Ordering::Relaxed) && Instant::now() < deadline {
        match reader.read(&mut block) {
            Ok(0) => return Ok(payload),
            Ok(size) if payload.len() + size <= MAX_PAYLOAD => {
                payload.extend_from_slice(&block[..size])
            }
            Ok(_) => {
                return Err(io::Error::other(
                    "Wayland drop payload exceeded the size limit",
                ));
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::TimedOut,
        "Wayland drop payload ended or timed out",
    ))
}

fn parse(bytes: &[u8], allow_plain_paths: bool) -> Vec<OsString> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.contains('\0'))
        .filter_map(|line| parse_reference(line, allow_plain_paths))
        .collect()
}

fn parse_reference(line: &str, allow_plain_paths: bool) -> Option<OsString> {
    if allow_plain_paths && Path::new(line).is_absolute() {
        return Some(line.into());
    }
    let uri = url::Url::parse(line).ok()?;
    if uri.scheme() == "file" {
        return uri.to_file_path().ok().map(|path| path.into_os_string());
    }
    riina_core::media_reference::validate_network_url(line)
        .ok()
        .map(OsString::from)
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn uri_list_decodes_local_paths_and_preserves_valid_network_references() {
        assert_eq!(
            parse(b"# files\r\nfile:///tmp/clip%20%CE%B1.mkv\r\nhttps://example.test/video%20one\nfile://other-host/private\n", false),
            ["/tmp/clip α.mkv", "https://example.test/video%20one"],
        );
        assert!(parse(b"/tmp/file.mkv\nrelative.mkv\n", false).is_empty());
        assert_eq!(
            parse(b"/tmp/file.mkv\nrelative.mkv\n", true),
            ["/tmp/file.mkv"]
        );
    }
}
