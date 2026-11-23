//! The shared console formats values inside their owning JavaScript realm.
use super::{
    HostEvent, UiEffect, developer,
    output::{record, record_console},
};
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
    developer: developer::Prepared,
) -> rquickjs::Result<()> {
    let log_events = events.clone();
    let developer_log = developer.clone();
    api.set(
        "log",
        Func::from(move |message: String| {
            developer_log.log(Level::Debug, &message);
            record(&log_events, UiEffect::Log(message));
        }),
    )?;
    let sink = Object::new(api.ctx().clone())?;
    for (method, level) in [
        ("log", Level::Debug),
        ("info", Level::Info),
        ("warn", Level::Warning),
        ("error", Level::Error),
    ] {
        let events = events.clone();
        let developer = developer.clone();
        sink.set(
            method,
            Func::from(move |text: String| {
                developer.log(level, &text);
                record_console(&events, Message { level, text });
            }),
        )?;
    }
    let setup: Function = api.ctx().eval(include_str!("console.js"))?;
    setup.call((api.clone(), sink))
}
