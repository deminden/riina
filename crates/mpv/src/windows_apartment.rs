//! Audio-device enumeration needs a live Windows COM apartment through core teardown.
use std::{ffi::c_void, marker::PhantomData, rc::Rc};

#[link(name = "ole32")]
unsafe extern "system" {
    fn CoInitializeEx(reserved: *mut c_void, flags: u32) -> i32;
    fn CoUninitialize();
}

pub(super) struct Apartment(PhantomData<Rc<()>>);

impl Apartment {
    pub(super) fn initialize() -> Result<Self, String> {
        // The fresh event worker owns one MTA initialization. A live outer
        // apartment keeps Windows audio notification objects valid while mpv
        // performs its nested enumeration and destroys the owning core.
        // SAFETY: null reserved pointer and COINIT_MULTITHREADED on this thread.
        let result = unsafe { CoInitializeEx(std::ptr::null_mut(), 0) };
        if result < 0 {
            return Err(format!(
                "Could not initialize Windows playback COM: {result:#x}"
            ));
        }
        // S_OK and S_FALSE both require a matching CoUninitialize. The Rc marker
        // prevents moving that obligation to another thread.
        Ok(Self(PhantomData))
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: this guard remains on the initializing thread and is dropped
        // after the native core, callbacks, and their COM objects are destroyed.
        unsafe { CoUninitialize() };
    }
}
