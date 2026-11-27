//! A cancellable open belongs to one native playlist entry.
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

use crate::{CommandQueue, CommandSender, EngineError, Message, send_error};
use riina_core::{Command, MediaLink, sync::lock};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StreamStatus {
    #[default]
    Loading,
    Opened,
    Failed,
    Cancelled,
}

#[derive(Clone)]
pub struct StreamAttempt {
    id: u64,
    status: Arc<Mutex<StreamStatus>>,
}

impl Default for StreamAttempt {
    fn default() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            status: Arc::default(),
        }
    }
}

impl StreamAttempt {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn status(&self) -> StreamStatus {
        *lock(&self.status)
    }

    pub fn mark_failed(&self) {
        self.finish(StreamStatus::Failed);
    }

    pub fn cancel(&self) {
        self.finish(StreamStatus::Cancelled);
    }

    pub(crate) fn finish(&self, result: StreamStatus) -> bool {
        let mut status = lock(&self.status);
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
        self.open_stream_with_options(source, None, attempt)
    }

    pub fn open_stream_with_options(
        &self,
        source: StreamSource,
        options: Option<MediaLink>,
        attempt: StreamAttempt,
    ) -> Result<(), EngineError> {
        let command = if let Some(mut options) = options {
            options.path.clone_from(&source.address);
            options.append = false;
            Command::OpenMediaLink(options)
        } else {
            Command::Load(source.address.clone())
        };
        self.send_stream(command, source, attempt)
    }

    pub fn open_stream_playlist(
        &self,
        source: StreamSource,
        following: &[String],
        explicit: bool,
        attempt: StreamAttempt,
    ) -> Result<(), EngineError> {
        let paths = std::iter::once(source.address.clone())
            .chain(following.iter().cloned())
            .collect();
        let command = if explicit {
            Command::OpenPlaylist(paths)
        } else {
            Command::OpenFiles(paths)
        };
        self.send_stream(command, source, attempt)
    }

    fn send_stream(
        &self,
        command: Command,
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
                .try_send(Message::OpenStream(command, source, attempt))
                .map_err(send_error),
            #[cfg(feature = "test-support")]
            CommandQueue::Recording(_) => self.send(command),
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
