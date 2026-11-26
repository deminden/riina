//! Refresh built-in script ownership only after input can change it.
use super::{EventRef, Handle};
use crate::{MpvPropertyFormat, client_request::Operation, ffi::event_id};
use riina_core::{
    Playback,
    key_bindings::{SCRIPT_KEY_LIMIT, normalize_mpv_key},
};
use serde_json::Value;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

const REPLY: u64 = u64::MAX - 2;
// Lua installs forced bindings during its idle callback. libmpv sends no
// input-bindings notification, even when its console open property changes.
const REFRESH_OFFSETS_MS: [u64; 6] = [0, 25, 75, 180, 450, 1000];

#[derive(Default)]
pub(super) struct Watch {
    refresh: Option<(Instant, usize)>,
    pending: bool,
}

impl Watch {
    pub(super) fn schedule(&mut self) {
        self.refresh = Some((Instant::now(), 0));
    }

    pub(super) fn recover(&mut self) {
        self.pending = false;
        self.schedule();
    }

    pub(super) fn deadline(&self) -> Option<Instant> {
        let (started, index) = self.refresh?;
        (!self.pending).then(|| started + Duration::from_millis(REFRESH_OFFSETS_MS[index]))
    }

    pub(super) fn request_due(&mut self, handle: &Handle) {
        if self.deadline().is_none_or(|due| due > Instant::now()) {
            return;
        }
        if let Some((started, index)) = self.refresh {
            // Skip any samples missed while the worker was occupied.
            self.refresh = (index + 1..REFRESH_OFFSETS_MS.len())
                .find(|&next| started.elapsed().as_millis() < REFRESH_OFFSETS_MS[next].into())
                .map(|next| (started, next));
        }
        self.pending = handle
            .request_property(
                REPLY,
                &Operation::Read {
                    name: c"input-bindings".to_owned(),
                    format: MpvPropertyFormat::Node,
                },
            )
            .is_ok();
    }

    pub(super) fn is_reply(event: &EventRef<'_>) -> bool {
        event.id() == event_id::GET_PROPERTY_REPLY && event.userdata() == REPLY
    }

    pub(super) fn apply_reply(&mut self, event: &EventRef<'_>, state: &mut Playback) -> bool {
        self.pending = false;
        event
            .client_reply()
            .is_ok_and(|value| project(state, &value))
    }
}

pub(super) fn changes_statistics(action: &[String]) -> bool {
    match action {
        [verb, binding] if verb == "script-binding" => binding.starts_with("stats/"),
        [verb, target, ..] if verb == "script-message-to" => target == "stats",
        _ => false,
    }
}

fn project(state: &mut Playback, value: &Value) -> bool {
    let mut console = false;
    let mut statistics = Vec::new();
    for binding in value.as_array().into_iter().flatten() {
        if binding.get("is_weak").and_then(Value::as_bool) != Some(false)
            || binding
                .get("priority")
                .and_then(Value::as_i64)
                .is_none_or(|priority| priority < 0)
        {
            continue;
        }
        let Some(key) = binding.get("key").and_then(Value::as_str) else {
            continue;
        };
        match binding.get("owner").and_then(Value::as_str) {
            Some("console") if key == "ANY_UNICODE" => console = true,
            Some("stats") if statistics.len() < SCRIPT_KEY_LIMIT => {
                statistics.push(normalize_mpv_key(key));
            }
            _ => {}
        }
    }
    let changed = state.mpv_console_keys_active != console;
    state.mpv_console_keys_active = console;
    statistics.sort_unstable();
    statistics.dedup();
    if *state.mpv_statistics_keys == statistics {
        return changed;
    }
    state.mpv_statistics_keys = Arc::new(statistics);
    true
}
