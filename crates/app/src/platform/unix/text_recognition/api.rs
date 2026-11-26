//! Keep the loaded library alive while any C entry point or handle is usable.
mod engine;

use libloading::Library;
use std::ffi::{c_char, c_int, c_void};

macro_rules! opaque_handles {
    ($($handle:ident),+ $(,)?) => {$(#[repr(C)] struct $handle { _opaque: [u8; 0] })+};
}

opaque_handles!(Base, Monitor, IteratorHandle, Page);

type CancelCallback = unsafe extern "C" fn(*mut c_void, c_int) -> bool;

macro_rules! tesseract_api {
    ($($field:ident: $ty:ty = $symbol:literal),+ $(,)?) => {
        pub(super) struct Api {
            $($field: $ty,)+
            _library: Library,
        }

        impl Api {
            pub(super) fn load() -> Result<Self, String> {
                // SAFETY: the explicit soname selects the system Tesseract 5 ABI.
                let library = unsafe { Library::new("libtesseract.so.5") }.map_err(|error| error.to_string())?;
                // SAFETY: each C symbol is loaded with its public capi.h signature.
                unsafe {
                    Ok(Self {
                        $($field: *library.get::<$ty>($symbol).map_err(|error| error.to_string())?,)+
                        _library: library,
                    })
                }
            }
        }
    };
}

/// A recognized line in top-left image coordinates.
pub(super) struct Line {
    pub(super) text: String,
    pub(super) bounds: [c_int; 4],
}

tesseract_api! {
    create: unsafe extern "C" fn() -> *mut Base = b"TessBaseAPICreate\0",
    delete: unsafe extern "C" fn(*mut Base) = b"TessBaseAPIDelete\0",
    init: unsafe extern "C" fn(*mut Base, *const c_char, *const c_char) -> c_int = b"TessBaseAPIInit3\0",
    segmentation: unsafe extern "C" fn(*mut Base, c_int) = b"TessBaseAPISetPageSegMode\0",
    image: unsafe extern "C" fn(*mut Base, *const u8, c_int, c_int, c_int, c_int) = b"TessBaseAPISetImage\0",
    recognize: unsafe extern "C" fn(*mut Base, *mut Monitor) -> c_int = b"TessBaseAPIRecognize\0",
    iterator: unsafe extern "C" fn(*mut Base) -> *mut IteratorHandle = b"TessBaseAPIGetIterator\0",
    iterator_delete: unsafe extern "C" fn(*mut IteratorHandle) = b"TessResultIteratorDelete\0",
    iterator_next: unsafe extern "C" fn(*mut IteratorHandle, c_int) -> c_int = b"TessResultIteratorNext\0",
    iterator_text: unsafe extern "C" fn(*const IteratorHandle, c_int) -> *mut c_char = b"TessResultIteratorGetUTF8Text\0",
    page_iterator: unsafe extern "C" fn(*const IteratorHandle) -> *mut Page = b"TessResultIteratorGetPageIterator\0",
    bounds: unsafe extern "C" fn(*const Page, c_int, *mut c_int, *mut c_int, *mut c_int, *mut c_int) -> c_int = b"TessPageIteratorBoundingBox\0",
    text_delete: unsafe extern "C" fn(*const c_char) = b"TessDeleteText\0",
    monitor: unsafe extern "C" fn() -> *mut Monitor = b"TessMonitorCreate\0",
    monitor_delete: unsafe extern "C" fn(*mut Monitor) = b"TessMonitorDelete\0",
    monitor_deadline: unsafe extern "C" fn(*mut Monitor, c_int) = b"TessMonitorSetDeadlineMSecs\0",
    monitor_set_context: unsafe extern "C" fn(*mut Monitor, *mut c_void) = b"TessMonitorSetCancelThis\0",
    monitor_cancel: unsafe extern "C" fn(*mut Monitor, CancelCallback) = b"TessMonitorSetCancelFunc\0",
}
