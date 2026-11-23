//! Client-owned hook callbacks and cross-thread continuation signals.
use crate::{CommandQueue, CommandSender, EngineError, Message, client_request::Operation};
use std::{
    ffi::CString,
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, Ordering},
        mpsc::SyncSender,
    },
    time::Instant,
};

pub(super) type HookHandler = dyn Fn(MpvHookContinuation) + Send + Sync;

pub(super) struct Registration {
    pub(super) name: CString,
    pub(super) priority: i32,
    pub(super) handler: Weak<HookHandler>,
}

/// Dropping the registration releases any load waiting on this handler.
pub struct MpvHookRegistration {
    handler: Option<Arc<HookHandler>>,
    wake: SyncSender<Message>,
}

impl Drop for MpvHookRegistration {
    fn drop(&mut self) {
        drop(self.handler.take());
        let _ = self.wake.try_send(Message::Wake);
    }
}

/// A one-shot signal. Only the native worker calls `mpv_hook_continue`.
pub struct MpvHookContinuation {
    pub(super) finished: Arc<AtomicBool>,
    pub(super) wake: SyncSender<Message>,
}

impl MpvHookContinuation {
    pub fn is_pending(&self) -> bool {
        !self.finished.load(Ordering::Acquire)
    }

    pub fn finish(&self) {
        if !self.finished.swap(true, Ordering::AcqRel) {
            // A full command queue already guarantees another worker turn.
            let _ = self.wake.try_send(Message::Wake);
        }
    }
}

impl Drop for MpvHookContinuation {
    fn drop(&mut self) {
        self.finish();
    }
}

impl CommandSender {
    /// Register from a plugin worker. The handler must enqueue and return promptly.
    pub fn register_mpv_hook(
        &self,
        name: &str,
        priority: i32,
        deadline: Instant,
        handler: impl Fn(MpvHookContinuation) + Send + Sync + 'static,
    ) -> Result<MpvHookRegistration, EngineError> {
        let wake = match &self.sender {
            CommandQueue::Native(sender) => sender.clone(),
            #[cfg(feature = "test-support")]
            CommandQueue::Recording(_) => return Err(EngineError::Stopped),
        };
        let invalid_name = EngineError::InvalidArgument {
            reason: "The native hook name is empty, too long, or contains a control character",
        };
        if name.is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
            return Err(invalid_name);
        }
        let name = CString::new(name).map_err(|_| invalid_name)?;
        let handler: Arc<HookHandler> = Arc::new(handler);
        self.request_client(
            Operation::RegisterHook(Registration {
                name,
                priority,
                handler: Arc::downgrade(&handler),
            }),
            deadline,
        )?;
        Ok(MpvHookRegistration {
            handler: Some(handler),
            wake,
        })
    }
}
