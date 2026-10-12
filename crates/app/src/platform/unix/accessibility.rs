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
    if let Ok(mut watcher) = watcher.lock()
        && watcher.is_none()
    {
        *watcher = thread::Builder::new()
            .name("riina-xdg-accessibility".into())
            .spawn(watch_settings)
            .ok();
    }
    REDUCED_MOTION.load(Ordering::Relaxed)
}

fn watch_settings() {
    loop {
        if watch_portal().is_err() {
            thread::sleep(Duration::from_secs(3));
        }
    }
}

fn watch_portal() -> Result<(), zbus::Error> {
    use zbus::{
        blocking::{Connection, Proxy},
        zvariant::OwnedValue,
    };

    let connection = Connection::session()?;
    let proxy = Proxy::new(
        &connection,
        PORTAL_DESTINATION,
        PORTAL_PATH,
        PORTAL_INTERFACE,
    )?;
    // Subscribe first so a system change cannot land between the initial read
    // and installing the live update stream.
    let mut signals = proxy.receive_signal("SettingChanged")?;
    if let Ok(enabled) = read_reduced_motion(&proxy) {
        REDUCED_MOTION.store(enabled, Ordering::Relaxed);
    }

    for message in &mut signals {
        let Ok((namespace, key, value)) =
            message.body().deserialize::<(String, String, OwnedValue)>()
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

fn read_reduced_motion(proxy: &zbus::blocking::Proxy<'_>) -> Result<bool, zbus::Error> {
    use zbus::zvariant::OwnedValue;

    let key = (APPEARANCE_NAMESPACE, REDUCED_MOTION_KEY);
    let value: OwnedValue = match proxy.call("ReadOne", &key) {
        Err(zbus::Error::MethodError(name, ..))
            if name.as_str() == "org.freedesktop.DBus.Error.UnknownMethod" =>
        {
            proxy.call("Read", &key)?
        }
        result => result?,
    };
    Ok(value
        .downcast_ref::<u32>()
        .is_ok_and(|setting| setting == 1))
}
