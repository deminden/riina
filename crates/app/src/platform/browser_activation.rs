//! Deliver browser links and desktop file selections to the running application.
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub(crate) use macos::{
    ApplicationEvents, refresh_idle_menu, refresh_idle_plugins, show_idle_menu,
};
#[cfg(not(target_os = "macos"))]
pub(crate) fn refresh_idle_menu() {}
#[cfg(any(target_os = "macos", test))]
mod playback_menu_options;
#[cfg(test)]
mod playback_menu_tests;
#[cfg(unix)]
#[path = "browser_activation/unix.rs"]
mod transport;
#[cfg(windows)]
#[path = "browser_activation/windows.rs"]
mod transport;

use crate::browser_links::{Delivery, Inbox, Link, MAX_LINK_BYTES};
use sha2::{Digest, Sha256};
use std::{borrow::Cow, io, path::Path, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    runtime::Builder,
    sync::oneshot,
    time::timeout,
};

const DEADLINE: Duration = Duration::from_secs(2);
#[cfg(target_os = "linux")]
const MAX_DOCUMENT_BYTES: usize = 256 * 1024;
#[cfg(target_os = "linux")]
const MAX_PACKET_BYTES: usize = MAX_DOCUMENT_BYTES;
#[cfg(not(target_os = "linux"))]
const MAX_PACKET_BYTES: usize = MAX_LINK_BYTES;

pub(crate) enum Request {
    Browser(String),
    #[cfg(target_os = "linux")]
    Documents(Vec<String>),
}

impl Request {
    #[cfg(target_os = "linux")]
    pub(crate) const DESKTOP_OPEN: &str = "--desktop-open";

    #[cfg(target_os = "linux")]
    pub(crate) fn documents(
        args: impl Iterator<Item = std::ffi::OsString>,
    ) -> Result<Option<Self>, String> {
        let paths = args
            .map(|path| {
                path.into_string()
                    .map_err(|_| "A selected filename is not valid Unicode")
            })
            .collect::<Result<Vec<_>, _>>()?;
        if paths.is_empty() {
            return Ok(None);
        }
        validate_documents(&paths).map_err(|error| error.to_string())?;
        Ok(Some(Self::Documents(paths)))
    }

    fn packet(&self) -> io::Result<Cow<'_, [u8]>> {
        match self {
            Self::Browser(link) if link.len() <= MAX_LINK_BYTES => Ok(link.as_bytes().into()),
            Self::Browser(_) => Err(invalid_packet("The browser link is too large")),
            #[cfg(target_os = "linux")]
            Self::Documents(paths) => {
                let bytes = serde_json::to_vec(paths)?;
                if bytes.len() > MAX_DOCUMENT_BYTES {
                    return Err(invalid_packet("The desktop file selection is too large"));
                }
                Ok(bytes.into())
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn validate_documents(paths: &[String]) -> io::Result<()> {
    if paths.is_empty()
        || !paths
            .iter()
            .all(|path| Path::new(path).is_absolute() && !path.contains('\0'))
    {
        return Err(invalid_packet(
            "Desktop file selections need absolute local paths",
        ));
    }
    Ok(())
}

fn invalid_packet(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn delivery(bytes: &[u8]) -> Option<Delivery> {
    #[cfg(target_os = "linux")]
    if bytes.first() == Some(&b'[') {
        let paths: Vec<String> = serde_json::from_slice(bytes).ok()?;
        validate_documents(&paths).ok()?;
        return Some(Delivery::Documents(paths));
    }
    Link::parse(std::str::from_utf8(bytes).ok()?)
        .ok()
        .map(Delivery::Link)
}

#[derive(Default)]
pub(crate) struct Activation {
    pub(crate) inbox: Inbox,
    worker: Option<Worker>,
}

impl Activation {
    // None means the request was acknowledged by an existing application.
    pub(crate) fn prepare(profile: &Path, request: Option<Request>) -> io::Result<Option<Self>> {
        let packet = request.as_ref().map(Request::packet).transpose()?;
        let mut activation = Self::default();
        let endpoint = transport::Endpoint::new(&profile_id(profile))?;
        let runtime = Builder::new_current_thread().enable_all().build()?;
        let listener = {
            let _enter = runtime.enter();
            endpoint.listen()
        };
        match listener {
            Ok(listener) => {
                let (stop, shutdown) = oneshot::channel();
                let inbox = activation.inbox.clone();
                let handle = crate::background::spawn_named("browser-links", move || {
                    runtime.block_on(async move {
                        tokio::select! {
                            result = listener.serve(&inbox) => {
                                if let Err(error) = result {
                                    crate::diagnostics::warn!("Browser-link listener closed: {error}");
                                }
                            }
                            _ = shutdown => {}
                        }
                    });
                })?;
                activation.worker = Some(Worker {
                    stop: Some(stop),
                    handle: Some(handle),
                });
                #[cfg(target_os = "linux")]
                if let Some(Request::Documents(paths)) = request {
                    activation.inbox.push(Delivery::Documents(paths));
                }
            }
            Err(error) if transport::occupied(&error) => {
                if let Some(packet) = packet {
                    runtime.block_on(async {
                        timeout(DEADLINE, async {
                            let stream = endpoint.connect().await?;
                            submit(stream, &packet).await
                        })
                        .await?
                    })?;
                    return Ok(None);
                }
            }
            Err(error) => return Err(error),
        }
        Ok(Some(activation))
    }
}

fn profile_id(profile: &Path) -> String {
    let bytes = profile.as_os_str().as_encoded_bytes();
    Sha256::digest(bytes)[..12]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

struct Worker {
    stop: Option<oneshot::Sender<()>>,
    handle: Option<crate::background::TaskHandle<()>>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

async fn accept(mut stream: impl AsyncRead + AsyncWrite + Unpin, inbox: &Inbox) -> io::Result<()> {
    let len = stream.read_u32().await? as usize;
    if len == 0 || len > MAX_PACKET_BYTES {
        return Err(invalid_packet("Invalid activation packet size"));
    }
    let mut bytes = vec![0; len];
    stream.read_exact(&mut bytes).await?;
    let accepted = delivery(&bytes).is_some_and(|delivery| inbox.push(delivery));
    stream.write_u8(u8::from(accepted)).await?;
    // Keep a Windows pipe connected until its client has received the reply.
    let _ = stream.read_u8().await;
    Ok(())
}

async fn submit(mut stream: impl AsyncRead + AsyncWrite + Unpin, packet: &[u8]) -> io::Result<()> {
    stream.write_u32(packet.len() as u32).await?;
    stream.write_all(packet).await?;
    if stream.read_u8().await? == 1 {
        Ok(())
    } else {
        Err(io::Error::other(
            "The running Riina application could not accept the open request",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_profile_accepts_only_valid_packets_and_can_be_reclaimed_after_close() {
        let dir = riina_test_support::TempDir::new("browser-listener").unwrap();
        let profile = dir.path().join("preferences.json");
        let listener = Activation::prepare(&profile, None).unwrap().unwrap();
        let browser = |link: &str| Some(Request::Browser(link.into()));
        let invalid =
            Activation::prepare(&profile, browser("riina://quit?url=https://example.test"));
        assert!(invalid.is_err());
        let valid = "riina://open?url=https://example.test/movie.mkv";
        assert!(
            Activation::prepare(&profile, browser(valid))
                .unwrap()
                .is_none()
        );
        drop(listener);
        assert!(
            Activation::prepare(&profile, browser(valid))
                .unwrap()
                .is_some()
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn desktop_activation_delivers_one_ordered_batch_of_local_paths() {
        let dir = riina_test_support::TempDir::new("desktop-open-listener").unwrap();
        let profile = dir.path().join("preferences.json");
        let _listener = Activation::prepare(&profile, None).unwrap().unwrap();
        let paths = ["/media/second file.avi", "/media/--pause\n日本語.avi"];
        let request = Request::documents(paths.into_iter().map(Into::into))
            .unwrap()
            .unwrap();
        assert!(matches!(
            delivery(&request.packet().unwrap()),
            Some(Delivery::Documents(decoded)) if decoded == paths
        ));
        assert!(
            Activation::prepare(&profile, Some(request))
                .unwrap()
                .is_none()
        );
        for path in [
            "--mpv-script=other.lua",
            "https://example.test/clip.avi",
            "/media/\0.avi",
        ] {
            assert!(Request::documents(std::iter::once(path.into())).is_err());
            assert!(
                Activation::prepare(&profile, Some(Request::Documents(vec![path.into()]))).is_err()
            );
        }
    }
}
