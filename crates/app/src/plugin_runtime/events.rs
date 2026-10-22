//! Player event subscriptions and their bounded native observation requests.
use super::GlobalApiMode;
use rquickjs::{Ctx, Object, Value, function::Func};
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

const PLUGIN_EVENT_API: &str = r#"(() => {
    const listeners = new Map();
    let nextId = 0;
    let listenerCount = 0;
    const validName = (name) => {
        if (typeof name !== "string") return false;
        const parts = name.split(".");
        return (parts.length === 2 || (parts.length === 3 && parts[2] === "changed"))
            && (parts[0] === "iina" || parts[0] === "mpv")
            && parts[1].length > 0
            && parts.every((part) => part.length > 0);
    };
    iina.event = {
        on(name, callback) {
            if (!validName(name)) throw new Error(`Incorrect event name syntax: ${String(name)}`);
            if (typeof callback !== "function") throw new TypeError("Event listener must be a function");
            if (listenerCount >= 1024) throw new Error("Plugin has reached the 1024 event listener limit");
            let entries = listeners.get(name);
            if (!entries) {
                iina.__subscribePlayerEvent(name);
                entries = new Map();
                listeners.set(name, entries);
            }
            const id = String(++nextId);
            entries.set(id, callback);
            listenerCount += 1;
            return id;
        },
        off(name, id) {
            const entries = listeners.get(name);
            if (!entries || !entries.delete(id)) {
                iina.log(`Event listener not found for id ${String(id)}`);
                return;
            }
            listenerCount -= 1;
            if (entries.size === 0) {
                listeners.delete(name);
                iina.__unsubscribePlayerEvent(name);
            }
        },
        __dispatch(name, data) {
            for (const callback of [...(listeners.get(name)?.values() || [])]) {
                try { callback(data); }
                catch (error) { iina.log(`Event callback failed for ${name}: ${String(error)}`); }
            }
        }
    };
})();"#;

pub(super) fn install<'js>(
    ctx: Ctx<'js>,
    api: Object<'js>,
    global_api_mode: GlobalApiMode,
    player_event_subscriptions: Arc<Mutex<BTreeSet<String>>>,
    observed_mpv_properties: Arc<Mutex<BTreeSet<String>>>,
    mpv_property_observation_requests: Arc<Mutex<BTreeSet<String>>>,
) -> rquickjs::Result<()> {
    if global_api_mode != GlobalApiMode::Controller {
        let subscriptions = player_event_subscriptions.clone();
        let observed_properties = observed_mpv_properties.clone();
        let observation_requests = mpv_property_observation_requests.clone();
        api.set(
            "__subscribePlayerEvent",
            Func::from(move |name: String| -> rquickjs::Result<()> {
                validate_player_event_name(&name).map_err(|message| {
                    rquickjs::Error::new_from_js_message(
                        "event name",
                        "iina.* or mpv.* event",
                        message,
                    )
                })?;
                let mut subscriptions = subscriptions.lock().map_err(|_| {
                    rquickjs::Error::new_from_js_message(
                        "event listener",
                        "available subscription registry",
                        "Plugin event subscriptions were poisoned",
                    )
                })?;
                if let Some(property) = mpv_property_for_changed_event(&name)
                    && !player_snapshot_mpv_property(property)
                {
                    let mut observed = observed_properties.lock().map_err(|_| {
                        rquickjs::Error::new_from_js_message(
                            "mpv property",
                            "available observation registry",
                            "Plugin property observations were poisoned",
                        )
                    })?;
                    if !observed.contains(property)
                        && observed.len() >= riina_core::MAX_PLUGIN_OBSERVED_MPV_PROPERTIES
                    {
                        return Err(rquickjs::Error::new_from_js_message(
                            "mpv property",
                            "at most 128 observed properties per plugin",
                            "Plugin has reached the 128 observed mpv property limit",
                        ));
                    }
                    if observed.insert(property.to_owned()) {
                        observation_requests
                            .lock()
                            .map_err(|_| {
                                rquickjs::Error::new_from_js_message(
                                    "mpv property",
                                    "available observation request queue",
                                    "Plugin observation request queue was poisoned",
                                )
                            })?
                            .insert(property.to_owned());
                    }
                }
                subscriptions.insert(name);
                Ok(())
            }),
        )?;
        let subscriptions = player_event_subscriptions.clone();
        api.set(
            "__unsubscribePlayerEvent",
            Func::from(move |name: String| {
                if let Ok(mut subscriptions) = subscriptions.lock() {
                    subscriptions.remove(&name);
                }
            }),
        )?;
        ctx.eval::<Value, _>(PLUGIN_EVENT_API)?;
    }
    Ok(())
}

pub(super) fn validate_player_event_name(name: &str) -> Result<(), String> {
    let parts = name.split('.').collect::<Vec<_>>();
    let valid_shape = parts.len() == 2 || (parts.len() == 3 && parts[2] == "changed");
    if !valid_shape
        || !matches!(parts[0], "iina" | "mpv")
        || parts[1].is_empty()
        || parts.iter().any(|part| part.is_empty())
        || name.len() > 256
        || name.chars().any(char::is_control)
    {
        return Err(format!("Incorrect event name syntax: \"{name}\""));
    }
    Ok(())
}

fn mpv_property_for_changed_event(name: &str) -> Option<&str> {
    let property = name.strip_prefix("mpv.")?.strip_suffix(".changed")?;
    (!property.is_empty() && !property.contains('.')).then_some(property)
}

fn player_snapshot_mpv_property(name: &str) -> bool {
    matches!(
        name,
        "pause"
            | "idle-active"
            | "time-pos"
            | "duration"
            | "speed"
            | "width"
            | "height"
            | "media-title"
            | "aid"
            | "audio-delay"
            | "volume"
            | "mute"
            | "sid"
            | "secondary-sid"
            | "sub-delay"
    )
}
