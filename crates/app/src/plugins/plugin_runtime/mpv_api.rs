//! Synchronous JavaScript getters backed by asynchronous requests to this player's engine.
use super::execution::ExecutionControl;
use riina_mpv::{CommandSender, MpvPropertyFormat};
use rquickjs::{Ctx, Object, Value, function::Func};
use std::{rc::Rc, sync::Arc};

struct Queries {
    player: Option<CommandSender>,
    execution: Arc<ExecutionControl>,
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
    iina.set("mpv", api)
}
