//! Synchronous JavaScript property operations on this player's engine.
use super::{
    AvailablePermissions,
    core_api::plugin_media_reference,
    execution::ExecutionControl,
    mpv_properties::{Scope, scope},
};
use crate::{diagnostics, plugin_files::PluginFiles};
use riina_mpv::{CommandSender, EngineError, MpvPropertyFormat};
use rquickjs::{Ctx, Object, Value, function::Func};
use std::{rc::Rc, sync::Arc};

struct Queries {
    player: Option<CommandSender>,
    execution: Arc<ExecutionControl>,
}

struct Writes {
    engine: Rc<Queries>,
    files: PluginFiles,
    permissions: AvailablePermissions,
    allowed_domains: Vec<String>,
}

impl Queries {
    fn read(&self, name: &str, format: MpvPropertyFormat) -> serde_json::Value {
        self.player
            .as_ref()
            .zip(self.execution.deadline())
            .and_then(|(player, deadline)| player.read_mpv_property(name, format, deadline).ok())
            .unwrap_or(serde_json::Value::Null)
    }
}

pub(super) fn install<'js>(
    iina: &Object<'js>,
    player: Option<CommandSender>,
    execution: Arc<ExecutionControl>,
    files: &PluginFiles,
    permissions: AvailablePermissions,
    allowed_domains: &[String],
) -> rquickjs::Result<()> {
    let queries = Rc::new(Queries { player, execution });
    let api = Object::new(iina.ctx().clone())?;
    let flags = queries.clone();
    api.set(
        "getFlag",
        Func::from(move |name: String| {
            flags
                .read(&name, MpvPropertyFormat::Flag)
                .as_bool()
                .unwrap_or(false)
        }),
    )?;
    let numbers = queries.clone();
    api.set(
        "getNumber",
        Func::from(move |name: String| {
            numbers
                .read(&name, MpvPropertyFormat::Number)
                .as_f64()
                .unwrap_or(0.0)
        }),
    )?;
    for (method, format) in [
        ("getString", MpvPropertyFormat::Text),
        ("getNative", MpvPropertyFormat::Node),
    ] {
        let queries = queries.clone();
        api.set(
            method,
            Func::from(
                move |ctx: Ctx<'js>, name: String| -> rquickjs::Result<Value<'js>> {
                    ctx.json_parse(queries.read(&name, format).to_string())
                },
            ),
        )?;
    }
    let writes = Writes {
        engine: queries,
        files: files.clone(),
        permissions,
        allowed_domains: allowed_domains.to_vec(),
    };
    api.set(
        "set",
        Func::from(move |ctx: Ctx<'js>, name: String, value: Value<'js>| {
            writes.set(&ctx, &name, value)
        }),
    )?;
    iina.set("mpv", api)
}

impl Writes {
    fn set<'js>(&self, ctx: &Ctx<'js>, name: &str, value: Value<'js>) -> rquickjs::Result<()> {
        let scope = scope(name, self.permissions).map_err(js_error)?;
        let json = json_value(ctx, value)?;
        let json = match scope {
            Scope::Playback => json,
            Scope::FileList => self.resolve_files(json).map_err(js_error)?,
        };
        let (player, deadline) = self
            .engine
            .player
            .as_ref()
            .zip(self.engine.execution.deadline())
            .ok_or_else(|| js_error("mpv.set: the player engine is unavailable".into()))?;
        match player.write_mpv_property(name, json, deadline) {
            Ok(()) => Ok(()),
            Err(error @ EngineError::Native { .. }) => {
                diagnostics::log_args(
                    diagnostics::Level::Warning,
                    module_path!(),
                    format_args!("mpv.set {name}: {error}"),
                );
                Ok(())
            }
            Err(error) => Err(js_error(format!("mpv.set: {name}: {error}"))),
        }
    }

    fn resolve_files(&self, input: serde_json::Value) -> Result<serde_json::Value, String> {
        let paths = match input {
            serde_json::Value::String(path) => vec![serde_json::Value::String(path)],
            serde_json::Value::Array(paths) => paths,
            _ => return Err("mpv.set: expected a path or an array of paths".into()),
        };
        paths
            .iter()
            .map(|item| {
                let path = item.as_str().ok_or("mpv.set: file lists require strings")?;
                if riina_core::media_reference::is_network_url_candidate(path)
                    || url::Url::parse(path).is_ok_and(|url| url.scheme() == "file")
                {
                    plugin_media_reference(
                        path,
                        &self.files,
                        true,
                        self.permissions.network_request,
                        &self.allowed_domains,
                    )
                } else {
                    self.files
                        .existing_path(path, true)
                        .map(|path| path.to_string_lossy().into_owned())
                }
                .map(serde_json::Value::String)
            })
            .collect::<Result<Vec<_>, _>>()
            .map(serde_json::Value::Array)
    }
}

fn json_value<'js>(ctx: &Ctx<'js>, value: Value<'js>) -> rquickjs::Result<serde_json::Value> {
    let structured = value.is_object();
    let json = ctx
        .json_stringify(value)?
        .ok_or_else(|| js_error("mpv.set: expected a number, string, boolean or object".into()))?
        .to_string()?;
    if json.len() > 1024 * 1024 {
        return Err(js_error("mpv.set: the value exceeds 1 MiB".into()));
    }
    let parsed: serde_json::Value =
        serde_json::from_str(&json).map_err(|error| js_error(error.to_string()))?;
    if parsed.is_null() || structured != (parsed.is_array() || parsed.is_object()) {
        return Err(js_error(
            "mpv.set: expected a finite number, string, boolean or object".into(),
        ));
    }
    Ok(parsed)
}

fn js_error(message: String) -> rquickjs::Error {
    rquickjs::Error::new_from_js_message("iina.mpv", "writable player property", message)
}
