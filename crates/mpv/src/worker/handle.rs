//! The event thread owns the core and every borrowed native event.
use super::super::{WakeupContext, diag, trace_shutdown, wakeup};
use crate::{
    EngineError,
    ffi::{Api, CommandReply, EndFile, Event, Property, event_id},
    properties::{apply_property, observed_property_value, queried_property_value},
    property_read::Request,
};
use riina_core::{CapturedFrame, Playback};
use std::{
    ffi::{CStr, CString, c_int, c_void},
    ptr::{self, NonNull},
    sync::Arc,
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

    pub(super) fn request_property(&self, id: u64, request: &Request) -> Result<(), EngineError> {
        // SAFETY: the live core copies the property name and format for the async read.
        self.api.check("mpv_get_property_async", unsafe {
            (self.api.get_property)(
                self.raw.as_ptr(),
                id,
                request.name.as_ptr(),
                request.format.native(),
            )
        })
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
        // SAFETY: libmpv keeps this event live until the next wait on this client.
        // The exclusive borrow prevents that call while the event can still be read.
        let event = unsafe { &*(self.api.wait)(self.raw.as_ptr(), 0.0) };
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
    pub(super) fn property_reply(&self) -> Result<serde_json::Value, EngineError> {
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

    pub(super) fn end_error(&self, end: &EndFile) -> Option<String> {
        self.api
            .check("mpv_end_file", end.error)
            .err()
            .map(String::from)
    }

    pub(super) fn apply_property(&self, state: &mut Playback, observer: Option<String>) {
        let Some(property) = self.payload::<Property>(event_id::PROPERTY_CHANGE) else {
            return;
        };
        if let Some(name) = observer {
            // SAFETY: the event owns this property and all node data during the copy.
            match unsafe { observed_property_value(property) } {
                Ok(value) => {
                    Arc::make_mut(&mut state.observed_mpv_properties).insert(name, value);
                }
                Err(error) => diagnostic!("Could not copy observed mpv plugin property: {error}"),
            }
        }
        // SAFETY: the live property payload stays borrowed until projection finishes.
        unsafe { apply_property(state, property) };
    }

    pub(super) fn frame(&self) -> Option<Result<CapturedFrame, EngineError>> {
        let reply = self.payload::<CommandReply>(event_id::COMMAND_REPLY)?;
        // SAFETY: the event keeps the screenshot-raw node live through this copy.
        Some(unsafe { reply.result.copy_rgba_frame() })
    }

    pub(super) fn screenshot_filename(&self) -> Option<String> {
        let reply = self.payload::<CommandReply>(event_id::COMMAND_REPLY)?;
        // SAFETY: the event keeps the result node live through this copy.
        unsafe { reply.result.copy_json() }
            .ok()?
            .get("filename")?
            .as_str()
            .map(str::to_owned)
    }
}
