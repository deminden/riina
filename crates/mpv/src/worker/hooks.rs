//! Preserve libmpv's ordering and release every event through its owning client.
use super::{Handle, Message};
use crate::{
    EngineError, MpvHookContinuation, diag,
    hooks::{HookHandler, Registration},
};
use std::{
    collections::HashMap,
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, RecvTimeoutError, SyncSender},
    },
    time::{Duration, Instant},
};

const MAX_NATIVE_HOOKS: usize = 1024;
const HOOK_TIMEOUT: Duration = Duration::from_secs(15);

struct Invocation {
    handler: Weak<HookHandler>,
    finished: Arc<AtomicBool>,
    deadline: Instant,
}

#[derive(Default)]
pub(super) struct Hooks {
    registrations: Vec<Registration>,
    pending: HashMap<u64, Invocation>,
}

impl Hooks {
    pub(super) fn register(
        &mut self,
        handle: &Handle,
        request: Registration,
    ) -> Result<(), EngineError> {
        let mut reusable = None;
        for entry in self.registrations.iter_mut().rev() {
            if entry.name != request.name || entry.priority != request.priority {
                continue;
            }
            if entry.handler.strong_count() != 0 {
                break;
            }
            reusable = Some(entry);
        }
        if let Some(entry) = reusable {
            // libmpv retains registrations until the client closes. Reusing a
            // slot after the last live peer preserves equal-priority ordering.
            entry.handler = request.handler;
            return Ok(());
        }
        let id = self.registrations.len();
        if id >= MAX_NATIVE_HOOKS {
            return Err(EngineError::InvalidArgument {
                reason: "The player has reached its native hook registration limit",
            });
        }
        handle.add_hook(id as u64, &request.name, request.priority)?;
        self.registrations.push(request);
        Ok(())
    }

    pub(super) fn receive(
        &mut self,
        handle: &Handle,
        id: u64,
        slot: u64,
        wake: &SyncSender<Message>,
    ) {
        let handler = usize::try_from(slot)
            .ok()
            .and_then(|slot| self.registrations.get(slot));
        let Some((registration, callback)) =
            handler.and_then(|entry| Some((entry, entry.handler.upgrade()?)))
        else {
            handle.continue_hook(id);
            return;
        };
        let finished = Arc::new(AtomicBool::new(false));
        self.pending.insert(
            id,
            Invocation {
                handler: registration.handler.clone(),
                finished: finished.clone(),
                deadline: Instant::now() + HOOK_TIMEOUT,
            },
        );
        callback(MpvHookContinuation {
            finished,
            wake: wake.clone(),
        });
    }

    pub(super) fn next_message(
        &self,
        receiver: &Receiver<Message>,
        script_deadline: Option<Instant>,
    ) -> Option<Message> {
        let Some(deadline) = self
            .pending
            .values()
            .map(|hook| hook.deadline)
            .chain(script_deadline)
            .min()
        else {
            return receiver.recv().ok();
        };
        match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(message) => Some(message),
            Err(RecvTimeoutError::Timeout) => Some(Message::Wake),
            Err(RecvTimeoutError::Disconnected) => None,
        }
    }

    pub(super) fn continue_ready(&mut self, handle: &Handle) {
        let now = Instant::now();
        self.pending.retain(|&id, hook| {
            let expired = hook.deadline <= now;
            if !expired
                && !hook.finished.load(Ordering::Acquire)
                && hook.handler.strong_count() != 0
            {
                return true;
            }
            if expired && !hook.finished.swap(true, Ordering::AcqRel) {
                diag("Plugin mpv hook timed out; resuming playback");
            }
            hook.finished.store(true, Ordering::Release);
            handle.continue_hook(id);
            false
        });
    }

    pub(super) fn continue_all(&mut self, handle: &Handle) {
        for (id, hook) in self.pending.drain() {
            hook.finished.store(true, Ordering::Release);
            handle.continue_hook(id);
        }
    }
}
