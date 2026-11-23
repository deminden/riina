//! Messages between a plugin's global and player realms.
use super::{
    AvailablePermissions, GlobalApiMode, HostEvent, MAX_PLUGIN_MESSAGE_BYTES, PluginWorkerHost,
    UiEffect,
    core_api::plugin_media_reference,
    output::{record, record_global_event},
};
use crate::{
    plugin_files::PluginFiles,
    plugins::plugin_players::{Message, Options, Target},
};
use rquickjs::{Ctx, Object, function::Func};
use serde_json::Value;
use std::{
    cell::Cell,
    sync::{Arc, Mutex},
};

pub(super) struct Host {
    events: Arc<Mutex<Vec<HostEvent>>>,
    files: PluginFiles,
    permissions: AvailablePermissions,
    allowed_domains: Vec<String>,
    label: Option<String>,
}

impl Host {
    pub(super) fn from_realm(
        host: &PluginWorkerHost,
        permissions: AvailablePermissions,
        allowed_domains: &[String],
    ) -> Option<(GlobalApiMode, Self)> {
        Some((
            host.global_api_mode?,
            Self {
                events: host.events.clone(),
                files: host.file_system.clone(),
                permissions,
                allowed_domains: allowed_domains.to_vec(),
                label: host.player_label.clone(),
            },
        ))
    }
}

pub(super) fn install<'js>(
    ctx: Ctx<'js>,
    api: Object<'js>,
    global_api_mode: GlobalApiMode,
    host: Host,
) -> rquickjs::Result<()> {
    let global_api = Object::new(ctx.clone())?;
    match global_api_mode {
        GlobalApiMode::Controller => {
            let message_events = host.events.clone();
            global_api.set(
                "__postMessage",
                Func::from(
                    move |target: String, name: String, data: String| -> rquickjs::Result<()> {
                        let target =
                            decode_plugin_json(&target).map_err(plugin_message_js_error)?;
                        let Some(target) = message_target(target) else {
                            return Ok(());
                        };
                        post(&message_events, Some(target), name, &data)
                    },
                ),
            )?;
            let next_id = Cell::new(0_u64);
            global_api.set(
                "__createPlayerInstance",
                Func::from(move |wire: String| -> rquickjs::Result<Option<u64>> {
                    let options = match launch_options(&wire, &host) {
                        Ok(options) => options,
                        Err(error) => {
                            record(
                                &host.events,
                                UiEffect::Log(format!("Could not create player: {error}")),
                            );
                            return Ok(None);
                        }
                    };
                    let id = next_id
                        .get()
                        .checked_add(1)
                        .filter(|id| *id <= u64::from(u32::MAX))
                        .ok_or_else(|| {
                            plugin_message_js_error("Player instance IDs exhausted".into())
                        })?;
                    record_global_event(&host.events, HostEvent::CreatePluginPlayer(id, options))
                        .map_err(plugin_message_js_error)?;
                    next_id.set(id);
                    Ok(Some(id))
                }),
            )?;
        }
        GlobalApiMode::Child => {
            global_api.set("__playerLabel", host.label)?;
            global_api.set(
                "__postMessage",
                Func::from(move |name: String, data: String| -> rquickjs::Result<()> {
                    post(&host.events, None, name, &data)
                }),
            )?;
        }
    }
    api.set("global", global_api)?;
    let wrapper = match global_api_mode {
        GlobalApiMode::Controller => {
            r#"(() => {
                const api = iina.global;
                const postMessage = api.__postMessage;
                const createPlayerInstance = api.__createPlayerInstance;
                const parse = JSON.parse;
                const listeners = new Map();
                api.createPlayerInstance = (options = {}) => createPlayerInstance(JSON.stringify(options)) ?? false;
                api.postMessage = (target, name, data) => {
                    if (target === null || typeof target === "number" || typeof target === "string") {
                        if (typeof target !== "number" || Number.isFinite(target)) {
                            postMessage(JSON.stringify(target), name, JSON.stringify(data) ?? 'null');
                        }
                    }
                };
                api.onMessage = (name, callback) => {
                    if (typeof name !== 'string' || name.length === 0) throw new TypeError('message name must be a non-empty string');
                    if (typeof callback !== 'function') throw new TypeError('callback must be a function');
                    listeners.set(name, callback);
                };
                api.__deliverMessage = (name, data, sender) => listeners.get(name)?.(parse(data), sender);
                delete api.__createPlayerInstance;
                delete api.__postMessage;
            })();"#
        }
        GlobalApiMode::Child => {
            r#"(() => {
                const api = iina.global;
                const postMessage = api.__postMessage;
                const label = api.__playerLabel ?? null;
                const parse = JSON.parse;
                const listeners = new Map();
                api.postMessage = (name, data) => postMessage(name, JSON.stringify(data) ?? 'null');
                api.onMessage = (name, callback) => {
                    if (typeof name !== 'string' || name.length === 0) throw new TypeError('message name must be a non-empty string');
                    if (typeof callback !== 'function') throw new TypeError('callback must be a function');
                    listeners.set(name, callback);
                };
                api.getLabel = () => label;
                api.__deliverMessage = (name, data, sender) => listeners.get(name)?.(parse(data), sender);
                delete api.__playerLabel;
                delete api.__postMessage;
            })();"#
        }
    };
    ctx.eval::<(), _>(wrapper)?;
    Ok(())
}

fn launch_options(wire: &str, host: &Host) -> Result<Options, String> {
    if wire.len() > 32 * 1024 {
        return Err("Player options exceed 32 KiB".into());
    }
    let options = decode_plugin_json(wire)?;
    let source = options["url"]
        .as_str()
        .map(|value| {
            plugin_media_reference(
                value,
                &host.files,
                host.permissions.file_system,
                host.permissions.network_request,
                &host.allowed_domains,
            )
        })
        .transpose()?;
    let label = options["label"].as_str().map(str::to_owned);
    if label.as_ref().is_some_and(|label| label.len() > 1024) {
        return Err("Player label exceeds 1 KiB".into());
    }
    Ok(Options {
        source,
        label,
        enable_plugins: options["enablePlugins"] == Value::Bool(true),
        disable_ui: options["disableUI"] == Value::Bool(true),
        disable_window_animation: options["disableWindowAnimation"] == Value::Bool(true),
    })
}

fn message_target(target: Value) -> Option<Target> {
    match target {
        Value::Null => Some(Target::Broadcast),
        Value::String(label) if label.len() <= 1024 => Some(Target::PlayerLabel(label)),
        Value::Number(id) => id
            .as_f64()
            .filter(|id| (1.0..=f64::from(u32::MAX)).contains(id))
            .map(|id| Target::Instance(id as u64)),
        _ => None,
    }
}

fn post(
    events: &Mutex<Vec<HostEvent>>,
    target: Option<Target>,
    name: String,
    data: &str,
) -> rquickjs::Result<()> {
    let data = decode_plugin_json(data).map_err(plugin_message_js_error)?;
    validate_plugin_message(&name, &data).map_err(plugin_message_js_error)?;
    let event = HostEvent::PluginMessage(Message { target, name, data });
    record_global_event(events, event).map_err(plugin_message_js_error)
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
