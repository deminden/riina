//! Player input registrations and bounded delivery to each owning realm.
#[cfg(test)]
mod tests;

use super::RuntimeCommand;
use rquickjs::{Context, Ctx, Function, Object, Value, function::Func};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, RwLock, Weak,
        mpsc::{self, SyncSender},
    },
    time::{Duration, Instant},
};

pub(crate) const HIGH: i32 = 200;
const INPUT_BUDGET: Duration = Duration::from_millis(16);
const MAX_LISTENERS: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[repr(usize)]
pub(crate) enum Kind {
    KeyDown,
    KeyUp,
    MouseDown,
    MouseUp,
    MouseDrag,
}

impl Kind {
    fn from_index(value: usize) -> Option<Self> {
        [
            Self::KeyDown,
            Self::KeyUp,
            Self::MouseDown,
            Self::MouseUp,
            Self::MouseDrag,
        ]
        .get(value)
        .copied()
    }

    fn key(self, code: &str) -> Option<String> {
        if code.is_empty() || code.len() > 128 || code.chars().any(char::is_control) {
            return None;
        }
        match self {
            Self::KeyDown | Self::KeyUp => crate::key_binding_preferences::plugin_key_binding(code),
            _ => matches!(code, "*mouse" | "*rightMouse" | "*otherMouse").then(|| code.to_owned()),
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct Bindings(Arc<RwLock<Option<String>>>);

impl Bindings {
    pub(crate) fn replace(&self, json: String) {
        if let Ok(mut bindings) = self.0.write() {
            *bindings = Some(json);
        }
    }
}

pub(crate) struct Endpoint {
    listeners: RwLock<BTreeMap<(Kind, String), i32>>,
    commands: SyncSender<RuntimeCommand>,
    bindings: Bindings,
}

impl Endpoint {
    pub(super) fn new(commands: SyncSender<RuntimeCommand>, bindings: Bindings) -> Self {
        Self {
            listeners: RwLock::default(),
            commands,
            bindings,
        }
    }

    fn register(
        &self,
        kind: usize,
        code: &str,
        priority: i32,
        enabled: bool,
    ) -> rquickjs::Result<String> {
        let kind = Kind::from_index(kind).ok_or_else(|| input_error("Unknown input event"))?;
        let code = kind
            .key(code)
            .ok_or_else(|| input_error("Invalid plugin input binding"))?;
        let key = (kind, code.clone());
        let mut listeners = self
            .listeners
            .write()
            .map_err(|_| input_error("Input listener state is unavailable"))?;
        if enabled {
            if listeners.len() >= MAX_LISTENERS && !listeners.contains_key(&key) {
                return Err(input_error(
                    "The plugin has reached its input listener limit",
                ));
            }
            listeners.insert(key, priority);
        } else {
            listeners.remove(&key);
        }
        Ok(code)
    }
}

#[derive(Clone, Default)]
pub(crate) struct Router {
    endpoints: Arc<RwLock<Vec<Weak<Endpoint>>>>,
    pub(crate) bindings: Bindings,
    #[cfg(test)]
    test_budget: Option<Duration>,
}

#[derive(Clone, Copy, Serialize)]
#[serde(untagged)]
pub(crate) enum Arguments {
    #[serde(rename_all = "camelCase")]
    Key { x: f32, y: f32, is_repeat: bool },
    #[serde(rename_all = "camelCase")]
    Mouse {
        x: f32,
        y: f32,
        click_count: u32,
        pressure: f32,
    },
}

struct Input {
    key: (Kind, String),
    arguments: Arguments,
}

impl Router {
    #[cfg(test)]
    pub(crate) fn with_test_budget(mut self, budget: Duration) -> Self {
        self.test_budget = Some(budget);
        self
    }

    pub(crate) fn add(&self, endpoint: &Arc<Endpoint>) {
        if let Ok(mut endpoints) = self.endpoints.write() {
            endpoints.retain(|endpoint| endpoint.strong_count() > 0);
            endpoints.push(Arc::downgrade(endpoint));
        }
    }

    pub(crate) fn begin(&self, kind: Kind, code: String, arguments: Arguments) -> Option<Dispatch> {
        let key = (kind, code);
        let mut listeners = Vec::new();
        for endpoint in self
            .endpoints
            .try_read()
            .ok()?
            .iter()
            .filter_map(Weak::upgrade)
        {
            let priority = endpoint
                .listeners
                .try_read()
                .ok()
                .and_then(|table| table.get(&key).copied());
            if let Some(priority) = priority {
                listeners.push((priority, endpoint));
            }
        }
        if listeners.is_empty() {
            return None;
        }
        listeners.sort_by_key(|(priority, _)| std::cmp::Reverse(*priority));
        let budget = INPUT_BUDGET;
        #[cfg(test)]
        let budget = self.test_budget.unwrap_or(budget);
        Some(Dispatch {
            input: Arc::new(Input { key, arguments }),
            listeners,
            deadline: Instant::now() + budget,
        })
    }
}

pub(crate) struct Dispatch {
    input: Arc<Input>,
    listeners: Vec<(i32, Arc<Endpoint>)>,
    deadline: Instant,
}

impl Dispatch {
    pub(crate) fn handled(&self, before_binding: bool) -> bool {
        for (priority, endpoint) in &self.listeners {
            if (*priority >= HIGH) != before_binding {
                continue;
            }
            let current = Instant::now();
            if current >= self.deadline {
                break;
            }
            let (reply, response) = mpsc::sync_channel(1);
            let request = Request {
                input: self.input.clone(),
                deadline: self.deadline,
                reply,
            };
            if endpoint
                .commands
                .try_send(RuntimeCommand::DeliverInput(request))
                .is_ok()
                && response.recv_timeout(self.deadline.saturating_duration_since(Instant::now()))
                    == Ok(true)
            {
                return true;
            }
        }
        false
    }
}

pub(super) struct Request {
    input: Arc<Input>,
    deadline: Instant,
    reply: SyncSender<bool>,
}

impl Request {
    pub(super) fn dispatch(self, context: &Context) -> Result<(), String> {
        if Instant::now() >= self.deadline {
            return Ok(());
        }
        let result = context.with(|ctx| {
            let deliver: Function<'_> = ctx.globals().get("__riinaPluginInput")?;
            let (kind, key) = &self.input.key;
            let json = serde_json::to_string(&self.input.arguments)
                .map_err(|error| input_error(error.to_string()))?;
            let args: Value<'_> = ctx.json_parse(json)?;
            deliver
                .call::<_, bool>((*kind as usize, key.as_str(), args))
                .inspect_err(|error| {
                    if error.is_exception() {
                        ctx.catch();
                    }
                })
        });
        _ = self
            .reply
            .try_send(result.as_ref().is_ok_and(|handled| *handled));
        result.map(|_| ()).map_err(|error| error.to_string())
    }
}

pub(super) fn install<'js>(api: &Object<'js>, endpoint: Arc<Endpoint>) -> rquickjs::Result<()> {
    let ctx = api.ctx();
    let native = Object::new(ctx.clone())?;
    let register = endpoint.clone();
    native.set(
        "register",
        Func::from(
            move |kind: usize, code: String, priority: i32, enabled: bool| {
                register.register(kind, &code, priority, enabled)
            },
        ),
    )?;
    native.set(
        "normalize",
        Func::from(move |key: String| riina_core::key_bindings::normalize_mpv_key(&key)),
    )?;
    native.set(
        "bindings",
        Func::from(move |ctx: Ctx<'js>| {
            let bindings = endpoint
                .bindings
                .0
                .read()
                .map_err(|_| input_error("Key bindings are unavailable"))?;
            ctx.json_parse(bindings.as_deref().unwrap_or("{}"))
        }),
    )?;
    let setup: Function<'_> = ctx.eval(include_str!("input.js"))?;
    let deliver: Function<'_> = setup.call((api.clone(), native, HIGH))?;
    ctx.globals().set("__riinaPluginInput", deliver)
}

fn input_error(message: impl Into<String>) -> rquickjs::Error {
    rquickjs::Error::new_from_js_message("iina.input", "input binding", message.into())
}
