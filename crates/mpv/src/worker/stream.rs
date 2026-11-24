use super::{EventRef, Handle, WorkerState};
use crate::{
    client_request::Operation,
    ffi::{EndFile, end_reason, event_id},
    pending::ReplyKind,
    stream::{StreamAttempt, StreamSource, StreamStatus},
};
use riina_core::Command;

pub(super) const AUTH_HOOK_SLOT: u64 = u64::MAX;
const TRANSPORT_PROPERTIES: [&str; 2] = ["stream-open-filename", "stream-path"];

pub(super) struct StreamLoad {
    attempt: StreamAttempt,
    reply: u64,
    entry: Option<i64>,
    authentication: Option<String>,
    address: String,
}

impl StreamLoad {
    fn owns(&self, entry: i64) -> bool {
        self.entry == Some(entry)
    }

    pub(super) fn superseded_by(&self, entry: i64) -> bool {
        self.entry.is_some() && !self.owns(entry)
    }

    pub(super) fn end(&mut self, event: &EndFile) -> bool {
        if !self.owns(event.playlist_entry_id) {
            return false;
        }
        if event.reason == end_reason::REDIRECT && event.playlist_insert_num_entries > 0 {
            self.entry = Some(event.playlist_insert_id);
            return false;
        }
        true
    }
}

impl Drop for StreamLoad {
    fn drop(&mut self) {
        self.attempt.finish(StreamStatus::Failed);
    }
}

#[derive(Default)]
pub(super) struct AuthenticationHook {
    installed: bool,
    pending_reply: Option<(u64, u64, Option<i64>)>,
    pub(super) public_address: Option<String>,
}

impl AuthenticationHook {
    pub(super) fn protects(&self, name: &str) -> bool {
        self.public_address.is_some() && TRANSPORT_PROPERTIES.contains(&name)
    }

    pub(super) fn take_reply(&mut self, event: &EventRef<'_>) -> Option<(u64, Option<i64>, bool)> {
        let (reply, hook, entry) = self.pending_reply?;
        if event.id() != event_id::SET_PROPERTY_REPLY || event.userdata() != reply {
            return None;
        }
        self.pending_reply = None;
        Some((hook, entry, event.failed()))
    }
}

impl WorkerState {
    pub(super) fn open_stream(
        &mut self,
        handle: &Handle,
        source: StreamSource,
        attempt: StreamAttempt,
    ) {
        if attempt.status() != StreamStatus::Loading {
            return;
        }
        if source.authenticated_address.is_some() && !self.stream_auth.installed {
            if handle.add_hook(AUTH_HOOK_SLOT, c"on_load", 0).is_err() {
                attempt.finish(StreamStatus::Failed);
                return;
            }
            self.stream_auth.installed = true;
        }

        let before = self.next_id;
        self.handle_intent(handle, Command::Load(source.address.clone()));
        if before == self.next_id
            || self
                .pending
                .get(&self.next_id)
                .is_none_or(|reply| reply.label != "loadfile")
        {
            attempt.finish(StreamStatus::Failed);
            return;
        }
        self.stream = Some(StreamLoad {
            attempt,
            reply: self.next_id,
            entry: None,
            authentication: source.authenticated_address,
            address: source.address,
        });
    }

    pub(super) fn on_stream_reply(&mut self, event: &EventRef<'_>) {
        let Some(load) = self
            .stream
            .as_mut()
            .filter(|load| load.reply == event.userdata())
        else {
            return;
        };
        if let Some(entry) = event.loaded_entry() {
            load.entry = Some(entry);
        } else {
            self.stream = None;
        }
    }

    pub(super) fn complete_open_stream(&mut self) {
        if let Some(load) = &self.stream
            && self.active_entry.is_some_and(|entry| load.owns(entry))
            && load.attempt.finish(StreamStatus::Opened)
        {
            self.stream = None;
        }
    }

    pub(super) fn stop_cancelled_stream(&mut self, handle: &Handle) {
        let shall_stop = self.stream.as_ref().is_some_and(|load| {
            load.entry.is_some() && load.attempt.status() == StreamStatus::Cancelled
        });
        if shall_stop {
            self.stream = None;
            // Only this request's load was acknowledged. A superseding media
            // command or start event discards that ownership before it can stop.
            let _ = self.issue(handle, &["stop".into()], ReplyKind::Plain);
        }
    }

    pub(super) fn authenticate_stream(&mut self, handle: &Handle, hook: u64) {
        let address = self
            .stream
            .as_mut()
            .filter(|load| {
                self.active_entry.is_some_and(|entry| load.owns(entry))
                    && load.attempt.status() == StreamStatus::Loading
            })
            .and_then(|load| Some((load.authentication.take()?, load.address.clone())));
        let Some((address, public_address)) = address else {
            handle.continue_hook(hook);
            return;
        };
        self.next_id += 1;
        // Keep the uncredentialed filename in mpv's playlist and observations.
        // FFmpeg receives user information only on the actual HTTP(S) transport.
        let operation = Operation::Write {
            name: c"stream-open-filename".to_owned(),
            value: serde_json::Value::String(address),
        };
        if handle.request_property(self.next_id, &operation).is_ok() {
            self.stream_auth.public_address = Some(public_address.clone());
            for property in TRANSPORT_PROPERTIES {
                if self.observers.values().any(|name| name == property) {
                    std::sync::Arc::make_mut(&mut self.playback.observed_mpv_properties).insert(
                        property.into(),
                        serde_json::Value::String(public_address.clone()),
                    );
                }
            }
            self.stream_auth.pending_reply = Some((self.next_id, hook, self.active_entry));
        } else {
            self.finish_stream_auth(handle, hook, self.active_entry, true);
        }
    }

    pub(super) fn finish_stream_auth(
        &mut self,
        handle: &Handle,
        hook: u64,
        entry: Option<i64>,
        failed: bool,
    ) {
        if failed && self.stream.as_ref().is_some_and(|load| load.entry == entry) {
            self.stream = None;
            let _ = self.issue(handle, &["stop".into()], ReplyKind::Plain);
        }
        handle.continue_hook(hook);
    }

    pub(super) fn continue_stream_auth(&mut self, handle: &Handle) {
        if let Some((_, hook, _)) = self.stream_auth.pending_reply.take() {
            handle.continue_hook(hook);
        }
    }
}
