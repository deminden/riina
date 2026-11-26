use super::{Api, Base, Line};
use crate::platform::text_recognition::Cancellation;
use riina_core::CapturedFrame;
use std::{
    ffi::{CStr, c_int, c_void},
    ptr::{self, NonNull},
};

use super::super::languages::Configuration;

const SPARSE_TEXT: c_int = 11; // PSM_SPARSE_TEXT
const TEXT_LINE: c_int = 2; // RIL_TEXTLINE

pub(in super::super) struct Engine<'a> {
    api: &'a Api,
    base: Owned<Base>,
}

impl Api {
    pub(in super::super) fn open(
        &self,
        configuration: &Configuration,
    ) -> Result<Engine<'_>, String> {
        // SAFETY: the constructor returns a new engine handle or null.
        let base = Owned::new(unsafe { (self.create)() }, self.delete)
            .ok_or("Could not allocate Tesseract")?;
        let directory = configuration
            .directory
            .as_ref()
            .map_or(ptr::null(), |path| path.as_ptr());
        // SAFETY: the engine is owned and both strings are terminated or optional.
        unsafe {
            if (self.init)(base.as_ptr(), directory, configuration.languages.as_ptr()) != 0 {
                return Err("Tesseract could not load its text recognition data".into());
            }
            // Paused video can contain separate lines in unrelated parts of the frame.
            (self.segmentation)(base.as_ptr(), SPARSE_TEXT);
        }
        Ok(Engine { api: self, base })
    }
}

impl Engine<'_> {
    pub(in super::super) fn lines(
        &mut self,
        frame: &CapturedFrame,
        cancellation: &Cancellation,
    ) -> Result<Vec<Line>, String> {
        if cancellation.is_cancelled() {
            return Ok(Vec::new());
        }
        let api = self.api;
        let base = self.base.as_ptr();
        let width = c_int::try_from(frame.width).map_err(|error| error.to_string())?;
        let height = c_int::try_from(frame.height).map_err(|error| error.to_string())?;
        let stride = width
            .checked_mul(4)
            .ok_or("Invalid text recognition row stride")?;
        let size = usize::try_from(stride)
            .ok()
            .and_then(|row| row.checked_mul(frame.height as usize));
        if width == 0 || height == 0 || size != Some(frame.rgba.len()) {
            return Err("Captured frame data does not match its dimensions".into());
        }
        // SAFETY: the constructor transfers a fresh monitor, which this call owns.
        let monitor = Owned::new(unsafe { (api.monitor)() }, api.monitor_delete)
            .ok_or("Could not allocate a Tesseract progress monitor")?;
        // SAFETY: the monitor and tightly packed frame remain live for synchronous OCR.
        // Its callback borrows cancellation only within this call, on this worker.
        let recognized = unsafe {
            (api.image)(base, frame.rgba.as_ptr(), width, height, 4, stride);
            (api.monitor_set_context)(
                monitor.as_ptr(),
                ptr::from_ref(cancellation).cast_mut().cast(),
            );
            (api.monitor_cancel)(monitor.as_ptr(), cancelled);
            (api.monitor_deadline)(monitor.as_ptr(), 3_000);
            (api.recognize)(base, monitor.as_ptr())
        };
        if cancellation.is_cancelled() {
            return Ok(Vec::new());
        }
        if recognized != 0 {
            return Err("Tesseract did not complete text recognition".into());
        }
        // SAFETY: the completed engine lends its result memory to the owned iterator.
        let result = Owned::new(unsafe { (api.iterator)(base) }, api.iterator_delete);
        Ok(result.map_or_else(Vec::new, |result| {
            self.copy_lines(result.as_ptr(), cancellation)
        }))
    }

    fn copy_lines(
        &self,
        result: *mut super::IteratorHandle,
        cancellation: &Cancellation,
    ) -> Vec<Line> {
        const MAX_LINES: usize = 256;
        let api = self.api;
        // SAFETY: the page iterator is borrowed from the live result on this worker.
        let page = unsafe { (api.page_iterator)(result) };
        let mut lines = Vec::new();
        if page.is_null() {
            return lines;
        }
        for _ in 0..MAX_LINES {
            if cancellation.is_cancelled() {
                return Vec::new();
            }
            let [mut left, mut top, mut right, mut bottom] = [0; 4];
            // SAFETY: the current result is live, and each out parameter is writable.
            unsafe {
                if (api.bounds)(
                    page,
                    TEXT_LINE,
                    &mut left,
                    &mut top,
                    &mut right,
                    &mut bottom,
                ) != 0
                {
                    let text = (api.iterator_text)(result, TEXT_LINE);
                    if !text.is_null() {
                        let owned = CStr::from_ptr(text).to_string_lossy().trim().to_owned();
                        (api.text_delete)(text);
                        if !owned.is_empty() {
                            lines.push(Line {
                                text: owned,
                                bounds: [left, top, right, bottom],
                            });
                        }
                    }
                }
                if (api.iterator_next)(result, TEXT_LINE) == 0 {
                    break;
                }
            }
        }
        lines
    }
}

unsafe extern "C" fn cancelled(context: *mut c_void, _words: c_int) -> bool {
    // SAFETY: Engine::lines passes its shared reference for synchronous recognition only.
    unsafe { context.cast::<Cancellation>().as_ref() }.is_some_and(Cancellation::is_cancelled)
}

struct Owned<T> {
    raw: NonNull<T>,
    delete: unsafe extern "C" fn(*mut T),
}

impl<T> Owned<T> {
    fn new(raw: *mut T, delete: unsafe extern "C" fn(*mut T)) -> Option<Self> {
        Some(Self {
            raw: NonNull::new(raw)?,
            delete,
        })
    }

    fn as_ptr(&self) -> *mut T {
        self.raw.as_ptr()
    }
}

impl<T> Drop for Owned<T> {
    fn drop(&mut self) {
        // SAFETY: each wrapper owns the handle and retains its matching destructor.
        unsafe { (self.delete)(self.raw.as_ptr()) }
    }
}
