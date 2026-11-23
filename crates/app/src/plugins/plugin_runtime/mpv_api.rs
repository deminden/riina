//! Synchronous JavaScript operations on this player's engine.
use super::{
    AvailablePermissions, HostEvent, UiEffect,
    execution::ExecutionControl,
    mpv_commands::{self, Action, scoped_media_reference},
    mpv_properties::{Scope, scope},
    output::try_record_ui_effect,
};
use crate::{diagnostics, plugin_files::PluginFiles};
use riina_core::{MAX_PLUGIN_MPV_ARGUMENT_BYTES, MAX_PLUGIN_MPV_ARGUMENTS, plugin::WindowAction};
use riina_mpv::{CommandSender, EngineError, MpvPropertyFormat};
use rquickjs::{
    Array, Ctx, Object, Value,
    function::{Func, Opt},
};
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
    time::Instant,
};

struct Queries {
    player: Option<CommandSender>,
    execution: Arc<ExecutionControl>,
}

struct Writes {
    engine: Rc<Queries>,
    files: PluginFiles,
    permissions: AvailablePermissions,
    allowed_domains: Vec<String>,
    events: Arc<Mutex<Vec<HostEvent>>>,
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
    events: &Arc<Mutex<Vec<HostEvent>>>,
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
    let writes = Rc::new(Writes {
        engine: queries,
        files: files.clone(),
        permissions,
        allowed_domains: allowed_domains.to_vec(),
        events: events.clone(),
    });
    let properties = writes.clone();
    api.set(
        "set",
        Func::from(move |ctx: Ctx<'js>, name: String, value: Value<'js>| {
            properties.set(&name, json_value(&ctx, value)?)
        }),
    )?;
    api.set(
        "command",
        Func::from(move |name: String, args: Opt<Array<'js>>| {
            writes.command(&name, read_arguments(args.0)?)
        }),
    )?;
    iina.set("mpv", api)
}

impl Writes {
    fn set(&self, name: &str, json: serde_json::Value) -> rquickjs::Result<()> {
        let scope = scope(name, self.permissions).map_err(js_error)?;
        let json = match scope {
            Scope::Playback => json,
            Scope::FileList => self.resolve_files(json).map_err(js_error)?,
        };
        let (player, deadline) = self.engine()?;
        report_native_reply(
            "mpv.set",
            name,
            player.write_mpv_property(name, json, deadline),
        )
    }

    fn command(&self, name: &str, args: Vec<String>) -> rquickjs::Result<()> {
        match mpv_commands::plan(
            name,
            args,
            &self.files,
            self.permissions,
            &self.allowed_domains,
        )
        .map_err(js_error)?
        {
            Action::Playback(command) => {
                let (player, deadline) = self.engine()?;
                report_native_reply(
                    "mpv.command",
                    name,
                    player.run_plugin_command(command, deadline),
                )
            }
            Action::SetProperty { name, value } => {
                self.set(&name, serde_json::Value::String(value))
            }
            Action::Quit => {
                if try_record_ui_effect(&self.events, UiEffect::WindowAction(WindowAction::Quit)) {
                    Ok(())
                } else {
                    Err(js_error(
                        "mpv.command: the player event queue is full".into(),
                    ))
                }
            }
        }
    }

    fn engine(&self) -> rquickjs::Result<(&CommandSender, Instant)> {
        self.engine
            .player
            .as_ref()
            .zip(self.engine.execution.deadline())
            .ok_or_else(|| js_error("The player engine is unavailable".into()))
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
                scoped_media_reference(path, &self.files, self.permissions, &self.allowed_domains)
                    .map(serde_json::Value::String)
            })
            .collect::<Result<Vec<_>, _>>()
            .map(serde_json::Value::Array)
    }
}

fn read_arguments(args: Option<Array<'_>>) -> rquickjs::Result<Vec<String>> {
    let Some(args) = args else {
        return Ok(Vec::new());
    };
    let count = args.len();
    if count > MAX_PLUGIN_MPV_ARGUMENTS {
        return Err(js_error("mpv.command: too many arguments".into()));
    }
    let mut arguments = Vec::with_capacity(count);
    let mut bytes = 0;
    for index in 0..count {
        let argument = args.get::<String>(index)?;
        bytes += argument.len();
        if bytes > MAX_PLUGIN_MPV_ARGUMENT_BYTES {
            return Err(js_error("mpv.command: arguments exceed 16 KiB".into()));
        }
        arguments.push(argument);
    }
    Ok(arguments)
}

fn report_native_reply(
    api: &str,
    name: &str,
    result: Result<(), EngineError>,
) -> rquickjs::Result<()> {
    match result {
        Ok(()) => Ok(()),
        Err(error @ EngineError::Native { .. }) => {
            diagnostics::log_args(
                diagnostics::Level::Warning,
                module_path!(),
                format_args!("{api} {name}: {error}"),
            );
            Ok(())
        }
        Err(error) => Err(js_error(format!("{api}: {name}: {error}"))),
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
    rquickjs::Error::new_from_js_message("iina.mpv", "valid player operation", message)
}
