use riina_core::playback_power::Mode;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use windows::{
    Win32::{
        Foundation::HANDLE,
        System::{
            Power::{
                PowerClearRequest, PowerCreateRequest, PowerRequestDisplayRequired,
                PowerRequestSystemRequired, PowerSetRequest,
            },
            SystemServices::POWER_REQUEST_CONTEXT_VERSION,
            Threading::{POWER_REQUEST_CONTEXT_SIMPLE_STRING, REASON_CONTEXT, REASON_CONTEXT_0},
        },
    },
    core::PWSTR,
};

pub(super) struct Lease {
    handle: OwnedHandle,
    system: bool,
    display: bool,
}

impl Lease {
    pub(super) fn new(mode: Mode) -> Option<Self> {
        if mode == Mode::Idle {
            return None;
        }
        Self::acquire(mode)
            .inspect_err(|error| {
                crate::diagnostics::warn!("Could not set playback power request: {error}")
            })
            .ok()
    }

    fn acquire(mode: Mode) -> windows::core::Result<Self> {
        let mut reason: Vec<_> = "Riina playback is in progress"
            .encode_utf16()
            .chain([0])
            .collect();
        let context = REASON_CONTEXT {
            Version: POWER_REQUEST_CONTEXT_VERSION,
            Flags: POWER_REQUEST_CONTEXT_SIMPLE_STRING,
            Reason: REASON_CONTEXT_0 {
                SimpleReasonString: PWSTR(reason.as_mut_ptr()),
            },
        };
        // SAFETY: the reason stays valid during creation and the resulting process
        // handle is owned here. Only successfully set requests need clearing.
        unsafe {
            let handle = PowerCreateRequest(&context)?;
            let mut lease = Self {
                handle: OwnedHandle::from_raw_handle(handle.0),
                system: false,
                display: false,
            };
            PowerSetRequest(handle, PowerRequestSystemRequired)?;
            lease.system = true;
            if mode == Mode::Display {
                PowerSetRequest(handle, PowerRequestDisplayRequired)?;
                lease.display = true;
            }
            trace("acquired", mode);
            Ok(lease)
        }
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        let handle = HANDLE(self.handle.as_raw_handle());
        // SAFETY: the handle still owns this lease's matching request counts.
        unsafe {
            let display =
                !self.display || PowerClearRequest(handle, PowerRequestDisplayRequired).is_ok();
            let system =
                !self.system || PowerClearRequest(handle, PowerRequestSystemRequired).is_ok();
            if display && system && self.system {
                trace(
                    "released",
                    if self.display {
                        Mode::Display
                    } else {
                        Mode::System
                    },
                );
            }
        }
    }
}

fn trace(action: &str, mode: Mode) {
    if cfg!(debug_assertions) {
        crate::diagnostics::log_args(
            crate::diagnostics::Level::Debug,
            "playback-power",
            format_args!("Windows request {action}: {mode:?}"),
        );
    }
}
