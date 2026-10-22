//! Messages between a plugin's global and player realms.
use super::{GlobalApiMode, HostEvent, MAX_PLUGIN_MESSAGE_BYTES, output::record_plugin_message};
use rquickjs::{Ctx, Object, function::Func};
use std::sync::{Arc, Mutex};

pub(super) fn install<'js>(
    ctx: Ctx<'js>,
    api: Object<'js>,
    global_api_mode: GlobalApiMode,
    events: Arc<Mutex<Vec<HostEvent>>>,
) -> rquickjs::Result<()> {
    if global_api_mode != GlobalApiMode::None {
        let global_api = Object::new(ctx.clone())?;
        match global_api_mode {
            GlobalApiMode::Controller => {
                let message_events = events.clone();
                global_api.set(
                "__postMessage",
                Func::from(
                    move |target: String, name: String, data: String| -> rquickjs::Result<()> {
                        let target: serde_json::Value =
                            decode_plugin_json(&target).map_err(plugin_message_js_error)?;
                        if !target.is_null() {
                            return Err(plugin_message_js_error(
                                "Only broadcast messages are available without plugin-created player instances".into(),
                            ));
                        }
                        let data: serde_json::Value =
                            decode_plugin_json(&data).map_err(plugin_message_js_error)?;
                        record_plugin_message(&message_events, name, data)
                            .map_err(plugin_message_js_error)
                    },
                ),
            )?;
            }
            GlobalApiMode::Child => {
                let message_events = events.clone();
                global_api.set(
                    "__postMessage",
                    Func::from(move |name: String, data: String| -> rquickjs::Result<()> {
                        let data: serde_json::Value =
                            decode_plugin_json(&data).map_err(plugin_message_js_error)?;
                        record_plugin_message(&message_events, name, data)
                            .map_err(plugin_message_js_error)
                    }),
                )?;
            }
            GlobalApiMode::None => unreachable!(),
        }
        api.set("global", global_api)?;
        let wrapper = match global_api_mode {
            GlobalApiMode::Controller => {
                r#"(() => {
                const api = iina.global;
                const postMessage = api.__postMessage;
                const listeners = new Map();
                api.postMessage = (target, name, data) => postMessage(JSON.stringify(target), name, JSON.stringify(data));
                api.onMessage = (name, callback) => {
                    if (typeof name !== 'string' || name.length === 0) throw new TypeError('message name must be a non-empty string');
                    if (typeof callback !== 'function') throw new TypeError('callback must be a function');
                    listeners.set(name, [callback]);
                };
                api.__deliverMessage = (name, data) => {
                    for (const callback of listeners.get(name) || []) callback(data, null);
                };
                delete api.__postMessage;
            })();"#
            }
            GlobalApiMode::Child => {
                r#"(() => {
                const api = iina.global;
                const postMessage = api.__postMessage;
                const listeners = new Map();
                api.postMessage = (name, data) => postMessage(name, JSON.stringify(data));
                api.onMessage = (name, callback) => {
                    if (typeof name !== 'string' || name.length === 0) throw new TypeError('message name must be a non-empty string');
                    if (typeof callback !== 'function') throw new TypeError('callback must be a function');
                    listeners.set(name, [callback]);
                };
                api.getLabel = () => null;
                api.__deliverMessage = (name, data) => {
                    for (const callback of listeners.get(name) || []) callback(data, null);
                };
                delete api.__postMessage;
            })();"#
            }
            GlobalApiMode::None => unreachable!(),
        };
        ctx.eval::<(), _>(wrapper)?;
    }
    Ok(())
}

pub(super) fn validate_plugin_message(name: &str, data: &serde_json::Value) -> Result<(), String> {
    if name.is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
        return Err("Plugin message name is invalid or exceeds 256 bytes".into());
    }
    let data_bytes = serde_json::to_vec(data)
        .map_err(|error| format!("Plugin message is not valid JSON: {error}"))?;
    if data_bytes.len() > MAX_PLUGIN_MESSAGE_BYTES {
        return Err("Plugin message exceeds the 1 MiB limit".into());
    }
    Ok(())
}

fn decode_plugin_json(text: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(text).map_err(|error| format!("Plugin message is not valid JSON: {error}"))
}

fn plugin_message_js_error(message: String) -> rquickjs::Error {
    rquickjs::Error::new_from_js_message("plugin message", "valid bounded message", message)
}
