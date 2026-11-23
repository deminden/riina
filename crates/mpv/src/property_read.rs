//! Typed requests and event-owned replies for direct libmpv property reads.
use crate::{CommandQueue, CommandSender, EngineError, Message, ffi::format, send_error};
use serde_json::Value;
use std::{
    ffi::{CString, c_int},
    sync::{
        atomic::Ordering,
        mpsc::{self, SyncSender},
    },
    time::Instant,
};

#[derive(Clone, Copy)]
pub enum MpvPropertyFormat {
    Flag,
    Number,
    Text,
    Node,
}

impl MpvPropertyFormat {
    pub(super) fn native(self) -> c_int {
        match self {
            Self::Flag => format::FLAG,
            Self::Number => format::DOUBLE,
            Self::Text => format::STRING,
            Self::Node => format::NODE,
        }
    }
}

pub(super) struct Request {
    pub(super) name: CString,
    pub(super) format: MpvPropertyFormat,
    pub(super) pending: Pending,
}

pub(super) struct Pending {
    pub(super) deadline: Instant,
    pub(super) reply: SyncSender<Result<Value, EngineError>>,
}

impl CommandSender {
    /// Use from a non-UI thread. The event worker submits an asynchronous mpv read
    /// so it can continue processing events while the caller waits for this reply.
    pub fn read_mpv_property(
        &self,
        name: &str,
        format: MpvPropertyFormat,
        deadline: Instant,
    ) -> Result<Value, EngineError> {
        if self.stopped.load(Ordering::Acquire) {
            return Err(EngineError::Stopped);
        }
        if name.is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
            return Err(invalid_name());
        }
        if Instant::now() >= deadline {
            return Err(EngineError::PropertyReadTimeout);
        }
        let name = CString::new(name).map_err(|_| invalid_name())?;
        let (reply, response) = mpsc::sync_channel(1);
        let request = Request {
            name,
            format,
            pending: Pending { deadline, reply },
        };
        match &self.sender {
            CommandQueue::Native(sender) => sender
                .try_send(Message::ReadProperty(request))
                .map_err(send_error)?,
            #[cfg(feature = "test-support")]
            CommandQueue::Recording(_) => return Ok(Value::Null),
        }
        response
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|error| match error {
                mpsc::RecvTimeoutError::Timeout => EngineError::PropertyReadTimeout,
                mpsc::RecvTimeoutError::Disconnected => EngineError::Stopped,
            })?
    }
}

fn invalid_name() -> EngineError {
    EngineError::InvalidArgument {
        reason: "The native property name is empty, too long, or contains a control character",
    }
}
