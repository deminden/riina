use super::{EventRef, Handle, WorkerState};
use crate::{
    client_request::Operation,
    ffi::{EndFile, end_reason, event_id},
    pending::ReplyKind,
    stream::{StreamAttempt, StreamSource, StreamStatus},
};
use riina_core::{Command, FileFeedback, Phase};

pub(super) const AUTH_HOOK_SLOT: u64 = u64::MAX;
const TRANSPORT_PROPERTIES: [&str; 2] = ["stream-open-filename", "stream-path"];

pub(super) struct StreamLoad {
    attempt: StreamAttempt,
    reply: Option<u64>,
    cancel_reply: Option<u64>,
    entry: Option<i64>,
    authentication: Option<String>,
    address: String,
}

impl StreamLoad {
    pub(super) fn attempt_id(&self) -> u64 {
        self.attempt.id()
    }

    pub(super) fn cancelled(&self) -> bool {
        self.attempt.status() == StreamStatus::Cancelled
    }

    pub(super) fn for_reply(mut self, reply: u64) -> Self {
        self.reply = Some(reply);
        self
    }

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
        command: Command,
        source: StreamSource,
        attempt: StreamAttempt,
    ) {
        match attempt.status() {
            StreamStatus::Loading => {}
            StreamStatus::Cancelled => {
                // A cancelled request which never reached mpv may still own an
                // empty player. An earlier playable file keeps its core.
                if self.active_entry.is_none()
                    && self.playback.playlist.is_empty()
                    && self.media_generation_is_current(self.playback.generation)
                {
                    self.record_cancelled_open(attempt.id());
                }
                return;
            }
            _ => return,
        }
        if source.authenticated_address.is_some() && !self.stream_auth.installed {
            if handle.add_hook(AUTH_HOOK_SLOT, c"on_load", 0).is_err() {
                attempt.finish(StreamStatus::Failed);
                return;
            }
            self.stream_auth.installed = true;
        }

        let stream = StreamLoad {
            attempt,
            reply: None,
            cancel_reply: None,
            entry: None,
            authentication: source.authenticated_address,
            address: source.address,
        };
        self.handle_intent_with_stream(handle, command, Some(stream), None);
    }

    pub(super) fn on_stream_reply(&mut self, event: &EventRef<'_>) {
        let Some(load) = self.stream.as_mut() else {
            return;
        };
        let id = event.userdata();
        let cancelled_idle = load.cancelled() && self.active_entry.is_none();
        let ended_without_file = if load.reply == Some(id) {
            load.entry = event.loaded_entry();
            load.entry.is_none()
        } else {
            load.cancel_reply == Some(id) && !event.failed() && cancelled_idle
        };
        if ended_without_file {
            let attempt = load.attempt_id();
            self.stream = None;
            if cancelled_idle {
                self.record_cancelled_open(attempt);
            }
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
            load.entry.is_some() && load.cancel_reply.is_none() && load.cancelled()
        });
        // Keep the attempt through END_FILE or the Stop reply. A replacement
        // discards this ownership before its own playback can be stopped.
        if shall_stop
            && self
                .issue(handle, &["stop".into()], ReplyKind::Plain)
                .is_ok()
            && let Some(load) = self.stream.as_mut()
        {
            load.cancel_reply = Some(self.next_id);
        }
    }

    pub(super) fn record_cancelled_open(&mut self, stream_attempt: u64) {
        self.playback.phase = Phase::Stopped;
        self.playback.file_feedback = FileFeedback::OpenCancelled { stream_attempt };
        self.mark_changed();
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
