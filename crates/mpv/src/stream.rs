//! A cancellable open belongs to one native playlist entry.
use std::sync::{Arc, Mutex};

use crate::{CommandQueue, CommandSender, EngineError, Message, send_error};
use riina_core::sync::lock;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StreamStatus {
    #[default]
    Loading,
    Opened,
    Failed,
    Cancelled,
}

#[derive(Clone, Default)]
pub struct StreamAttempt(Arc<Mutex<StreamStatus>>);

impl StreamAttempt {
    pub fn status(&self) -> StreamStatus {
        *lock(&self.0)
    }

    pub fn mark_failed(&self) {
        self.finish(StreamStatus::Failed);
    }

    pub fn cancel(&self) {
        self.finish(StreamStatus::Cancelled);
    }

    pub(crate) fn finish(&self, result: StreamStatus) -> bool {
        let mut status = lock(&self.0);
        if *status == StreamStatus::Loading {
            *status = result;
            true
        } else {
            false
        }
    }
}

pub struct StreamSource {
    pub address: String,
    /// Supplied only to the native stream opener during its load hook.
    pub authenticated_address: Option<String>,
}

impl CommandSender {
    pub fn open_stream(
        &self,
        source: StreamSource,
        attempt: StreamAttempt,
    ) -> Result<(), EngineError> {
        if riina_core::media_reference::validate_network_url(&source.address).is_err() {
            return Err(EngineError::InvalidArgument {
                reason: "The stream address is invalid",
            });
        }
        if self.stopped.load(std::sync::atomic::Ordering::Acquire) {
            return Err(EngineError::Stopped);
        }
        match &self.sender {
            CommandQueue::Native(sender) => sender
                .try_send(Message::OpenStream(source, attempt))
                .map_err(send_error),
            #[cfg(feature = "test-support")]
            CommandQueue::Recording(_) => self.send(riina_core::Command::Load(source.address)),
        }
    }

    pub fn cancel_stream(&self, attempt: &StreamAttempt) {
        attempt.cancel();
        match &self.sender {
            CommandQueue::Native(sender) => {
                // A full queue already guarantees the worker will wake.
                let _ = sender.try_send(Message::Wake);
            }
            #[cfg(feature = "test-support")]
            CommandQueue::Recording(_) => {}
        }
    }
}
