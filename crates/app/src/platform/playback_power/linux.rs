use riina_core::playback_power::Mode;
use std::{collections::HashMap, sync::mpsc, time::Duration};
use zbus::{Connection, OwnedFd, OwnedObjectPath, Proxy, Variant, connection::Builder};

const TIMEOUT: Duration = Duration::from_secs(2);
const REASON: &str = "Riina playback is in progress";

#[derive(Default)]
pub(crate) struct Controller(Option<mpsc::Sender<Mode>>);

impl Controller {
    pub(crate) fn set(&mut self, mode: Mode) {
        if self
            .0
            .as_ref()
            .is_some_and(|worker| worker.send(mode).is_ok())
        {
            return;
        }
        if mode == Mode::Idle {
            self.0 = None;
            return;
        }
        let (sender, receiver) = mpsc::channel();
        if let Err(error) = sender.send(mode) {
            crate::diagnostics::warn!("Could not queue playback power request: {error}");
            return;
        }
        match crate::background::spawn_named("playback-power", move || run(&receiver)) {
            Ok(_) => self.0 = Some(sender),
            Err(error) => {
                crate::diagnostics::warn!("Could not start playback power request: {error}")
            }
        }
    }
}

fn run(requests: &mpsc::Receiver<Mode>) {
    let mut lease = Lease::default();
    while let Ok(mode) = requests.recv() {
        lease.set(requests.try_iter().last().unwrap_or(mode));
    }
}

#[derive(Default)]
struct Lease {
    mode: Mode,
    system: Option<OwnedFd>,
    display: Option<Display>,
}

impl Lease {
    fn set(&mut self, mode: Mode) {
        if self.mode == mode {
            return;
        }
        if mode == Mode::Idle {
            self.display = None;
            self.system = None;
        } else {
            if self.system.is_none() {
                self.system = zbus::block_on(inhibit_system()).ok();
            }
            self.display = (mode == Mode::Display)
                .then(|| zbus::block_on(inhibit_display()))
                .and_then(Result::ok);
            if self.system.is_none() && self.display.is_none() {
                crate::diagnostics::warn!(
                    "This desktop did not accept Riina's playback power request"
                );
            }
        }
        self.mode = mode;
    }
}

async fn inhibit_system() -> zbus::Result<OwnedFd> {
    let connection = Builder::system().method_timeout(TIMEOUT).build().await?;
    let proxy = Proxy::new(
        &connection,
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
    )
    .await?;
    // The idle action is the equivalent of automatic system sleep. A user's
    // explicit suspend request still follows the desktop's ordinary policy.
    proxy
        .call("Inhibit", &("idle", "Riina", REASON, "block"))
        .await
}

enum Display {
    ScreenSaver {
        connection: Connection,
        cookie: u32,
    },
    Portal {
        connection: Connection,
        handle: OwnedObjectPath,
    },
}

async fn inhibit_display() -> zbus::Result<Display> {
    let connection = Builder::session().method_timeout(TIMEOUT).build().await?;
    let screensaver = Proxy::new(
        &connection,
        "org.freedesktop.ScreenSaver",
        "/org/freedesktop/ScreenSaver",
        "org.freedesktop.ScreenSaver",
    )
    .await;
    if let Ok(screensaver) = screensaver
        && let Ok(cookie) = screensaver.call("Inhibit", &("Riina", REASON)).await
    {
        return Ok(Display::ScreenSaver { connection, cookie });
    }
    let portal = Proxy::new(
        &connection,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.Inhibit",
    )
    .await?;
    let options = HashMap::from([("reason", Variant::from(REASON))]);
    let handle = portal.call("Inhibit", &("", 8_u32, options)).await?;
    Ok(Display::Portal { connection, handle })
}

impl Drop for Display {
    fn drop(&mut self) {
        zbus::block_on(async {
            match self {
                Self::ScreenSaver { connection, cookie } => {
                    if let Ok(proxy) = Proxy::new(
                        connection,
                        "org.freedesktop.ScreenSaver",
                        "/org/freedesktop/ScreenSaver",
                        "org.freedesktop.ScreenSaver",
                    )
                    .await
                    {
                        let _ = proxy.call::<_, _, ()>("UnInhibit", &(*cookie,)).await;
                    }
                }
                Self::Portal { connection, handle } => {
                    if let Ok(proxy) = Proxy::new(
                        connection,
                        "org.freedesktop.portal.Desktop",
                        handle.as_str(),
                        "org.freedesktop.portal.Request",
                    )
                    .await
                    {
                        let _ = proxy.call::<_, _, ()>("Close", &()).await;
                    }
                }
            }
        });
    }
}
