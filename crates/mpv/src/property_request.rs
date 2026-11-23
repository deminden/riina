//! Typed asynchronous property operations and their one-shot replies.
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
    pub(super) operation: Operation,
    pub(super) pending: Pending,
}

impl Request {
    pub(super) fn apply_volume_limit(&mut self, boost: bool) -> Result<(), EngineError> {
        if self.name.as_c_str() != c"volume" {
            return Ok(());
        }
        let Operation::Write(value) = &mut self.operation else {
            return Ok(());
        };
        let number = value
            .as_f64()
            .or_else(|| value.as_str()?.parse::<f64>().ok())
            .filter(|number| number.is_finite())
            .ok_or(EngineError::InvalidArgument {
                reason: "Volume must be a finite number",
            })?;
        let limit = if boost { 200.0 } else { 100.0 };
        *value = Value::from(number.clamp(0.0, limit));
        Ok(())
    }
}

pub(super) enum Operation {
    Read(MpvPropertyFormat),
    Write(Value),
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
        self.request_mpv_property(name, Operation::Read(format), deadline)
    }

    /// Use from a non-UI thread. A successful return acknowledges the native write.
    pub fn write_mpv_property(
        &self,
        name: &str,
        value: Value,
        deadline: Instant,
    ) -> Result<(), EngineError> {
        self.request_mpv_property(name, Operation::Write(value), deadline)
            .map(|_| ())
    }

    fn request_mpv_property(
        &self,
        name: &str,
        operation: Operation,
        deadline: Instant,
    ) -> Result<Value, EngineError> {
        if self.stopped.load(Ordering::Acquire) {
            return Err(EngineError::Stopped);
        }
        if name.is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
            return Err(invalid_name());
        }
        if Instant::now() >= deadline {
            return Err(EngineError::PropertyTimeout);
        }
        let name = CString::new(name).map_err(|_| invalid_name())?;
        let (reply, response) = mpsc::sync_channel(1);
        let request = Request {
            name,
            operation,
            pending: Pending { deadline, reply },
        };
        match &self.sender {
            CommandQueue::Native(sender) => sender
                .try_send(Message::Property(request))
                .map_err(send_error)?,
            #[cfg(feature = "test-support")]
            CommandQueue::Recording(_) => return Ok(Value::Null),
        }
        response
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|error| match error {
                mpsc::RecvTimeoutError::Timeout => EngineError::PropertyTimeout,
                mpsc::RecvTimeoutError::Disconnected => EngineError::Stopped,
            })?
    }
}

fn invalid_name() -> EngineError {
    EngineError::InvalidArgument {
        reason: "The native property name is empty, too long, or contains a control character",
    }
}
