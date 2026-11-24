//! Local browser-link handoff. Only validated link packets can enter the UI.
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub(crate) use macos::{
    ApplicationEvents, refresh_idle_menu, refresh_idle_plugins, show_idle_menu,
};
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

use crate::browser_links::{Inbox, Link, MAX_LINK_BYTES};
use sha2::{Digest, Sha256};
use std::{io, path::Path, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    runtime::Builder,
    sync::oneshot,
    time::timeout,
};

const DEADLINE: Duration = Duration::from_secs(2);

#[derive(Default)]
pub(crate) struct Activation {
    pub(crate) inbox: Inbox,
    worker: Option<Worker>,
}

impl Activation {
    // None means the request was acknowledged by an existing application.
    pub(crate) fn prepare(profile: &Path, link: Option<&str>) -> io::Result<Option<Self>> {
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
            }
            Err(error) if link.is_none() && transport::occupied(&error) => {}
            Err(error) if transport::occupied(&error) => {
                if let Some(link) = link {
                    runtime.block_on(async {
                        timeout(DEADLINE, async {
                            let stream = endpoint.connect().await?;
                            submit(stream, link).await
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
    if len == 0 || len > MAX_LINK_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid browser-link packet size",
        ));
    }
    let mut bytes = vec![0; len];
    stream.read_exact(&mut bytes).await?;
    let accepted = std::str::from_utf8(&bytes)
        .ok()
        .and_then(|link| Link::parse(link).ok())
        .is_some_and(|link| inbox.push(link));
    stream.write_u8(u8::from(accepted)).await?;
    // Keep a Windows pipe connected until its client has received the reply.
    let _ = stream.read_u8().await;
    Ok(())
}

async fn submit(mut stream: impl AsyncRead + AsyncWrite + Unpin, link: &str) -> io::Result<()> {
    stream.write_u32(link.len() as u32).await?;
    stream.write_all(link.as_bytes()).await?;
    if stream.read_u8().await? == 1 {
        Ok(())
    } else {
        Err(io::Error::other(
            "The running Riina application could not accept the browser link",
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
        let invalid = Activation::prepare(&profile, Some("riina://quit?url=https://example.test"));
        assert!(invalid.is_err());
        let valid = "riina://open?url=https://example.test/movie.mkv";
        assert!(
            Activation::prepare(&profile, Some(valid))
                .unwrap()
                .is_none()
        );
        drop(listener);
        assert!(
            Activation::prepare(&profile, Some(valid))
                .unwrap()
                .is_some()
        );
    }
}
