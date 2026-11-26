//! The event thread owns the core and every borrowed native event.
use super::super::{WakeupContext, diag, trace_shutdown, wakeup};
use crate::{
    EngineError,
    client_request::Operation,
    ffi::{
        Api, CommandReply, EndFile, Event, HookEvent, Property, event_id, format, property_string,
        with_node,
    },
    properties::{apply_property, observed_property_value, queried_property_value},
};
use riina_core::{CapturedFrame, Playback};
use std::{
    ffi::{CStr, CString, c_int, c_void},
    ptr::{self, NonNull},
};

pub(super) struct Handle {
    api: Api,
    raw: NonNull<c_void>,
    callback: Option<Box<WakeupContext>>,
}

impl Handle {
    pub(super) fn new(api: Api) -> Result<Self, EngineError> {
        // SAFETY: this creates a new core owned solely by this worker.
        let raw = unsafe { (api.create)() };
        Ok(Self {
            api,
            raw: NonNull::new(raw).ok_or_else(|| EngineError::Native {
                call: "mpv_create",
                message: "mpv_create failed".into(),
            })?,
            callback: None,
        })
    }

    pub(super) fn set_option(&self, key: &CStr, value: &CStr) -> Result<(), EngineError> {
        // SAFETY: the core is live and both strings remain valid until mpv copies them.
        self.api.check("mpv_set_option_string", unsafe {
            (self.api.option)(self.raw.as_ptr(), key.as_ptr(), value.as_ptr())
        })
    }

    pub(super) fn initialize(&self) -> Result<(), EngineError> {
        // SAFETY: the worker calls this once after configuration and before using the client.
        self.api.check("mpv_initialize", unsafe {
            (self.api.initialize)(self.raw.as_ptr())
        })
    }

    pub(super) fn observe(&self, id: u64, name: &CStr, format: c_int) -> Result<(), EngineError> {
        // SAFETY: the live core copies the NUL-terminated property name during this call.
        self.api.check("mpv_observe_property", unsafe {
            (self.api.observe)(self.raw.as_ptr(), id, name.as_ptr(), format)
        })
    }

    pub(in crate::worker) fn add_hook(
        &self,
        userdata: u64,
        name: &CStr,
        priority: i32,
    ) -> Result<(), EngineError> {
        // SAFETY: libmpv copies the terminated hook name on this live client.
        self.api.check("mpv_hook_add", unsafe {
            (self.api.hook_add)(self.raw.as_ptr(), userdata, name.as_ptr(), priority)
        })
    }

    pub(in crate::worker) fn continue_hook(&self, id: u64) {
        // SAFETY: the worker consumes this ID once, on the client that received it.
        if let Err(error) = self.api.check("mpv_hook_continue", unsafe {
            (self.api.hook_continue)(self.raw.as_ptr(), id)
        }) {
            diag(&error.to_string());
        }
    }

    pub(super) fn request_property(
        &self,
        id: u64,
        operation: &Operation,
    ) -> Result<(), EngineError> {
        match operation {
            Operation::Read { name, format } => {
                // SAFETY: the live core copies the name and format for the async read.
                self.api.check("mpv_get_property_async", unsafe {
                    (self.api.get_property)(self.raw.as_ptr(), id, name.as_ptr(), format.native())
                })
            }
            Operation::Write { name, value } => self.write_property(id, name, value),
            Operation::Intent(_) | Operation::RegisterHook(_) => {
                Err(EngineError::InvalidArgument {
                    reason: "The operation is not a native property request",
                })
            }
        }
    }

    fn write_property(
        &self,
        id: u64,
        name: &CStr,
        value: &serde_json::Value,
    ) -> Result<(), EngineError> {
        use serde_json::Value;
        let submit = |format, data| {
            // SAFETY: each branch pairs the format with its matching storage.
            // The name and data stay live until libmpv has copied them.
            self.api.check("mpv_set_property_async", unsafe {
                (self.api.set_property)(self.raw.as_ptr(), id, name.as_ptr(), format, data)
            })
        };
        match value {
            Value::Bool(flag) => {
                let mut flag = c_int::from(*flag);
                submit(format::FLAG, (&raw mut flag).cast())
            }
            Value::Number(number) => {
                let mut number = number.as_f64().ok_or(EngineError::InvalidArgument {
                    reason: "Native property number cannot be represented",
                })?;
                submit(format::DOUBLE, (&raw mut number).cast())
            }
            Value::String(text) => {
                let text = property_string(text)?;
                let mut pointer = text.as_ptr();
                submit(format::STRING, (&raw mut pointer).cast())
            }
            Value::Array(_) | Value::Object(_) => with_node(value, |node| {
                submit(format::NODE, ptr::from_mut(node).cast())
            }),
            Value::Null => Err(EngineError::InvalidArgument {
                reason: "A native property value cannot be null",
            }),
        }
    }

    pub(super) fn register_wakeup(&mut self, mut callback: Box<WakeupContext>) {
        let context = (&mut *callback as *mut WakeupContext).cast();
        // SAFETY: the boxed context has a stable address and is retained through destruction.
        unsafe { (self.api.wakeup)(self.raw.as_ptr(), Some(wakeup), context) };
        self.callback = Some(callback);
    }

    pub(super) fn acknowledge_wakeup(&self) {
        if let Some(callback) = &self.callback {
            use std::sync::atomic::Ordering;
            callback.queued.store(false, Ordering::Release);
        }
    }

    pub(super) fn address(&self) -> usize {
        self.raw.as_ptr() as usize
    }

    pub(super) fn command(&self, id: u64, args: &[CString]) -> Result<(), EngineError> {
        let mut pointers: Vec<_> = args.iter().map(|arg| arg.as_ptr()).collect();
        pointers.push(ptr::null());
        // SAFETY: the live core copies the NUL-terminated array and strings before returning.
        self.api.check("mpv_command_async", unsafe {
            (self.api.command)(self.raw.as_ptr(), id, pointers.as_ptr())
        })
    }

    pub(super) fn stop_after_panic(&self) {
        let stop = [c"stop".as_ptr(), ptr::null()];
        // SAFETY: the core is live and the command array and its only string are terminated.
        unsafe { (self.api.command)(self.raw.as_ptr(), 0, stop.as_ptr()) };
    }

    pub(super) fn wait(&mut self) -> Option<EventRef<'_>> {
        self.wait_timeout(std::time::Duration::ZERO)
    }

    pub(super) fn wait_timeout(&mut self, timeout: std::time::Duration) -> Option<EventRef<'_>> {
        // SAFETY: libmpv keeps this event live until the next wait on this client.
        // The exclusive borrow prevents that call while the event can still be read.
        let event = unsafe { &*(self.api.wait)(self.raw.as_ptr(), timeout.as_secs_f64()) };
        (event.id != event_id::NONE).then_some(EventRef {
            event,
            api: &self.api,
        })
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        if self.callback.is_some() {
            // SAFETY: unregister prevents new calls while the callback storage stays live.
            unsafe { (self.api.wakeup)(self.raw.as_ptr(), None, ptr::null_mut()) };
            trace_shutdown("wakeup callback unregistered; terminating native core");
        }
        // SAFETY: the worker owns this core. The final owner has freed all render contexts.
        unsafe { (self.api.destroy)(self.raw.as_ptr()) };
        if self.callback.is_some() {
            trace_shutdown("native core terminated");
            drop(self.callback.take());
            trace_shutdown("callback context released");
        }
    }
}

pub(super) struct EventRef<'a> {
    event: &'a Event,
    api: &'a Api,
}

impl EventRef<'_> {
    pub(super) fn client_reply(&self) -> Result<serde_json::Value, EngineError> {
        if self.id() == event_id::COMMAND_REPLY {
            self.api.check("mpv_command_async", self.event.error)?;
            return Ok(serde_json::Value::Null);
        }
        if self.id() == event_id::SET_PROPERTY_REPLY {
            self.api.check("mpv_set_property_async", self.event.error)?;
            return Ok(serde_json::Value::Null);
        }
        if self.failed() {
            return Ok(serde_json::Value::Null);
        }
        let Some(property) = self.payload::<Property>(event_id::GET_PROPERTY_REPLY) else {
            return Ok(serde_json::Value::Null);
        };
        // SAFETY: both scalar and node data stay live until the worker waits again.
        unsafe { queried_property_value(property) }.map_err(|error| EngineError::Native {
            call: "mpv_get_property_async",
            message: error.to_string(),
        })
    }

    pub(super) fn id(&self) -> c_int {
        self.event.id
    }

    pub(super) fn userdata(&self) -> u64 {
        self.event.userdata
    }

    pub(super) fn hook_id(&self) -> Option<u64> {
        self.payload::<HookEvent>(event_id::HOOK)
            .map(|hook| hook.id)
    }

    pub(super) fn failed(&self) -> bool {
        self.event.error < 0
    }

    pub(super) fn error(&self) -> Option<String> {
        self.api
            .check("mpv_wait_event", self.event.error)
            .err()
            .map(String::from)
    }

    pub(super) fn error_code(&self) -> c_int {
        self.event.error
    }

    pub(super) fn name(&self) -> Option<&CStr> {
        // SAFETY: mpv accepts any event ID and returns a static C string or null.
        let name = unsafe { (self.api.event_name)(self.id()) };
        // SAFETY: the checked pointer is a NUL-terminated libmpv string.
        (!name.is_null()).then(|| unsafe { CStr::from_ptr(name) })
    }

    fn payload<T>(&self, id: c_int) -> Option<&T> {
        if self.id() != id || self.event.data.is_null() {
            return None;
        }
        // SAFETY: callers below bind each event ID to its libmpv payload type.
        // The result is lifetime-bound to this event, before the next wait.
        Some(unsafe { &*self.event.data.cast::<T>() })
    }

    pub(super) fn end_file(&self) -> Option<&EndFile> {
        self.payload(event_id::END_FILE)
    }

    pub(super) fn started_entry(&self) -> Option<i64> {
        self.payload::<crate::ffi::StartFile>(event_id::START_FILE)
            .map(|event| event.playlist_entry_id)
    }

    pub(super) fn end_error(&self, end: &EndFile) -> Option<String> {
        self.api
            .check("mpv_end_file", end.error)
            .err()
            .map(String::from)
    }

    pub(super) fn apply_property(&self, state: &mut Playback) {
        let Some(property) = self.payload::<Property>(event_id::PROPERTY_CHANGE) else {
            return;
        };
        // SAFETY: the live property payload stays borrowed until projection finishes.
        unsafe { apply_property(state, property) };
    }

    pub(super) fn plugin_property(&self) -> Option<(serde_json::Value, usize)> {
        let property = self.payload::<Property>(event_id::PROPERTY_CHANGE)?;
        // SAFETY: the event owns this property and all node data during the copy.
        match unsafe { observed_property_value(property) } {
            Ok(value) => Some(value),
            Err(error) => {
                diagnostic!("Could not copy observed mpv plugin property: {error}");
                None
            }
        }
    }

    pub(super) fn frame(&self) -> Option<Result<CapturedFrame, EngineError>> {
        let reply = self.payload::<CommandReply>(event_id::COMMAND_REPLY)?;
        // SAFETY: the event keeps the screenshot-raw node live through this copy.
        Some(unsafe { reply.result.copy_rgba_frame() })
    }

    pub(super) fn screenshot_filename(&self) -> Option<String> {
        self.command_result()?
            .get("filename")?
            .as_str()
            .map(str::to_owned)
    }

    pub(super) fn loaded_entry(&self) -> Option<i64> {
        self.command_result()?.get("playlist_entry_id")?.as_i64()
    }

    fn command_result(&self) -> Option<serde_json::Value> {
        let reply = self.payload::<CommandReply>(event_id::COMMAND_REPLY)?;
        // SAFETY: the event keeps the result node live through this copy.
        unsafe { reply.result.copy_json() }.ok()
    }
}
