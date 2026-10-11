//! Hand-written ABI subset, checked against IINA's pinned deps/include/mpv headers.
//! No foreign event pointer escapes the call to `mpv_wait_event` that owns it.

use libloading::Library;
use std::{
    ffi::{CStr, c_char, c_int, c_void},
    path::Path,
    sync::Arc,
};

#[repr(C)]
pub struct Event {
    pub id: c_int,
    pub error: c_int,
    pub userdata: u64,
    pub data: *mut c_void,
}

#[repr(C)]
pub struct Property {
    pub name: *const c_char,
    pub format: c_int,
    pub data: *mut c_void,
}

#[repr(C)]
pub union NodeValue {
    pub string: *const c_char,
    pub flag: c_int,
    pub integer: i64,
    pub double: f64,
    pub list: *const NodeList,
    pub byte_array: *const NodeByteArray,
}

#[repr(C)]
pub struct Node {
    pub value: NodeValue,
    pub format: c_int,
}

#[repr(C)]
pub struct CommandReply {
    pub result: Node,
}

#[repr(C)]
pub struct NodeList {
    pub count: c_int,
    pub values: *const Node,
    pub keys: *const *const c_char,
}

#[repr(C)]
pub struct NodeByteArray {
    pub data: *mut c_void,
    pub size: usize,
}

impl Node {
    /// Copy an event-owned node before the next wait_event invalidates its storage.
    /// Unsupported formats fail rather than interpreting an inactive union member.
    pub unsafe fn copy_json(&self) -> Result<serde_json::Value, String> {
        unsafe { self.copy_json_at(0) }
    }

    /// Copies a screenshot-raw reply while its command event still owns the node.
    pub unsafe fn copy_rgba_frame(&self) -> Result<riina_core::CapturedFrame, String> {
        // SAFETY: `format` selects the map arm before the union pointer is read.
        let list = if self.format == 8 {
            unsafe { self.value.list }
        } else {
            std::ptr::null()
        };
        if list.is_null() {
            return Err("mpv returned an invalid screenshot-raw map".into());
        }
        // SAFETY: the caller guarantees this is the live event-owned reply node.
        let list = unsafe { &*list };
        if !(0..=64).contains(&list.count)
            || (list.count > 0 && (list.values.is_null() || list.keys.is_null()))
        {
            return Err("mpv returned incomplete screenshot-raw fields".into());
        }

        let mut width = None;
        let mut height = None;
        let mut stride = None;
        let mut format = None;
        let mut bytes = None;
        for index in 0..list.count as usize {
            // SAFETY: every key/value lies in the validated event-owned node list.
            let key = unsafe { *list.keys.add(index) };
            if key.is_null() {
                return Err("mpv returned a null screenshot-raw key".into());
            }
            // SAFETY: mpv node-map keys are NUL-terminated for the event lifetime.
            let key = unsafe { CStr::from_ptr(key) }.to_bytes();
            // SAFETY: every listed node is readable until the next wait_event call.
            let value = unsafe { &*list.values.add(index) };
            match key {
                b"w" if value.format == 4 => width = Some(unsafe { value.value.integer }),
                b"h" if value.format == 4 => height = Some(unsafe { value.value.integer }),
                b"stride" if value.format == 4 => {
                    stride = Some(unsafe { value.value.integer });
                }
                b"format" if value.format == 1 && !unsafe { value.value.string }.is_null() => {
                    format = Some(unsafe { CStr::from_ptr(value.value.string) }.to_bytes());
                }
                b"data" if value.format == 9 && !unsafe { value.value.byte_array }.is_null() => {
                    bytes = Some(unsafe { &*value.value.byte_array });
                }
                _ => {}
            }
        }
        if format != Some(b"rgba".as_slice()) {
            return Err("mpv returned an unexpected screenshot pixel format".into());
        }
        let width = width
            .and_then(|value| u32::try_from(value).ok())
            .ok_or("mpv returned an invalid screenshot width")?;
        let height = height
            .and_then(|value| u32::try_from(value).ok())
            .ok_or("mpv returned an invalid screenshot height")?;
        let stride = stride
            .and_then(|value| usize::try_from(value).ok())
            .ok_or("mpv returned an invalid screenshot stride")?;
        let bytes = bytes.ok_or("mpv returned no screenshot pixels")?;
        if bytes.size > 512 * 1024 * 1024 || bytes.data.is_null() {
            return Err("mpv returned an invalid screenshot buffer".into());
        }
        // SAFETY: screenshot-raw owns `size` readable bytes until the next wait_event.
        let pixels = unsafe { std::slice::from_raw_parts(bytes.data.cast::<u8>(), bytes.size) };
        riina_core::CapturedFrame::new(width, height, stride, pixels)
    }

    unsafe fn copy_json_at(&self, depth: usize) -> Result<serde_json::Value, String> {
        use serde_json::Value;
        if depth > 32 {
            return Err("mpv property nesting exceeds 32 levels".into());
        }
        // SAFETY: caller provides a live mpv node; each union access follows its tag.
        unsafe {
            Ok(match self.format {
                0 => Value::Null,
                1 if !self.value.string.is_null() => Value::String(
                    CStr::from_ptr(self.value.string)
                        .to_string_lossy()
                        .into_owned(),
                ),
                3 => Value::Bool(self.value.flag != 0),
                4 => Value::Number(self.value.integer.into()),
                5 => serde_json::Number::from_f64(self.value.double)
                    .map(Value::Number)
                    // A native VO can publish 0/0 aspect/par in its color-only
                    // target parameters. Preserve that as unknown, not a clock
                    // value or a playback failure. Command validation stays strict.
                    .unwrap_or(Value::Null),
                7 | 8 if !self.value.list.is_null() => {
                    let list = &*self.value.list;
                    if !(0..=100_000).contains(&list.count) {
                        return Err("Invalid mpv node count".into());
                    }
                    if list.count > 0
                        && (list.values.is_null() || (self.format == 8 && list.keys.is_null()))
                    {
                        return Err("Missing mpv node storage".into());
                    }
                    let mut array = Vec::new();
                    let mut map = serde_json::Map::new();
                    for index in 0..list.count as usize {
                        let value = (&*list.values.add(index)).copy_json_at(depth + 1)?;
                        if self.format == 7 {
                            array.push(value);
                        } else {
                            let key = *list.keys.add(index);
                            if key.is_null() {
                                return Err("Missing mpv node key".into());
                            }
                            map.insert(CStr::from_ptr(key).to_string_lossy().into_owned(), value);
                        }
                    }
                    if self.format == 7 {
                        Value::Array(array)
                    } else {
                        Value::Object(map)
                    }
                }
                _ => return Err(format!("Unsupported mpv node format {}", self.format)),
            })
        }
    }
}

#[repr(C)]
pub struct EndFile {
    pub reason: c_int,
    pub error: c_int,
    pub playlist_entry_id: i64,
    pub playlist_insert_id: i64,
    pub playlist_insert_num_entries: c_int,
}

#[repr(C)]
pub struct Param {
    pub kind: c_int,
    pub data: *mut c_void,
}

impl Param {
    pub fn end() -> Self {
        Self {
            kind: 0,
            data: std::ptr::null_mut(),
        }
    }
}

#[repr(C)]
pub struct GlInit {
    pub get_proc: unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void,
    pub context: *mut c_void,
}

#[repr(C)]
pub struct Fbo {
    pub fbo: c_int,
    pub width: c_int,
    pub height: c_int,
    pub format: c_int,
}

#[derive(Clone)]
pub struct Api {
    // Holding the library keeps every copied function pointer valid through renderer teardown.
    pub _library: Arc<Library>,
    pub create: unsafe extern "C" fn() -> *mut c_void,
    pub initialize: unsafe extern "C" fn(*mut c_void) -> c_int,
    pub destroy: unsafe extern "C" fn(*mut c_void),
    pub option: unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int,
    pub command: unsafe extern "C" fn(*mut c_void, u64, *const *const c_char) -> c_int,
    pub observe: unsafe extern "C" fn(*mut c_void, u64, *const c_char, c_int) -> c_int,
    pub wait: unsafe extern "C" fn(*mut c_void, f64) -> *const Event,
    pub wakeup:
        unsafe extern "C" fn(*mut c_void, Option<unsafe extern "C" fn(*mut c_void)>, *mut c_void),
    pub error: unsafe extern "C" fn(c_int) -> *const c_char,
    pub render_create: unsafe extern "C" fn(*mut *mut c_void, *mut c_void, *mut Param) -> c_int,
    pub render: unsafe extern "C" fn(*mut c_void, *mut Param) -> c_int,
    pub render_callback:
        unsafe extern "C" fn(*mut c_void, Option<unsafe extern "C" fn(*mut c_void)>, *mut c_void),
    pub render_free: unsafe extern "C" fn(*mut c_void),
}

impl Api {
    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.is_absolute() || !path.is_file() {
            return Err(format!(
                "libmpv is missing at {}. Run scripts/fetch-mpv.ps1.",
                path.display()
            ));
        }
        // IINA links the engine for the application's lifetime. Retain the same
        // ownership boundary here: closing a player destroys its core, while
        // module-level codec state and foreign thread-local destructors remain
        // backed by a loaded library until process exit.
        static LOADED: std::sync::OnceLock<
            std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, Api>>,
        > = std::sync::OnceLock::new();
        let path = path.canonicalize().map_err(|error| error.to_string())?;
        let mut loaded = LOADED
            .get_or_init(Default::default)
            .lock()
            .map_err(|_| "The native engine library registry is unavailable".to_owned())?;
        if let Some(api) = loaded.get(&path) {
            return Ok(api.clone());
        }
        let api = Self::load_uncached(&path)?;
        loaded.insert(path, api.clone());
        Ok(api)
    }

    fn load_uncached(path: &Path) -> Result<Self, String> {
        // SAFETY: explicit absolute installation path; never search a media directory.
        // The selected binary's ABI is checked before any player handle is created.
        unsafe {
            #[cfg(windows)]
            let library: Library = libloading::os::windows::Library::load_with_flags(
                path,
                0x00000100 | 0x00000800, // DLL_LOAD_DIR | SYSTEM32
            )
            .map_err(|e| e.to_string())?
            .into();
            #[cfg(not(windows))]
            let library = Library::new(path).map_err(|e| e.to_string())?;
            let library = Arc::new(library);
            let version = *library
                .get::<unsafe extern "C" fn() -> u64>(b"mpv_client_api_version\0")
                .map_err(|e| e.to_string())?;
            let version = version();
            if version >> 16 != 2 || version & 0xffff < 5 {
                return Err(format!(
                    "Unsupported libmpv client API {version:#x}; require 2.5 or a compatible newer 2.x"
                ));
            }
            macro_rules! symbol {
                ($name:literal) => {
                    *library
                        .get(concat!($name, "\0").as_bytes())
                        .map_err(|e| e.to_string())?
                };
            }
            Ok(Self {
                create: symbol!("mpv_create"),
                initialize: symbol!("mpv_initialize"),
                destroy: symbol!("mpv_terminate_destroy"),
                option: symbol!("mpv_set_option_string"),
                command: symbol!("mpv_command_async"),
                observe: symbol!("mpv_observe_property"),
                wait: symbol!("mpv_wait_event"),
                wakeup: symbol!("mpv_set_wakeup_callback"),
                error: symbol!("mpv_error_string"),
                render_create: symbol!("mpv_render_context_create"),
                render: symbol!("mpv_render_context_render"),
                render_callback: symbol!("mpv_render_context_set_update_callback"),
                render_free: symbol!("mpv_render_context_free"),
                _library: library,
            })
        }
    }

    pub fn check(&self, result: c_int) -> Result<(), String> {
        if result >= 0 {
            return Ok(());
        }
        // SAFETY: mpv_error_string returns an immutable process-lifetime C string.
        Err(unsafe { CStr::from_ptr((self.error)(result)) }
            .to_string_lossy()
            .into_owned())
    }
}

#[cfg(test)]
mod raw_frame_tests {
    use super::*;

    #[test]
    fn screenshot_raw_node_copies_typed_rgba_bytes() {
        let key_strings = ["w", "h", "stride", "format", "data"]
            .map(|value| std::ffi::CString::new(value).unwrap());
        let keys = key_strings.each_ref().map(|value| value.as_ptr());
        let format = std::ffi::CString::new("rgba").unwrap();
        let pixels = [1, 2, 3, 4, 5, 6, 7, 8];
        let byte_array = NodeByteArray {
            data: pixels.as_ptr().cast_mut().cast(),
            size: pixels.len(),
        };
        let values = [
            Node {
                value: NodeValue { integer: 2 },
                format: 4,
            },
            Node {
                value: NodeValue { integer: 1 },
                format: 4,
            },
            Node {
                value: NodeValue { integer: 8 },
                format: 4,
            },
            Node {
                value: NodeValue {
                    string: format.as_ptr(),
                },
                format: 1,
            },
            Node {
                value: NodeValue {
                    byte_array: &byte_array,
                },
                format: 9,
            },
        ];
        let list = NodeList {
            count: values.len() as c_int,
            values: values.as_ptr(),
            keys: keys.as_ptr(),
        };
        let result = Node {
            value: NodeValue { list: &list },
            format: 8,
        };

        // SAFETY: all map keys, values, and pixel storage outlive the copy.
        let frame = unsafe { result.copy_rgba_frame() }.unwrap();
        assert_eq!((frame.width, frame.height), (2, 1));
        assert_eq!(&*frame.rgba, &pixels);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undefined_native_target_numbers_remain_unknown() {
        for double in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let node = Node {
                value: NodeValue { double },
                format: 5,
            };
            // SAFETY: this local node contains an active double, just as a
            // color-only native target's undefined aspect-ratio property does.
            assert_eq!(
                unsafe { node.copy_json() }.unwrap(),
                serde_json::Value::Null
            );
        }
    }

    #[test]
    fn node_layout_matches_x64_client_header() {
        assert_eq!(std::mem::size_of::<Node>(), 16);
        assert_eq!(std::mem::offset_of!(Node, format), 8);
        assert_eq!(std::mem::size_of::<NodeList>(), 24);
        assert_eq!(std::mem::offset_of!(NodeList, values), 8);
    }

    #[test]
    fn empty_list_and_unknown_tag_do_not_read_inactive_storage() {
        let list = NodeList {
            count: 0,
            values: std::ptr::null(),
            keys: std::ptr::null(),
        };
        let node = Node {
            value: NodeValue { list: &list },
            format: 7,
        };
        // SAFETY: list remains live; zero entries permit null values in client.h.
        assert_eq!(unsafe { node.copy_json() }.unwrap(), serde_json::json!([]));
        let unknown = Node {
            value: NodeValue { integer: 0 },
            format: 999,
        };
        assert!(unsafe { unknown.copy_json() }.is_err());
    }
}
