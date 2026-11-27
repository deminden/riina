//! Follow the desktop's XDG reduced-motion setting without blocking the UI loop.

use std::{
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

const APPEARANCE_NAMESPACE: &str = "org.freedesktop.appearance";
const REDUCED_MOTION_KEY: &str = "reduced-motion";
const PORTAL_DESTINATION: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const PORTAL_INTERFACE: &str = "org.freedesktop.portal.Settings";

static REDUCED_MOTION: AtomicBool = AtomicBool::new(false);
static WATCHER: OnceLock<Mutex<Option<JoinHandle<()>>>> = OnceLock::new();

pub(crate) fn system_reduce_motion_enabled() -> bool {
    let watcher = WATCHER.get_or_init(|| Mutex::new(None));
    let mut watcher = riina_core::sync::lock(watcher);
    if watcher.is_none() {
        *watcher = crate::background::spawn_named("riina-xdg-accessibility", watch_settings).ok();
    }
    REDUCED_MOTION.load(Ordering::Relaxed)
}

fn watch_settings() {
    loop {
        if zbus::block_on(watch_portal()).is_err() {
            thread::sleep(Duration::from_secs(3));
        }
    }
}

async fn watch_portal() -> Result<(), zbus::Error> {
    use futures_util::StreamExt;
    use zbus::{Connection, OwnedVariant, Proxy};

    let connection = Connection::session().await?;
    let proxy = Proxy::new(
        &connection,
        PORTAL_DESTINATION,
        PORTAL_PATH,
        PORTAL_INTERFACE,
    )
    .await?;
    // Subscribe first so a system change cannot land between the initial read
    // and installing the live update stream.
    let mut signals = proxy.receive_signal("SettingChanged").await?;
    if let Ok(enabled) = read_reduced_motion(&proxy).await {
        REDUCED_MOTION.store(enabled, Ordering::Relaxed);
    }

    while let Some(message) = signals.next().await {
        let Ok((namespace, key, value)) = message
            .body()
            .deserialize::<(String, String, OwnedVariant)>()
        else {
            continue;
        };
        if namespace == APPEARANCE_NAMESPACE
            && key == REDUCED_MOTION_KEY
            && let Ok(value) = value.downcast_ref::<u32>()
        {
            REDUCED_MOTION.store(value == 1, Ordering::Relaxed);
        }
    }
    Err(zbus::Error::Failure(
        "XDG Settings signal stream ended".into(),
    ))
}

async fn read_reduced_motion(proxy: &zbus::Proxy<'_>) -> Result<bool, zbus::Error> {
    let key = (APPEARANCE_NAMESPACE, REDUCED_MOTION_KEY);
    let value: zbus::OwnedVariant = match proxy.call("ReadOne", &key).await {
        Err(zbus::Error::MethodError(name, ..))
            if name.as_str() == "org.freedesktop.DBus.Error.UnknownMethod" =>
        {
            proxy.call("Read", &key).await?
        }
        result => result?,
    };
    Ok(value
        .downcast_ref::<u32>()
        .is_ok_and(|setting| setting == 1))
}
