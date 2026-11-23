//! Bridge between a plugin realm and its standalone window on the UI thread.
#[cfg(test)]
mod tests;

use super::{ExecutionControl, RuntimeCommand};
use riina_core::plugin_window::{Frame, Intent, Presentation, Properties, State, validate_message};
use rquickjs::{Context, Function, Object, Value, function::Func};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, Weak, mpsc::SyncSender},
};

#[derive(Clone, Default)]
pub(crate) struct Hub(Arc<Mutex<Registry>>);

#[derive(Default)]
struct Registry {
    next_id: u64,
    entries: Vec<Weak<Endpoint>>,
}

impl Hub {
    pub(super) fn register(
        &self,
        name: String,
        root: Option<PathBuf>,
        execution: Arc<ExecutionControl>,
        commands: SyncSender<RuntimeCommand>,
    ) -> Arc<Endpoint> {
        let mut registry = riina_core::sync::lock(&self.0);
        let id = registry.next_id;
        registry.next_id = registry.next_id.wrapping_add(1);
        let endpoint = Arc::new(Endpoint {
            id,
            origin: uuid::Uuid::new_v4(),
            name,
            root,
            execution,
            commands,
            state: Mutex::default(),
        });
        registry.entries.push(Arc::downgrade(&endpoint));
        endpoint
    }

    pub(crate) fn live_entries(&self) -> Vec<Arc<Endpoint>> {
        let mut registry = riina_core::sync::lock(&self.0);
        let mut open = Vec::new();
        registry.entries.retain(|weak| {
            let Some(endpoint) = weak.upgrade().filter(|endpoint| endpoint.is_live()) else {
                return false;
            };
            open.push(endpoint);
            true
        });
        open
    }
}

pub(crate) struct Endpoint {
    pub(crate) id: u64,
    pub(crate) origin: uuid::Uuid,
    pub(crate) name: String,
    root: Option<PathBuf>,
    execution: Arc<ExecutionControl>,
    commands: SyncSender<RuntimeCommand>,
    state: Mutex<State>,
}

impl Endpoint {
    pub(crate) fn package_root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    pub(crate) fn snapshot(&self, seen: Option<u64>) -> Option<Presentation> {
        let state = self.state.try_lock().ok()?;
        let presentation = state.presentation();
        (seen != Some(presentation.revision)).then(|| presentation.clone())
    }

    pub(crate) fn take_messages(&self, document: u64) -> Vec<(String, serde_json::Value)> {
        self.state
            .try_lock()
            .map_or_else(|_| Vec::new(), |mut state| state.take_messages(document))
    }

    pub(crate) fn user_closed(&self) {
        let _ = self.apply(Intent::Close);
    }

    pub(crate) fn presentation_failed(&self, revision: u64) {
        let mut state = riina_core::sync::lock(&self.state);
        if state.presentation().revision == revision {
            let _ = state.apply(Intent::Close);
        }
    }

    pub(crate) fn receive(&self, document: u64, name: String, data: serde_json::Value) {
        // Check the document again when the realm consumes a queued message.
        if self.is_live()
            && self
                .state
                .try_lock()
                .is_ok_and(|state| state.presentation().document_id == document)
            && validate_message(&name, &data).is_ok()
        {
            let _ = self
                .commands
                .try_send(RuntimeCommand::DeliverStandaloneMessage(
                    document, name, data,
                ));
        }
    }

    pub(super) fn dispatch(
        &self,
        context: &Context,
        document: u64,
        name: String,
        data: serde_json::Value,
    ) -> Result<(), String> {
        if riina_core::sync::lock(&self.state)
            .presentation()
            .document_id
            != document
        {
            return Ok(());
        }
        context
            .with(|ctx| {
                let callback: Function<'_> = ctx.globals().get("__riinaStandaloneMessage")?;
                let json =
                    serde_json::to_string(&data).map_err(|error| js_error(error.to_string()))?;
                let argument: Value<'_> = ctx.json_parse(json)?;
                callback
                    .call::<_, ()>((name, argument))
                    .inspect_err(|error| {
                        if error.is_exception() {
                            ctx.catch();
                        }
                    })
            })
            .map_err(|error: rquickjs::Error| error.to_string())
    }

    pub(crate) fn is_live(&self) -> bool {
        !self.execution.is_cancelled()
    }

    fn apply(&self, intent: Intent) -> Result<(), String> {
        if !self.is_live() {
            return Err("The standalone window's plugin has stopped".into());
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| "Standalone window state is unavailable")?;
        state.apply(intent)
    }
}

pub(super) fn install<'js>(api: &Object<'js>, endpoint: &Arc<Endpoint>) -> rquickjs::Result<()> {
    let ctx = api.ctx();
    let native = Object::new(ctx.clone())?;
    for (method, intent) in [
        ("open", Intent::Open),
        ("close", Intent::Close),
        ("simpleMode", Intent::SimpleMode),
    ] {
        let endpoint = endpoint.clone();
        native.set(
            method,
            Func::from(move || endpoint.apply(intent.clone()).map_err(js_error)),
        )?;
    }
    for (method, convert) in [
        ("loadFile", load_file as fn(String) -> Intent),
        ("setStyle", Intent::SetStyle),
        ("setContent", Intent::SetContent),
    ] {
        let endpoint = endpoint.clone();
        native.set(
            method,
            Func::from(move |value: String| endpoint.apply(convert(value)).map_err(js_error)),
        )?;
    }
    let visibility = endpoint.clone();
    native.set(
        "isOpen",
        Func::from(move || {
            riina_core::sync::lock(&visibility.state)
                .presentation()
                .visible
        }),
    )?;
    let properties = endpoint.clone();
    native.set(
        "setProperty",
        Func::from(move |json: String| {
            let value: Properties =
                serde_json::from_str(&json).map_err(|error| js_error(error.to_string()))?;
            properties
                .apply(Intent::SetProperties(value))
                .map_err(js_error)
        }),
    )?;
    let frame = endpoint.clone();
    native.set(
        "setFrame",
        Func::from(move |json: String| {
            let value: Frame =
                serde_json::from_str(&json).map_err(|error| js_error(error.to_string()))?;
            frame.apply(Intent::SetFrame(value)).map_err(js_error)
        }),
    )?;
    let endpoint = endpoint.clone();
    native.set(
        "postMessage",
        Func::from(move |name: String, json: String| {
            let data = serde_json::from_str(&json).map_err(|error| js_error(error.to_string()))?;
            endpoint
                .apply(Intent::PostMessage(name, data))
                .map_err(js_error)
        }),
    )?;
    let setup: Function<'_> = ctx.eval(include_str!("standalone.js"))?;
    let deliver: Function<'_> = setup.call((api.clone(), native))?;
    ctx.globals().set("__riinaStandaloneMessage", deliver)
}

fn load_file(value: String) -> Intent {
    Intent::LoadFile(value.into())
}

fn js_error(message: String) -> rquickjs::Error {
    rquickjs::Error::new_from_js_message("iina.standaloneWindow", "window", message)
}
