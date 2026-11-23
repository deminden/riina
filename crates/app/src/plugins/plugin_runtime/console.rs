//! The shared console formats values inside their owning JavaScript realm.
use super::{HostEvent, output::record_console};
use crate::diagnostics::Level;
use rquickjs::{Function, Object, function::Func};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub(crate) level: Level,
    pub(crate) text: String,
}

pub(super) fn install(
    api: &Object<'_>,
    events: &Arc<Mutex<Vec<HostEvent>>>,
) -> rquickjs::Result<()> {
    let sink = Object::new(api.ctx().clone())?;
    for (method, level) in [
        ("log", Level::Debug),
        ("info", Level::Info),
        ("warn", Level::Warning),
        ("error", Level::Error),
    ] {
        let events = events.clone();
        sink.set(
            method,
            Func::from(move |text: String| record_console(&events, Message { level, text })),
        )?;
    }
    let setup: Function = api.ctx().eval(include_str!("console.js"))?;
    setup.call((api.clone(), sink))
}
