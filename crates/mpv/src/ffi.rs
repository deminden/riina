//! Hand-written ABI subset, checked against IINA's pinned deps/include/mpv headers.
//! No foreign event pointer escapes the call to `mpv_wait_event` that owns it.

use crate::EngineError;
mod node_input;
use libloading::Library;
pub(crate) use node_input::{property_string, with_node};
use std::{
    ffi::{CStr, c_char, c_int, c_void},
    path::Path,
    sync::Arc,
};

// Keep C discriminants as integers so unknown values from newer runtimes stay readable.
pub(crate) mod event_id {
    pub const NONE: std::ffi::c_int = 0;
    pub const SHUTDOWN: std::ffi::c_int = 1;
    pub const GET_PROPERTY_REPLY: std::ffi::c_int = 3;
    pub const SET_PROPERTY_REPLY: std::ffi::c_int = 4;
    pub const COMMAND_REPLY: std::ffi::c_int = 5;
    pub const START_FILE: std::ffi::c_int = 6;
    pub const END_FILE: std::ffi::c_int = 7;
    pub const FILE_LOADED: std::ffi::c_int = 8;
    pub const SEEK: std::ffi::c_int = 20;
    pub const PLAYBACK_RESTART: std::ffi::c_int = 21;
    pub const PROPERTY_CHANGE: std::ffi::c_int = 22;
    pub const QUEUE_OVERFLOW: std::ffi::c_int = 24;
    pub const HOOK: std::ffi::c_int = 25;
}

pub(crate) mod format {
    pub const NONE: std::ffi::c_int = 0;
    pub const STRING: std::ffi::c_int = 1;
    pub const FLAG: std::ffi::c_int = 3;
    pub const INT64: std::ffi::c_int = 4;
    pub const DOUBLE: std::ffi::c_int = 5;
    pub const NODE: std::ffi::c_int = 6;
    pub const NODE_ARRAY: std::ffi::c_int = 7;
    pub const NODE_MAP: std::ffi::c_int = 8;
    pub const BYTE_ARRAY: std::ffi::c_int = 9;
}

pub(crate) mod end_reason {
    pub const EOF: std::ffi::c_int = 0;
    pub const ERROR: std::ffi::c_int = 4;
    pub const REDIRECT: std::ffi::c_int = 5;
}

pub(crate) mod render_param {
    pub const INVALID: std::ffi::c_int = 0;
    pub const API_TYPE: std::ffi::c_int = 1;
    pub const OPENGL_INIT_PARAMS: std::ffi::c_int = 2;
    pub const OPENGL_FBO: std::ffi::c_int = 3;
    pub const FLIP_Y: std::ffi::c_int = 4;
    pub const ICC_PROFILE: std::ffi::c_int = 6;
}

#[repr(C)]
pub struct Event {
    pub id: c_int,
    pub error: c_int,
    pub userdata: u64,
    pub data: *mut c_void,
}

#[repr(C)]
pub struct HookEvent {
    pub name: *const c_char,
    pub id: u64,
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

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum NodeDecodeError {
    NestingLimit,
    InvalidCount,
    MissingStorage,
    MissingKey,
    UnsupportedFormat(c_int),
}

impl std::fmt::Display for NodeDecodeError {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NestingLimit => output.write_str("mpv property nesting exceeds 32 levels"),
            Self::InvalidCount => output.write_str("Invalid mpv node count"),
            Self::MissingStorage => output.write_str("Missing mpv node storage"),
            Self::MissingKey => output.write_str("Missing mpv node key"),
            Self::UnsupportedFormat(format) => {
                write!(output, "Unsupported mpv node format {format}")
            }
        }
    }
}

impl std::error::Error for NodeDecodeError {}

impl Node {
    /// Copy an event-owned node before the next wait_event invalidates its storage.
    /// Unsupported formats fail rather than interpreting an inactive union member.
    pub(crate) unsafe fn copy_json(&self) -> Result<serde_json::Value, NodeDecodeError> {
        // SAFETY: the caller keeps the mpv node and its referenced data live for the copy.
        unsafe { self.copy_json_at(0) }
    }

    /// Copies a screenshot-raw reply while its command event still owns the node.
    pub unsafe fn copy_rgba_frame(&self) -> Result<riina_core::CapturedFrame, EngineError> {
        let list = if self.format == format::NODE_MAP {
            // SAFETY: MPV_FORMAT_NODE_MAP selects the list arm of this live reply.
            unsafe { self.value.list }
        } else {
            std::ptr::null()
        };
        if list.is_null() {
            return Err(EngineError::InvalidFrameReply {
                reason: "mpv returned an invalid screenshot-raw map",
            });
        }
        // SAFETY: the caller guarantees this is the live event-owned reply node.
        let list = unsafe { &*list };
        if !(0..=64).contains(&list.count)
            || (list.count > 0 && (list.values.is_null() || list.keys.is_null()))
        {
            return Err(EngineError::InvalidFrameReply {
                reason: "mpv returned incomplete screenshot-raw fields",
            });
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
                return Err(EngineError::InvalidFrameReply {
                    reason: "mpv returned a null screenshot-raw key",
                });
            }
            // SAFETY: mpv node-map keys are NUL-terminated for the event lifetime.
            let key = unsafe { CStr::from_ptr(key) }.to_bytes();
            // SAFETY: every listed node is readable until the next wait_event call.
            let value = unsafe { &*list.values.add(index) };
            match key {
                b"w" if value.format == format::INT64 => {
                    // SAFETY: the width node is live and tagged MPV_FORMAT_INT64.
                    width = Some(unsafe { value.value.integer });
                }
                b"h" if value.format == format::INT64 => {
                    // SAFETY: the height node is live and tagged MPV_FORMAT_INT64.
                    height = Some(unsafe { value.value.integer });
                }
                b"stride" if value.format == format::INT64 => {
                    // SAFETY: the stride node is live and tagged MPV_FORMAT_INT64.
                    stride = Some(unsafe { value.value.integer });
                }
                b"format" if value.format == format::STRING => {
                    // SAFETY: MPV_FORMAT_STRING selects the string arm of the live node.
                    let text = unsafe { value.value.string };
                    if !text.is_null() {
                        // SAFETY: mpv keeps this NUL-terminated string alive for the reply.
                        format = Some(unsafe { CStr::from_ptr(text) }.to_bytes());
                    }
                }
                b"data" if value.format == format::BYTE_ARRAY => {
                    // SAFETY: MPV_FORMAT_BYTE_ARRAY selects this pointer arm.
                    let array = unsafe { value.value.byte_array };
                    if !array.is_null() {
                        // SAFETY: the non-null descriptor belongs to the live command reply.
                        bytes = Some(unsafe { &*array });
                    }
                }
                _ => {}
            }
        }
        if format != Some(b"rgba".as_slice()) {
            return Err(EngineError::InvalidFrameReply {
                reason: "mpv returned an unexpected screenshot pixel format",
            });
        }
        let width = width.and_then(|value| u32::try_from(value).ok()).ok_or(
            EngineError::InvalidFrameReply {
                reason: "mpv returned an invalid screenshot width",
            },
        )?;
        let height = height.and_then(|value| u32::try_from(value).ok()).ok_or(
            EngineError::InvalidFrameReply {
                reason: "mpv returned an invalid screenshot height",
            },
        )?;
        let stride = stride.and_then(|value| usize::try_from(value).ok()).ok_or(
            EngineError::InvalidFrameReply {
                reason: "mpv returned an invalid screenshot stride",
            },
        )?;
        let bytes = bytes.ok_or(EngineError::InvalidFrameReply {
            reason: "mpv returned no screenshot pixels",
        })?;
        if bytes.size > 512 * 1024 * 1024 || bytes.data.is_null() {
            return Err(EngineError::InvalidFrameReply {
                reason: "mpv returned an invalid screenshot buffer",
            });
        }
        // SAFETY: screenshot-raw owns `size` readable bytes until the next wait_event.
        let pixels = unsafe { std::slice::from_raw_parts(bytes.data.cast::<u8>(), bytes.size) };
        riina_core::CapturedFrame::new(width, height, stride, pixels).map_err(EngineError::Frame)
    }

    unsafe fn copy_json_at(&self, depth: usize) -> Result<serde_json::Value, NodeDecodeError> {
        use serde_json::Value;
        if depth > 32 {
            return Err(NodeDecodeError::NestingLimit);
        }
        // SAFETY: caller provides a live mpv node; each union access follows its tag.
        unsafe {
            Ok(match self.format {
                format::NONE => Value::Null,
                format::STRING if !self.value.string.is_null() => Value::String(
                    CStr::from_ptr(self.value.string)
                        .to_string_lossy()
                        .into_owned(),
                ),
                format::FLAG => Value::Bool(self.value.flag != 0),
                format::INT64 => Value::Number(self.value.integer.into()),
                format::DOUBLE => serde_json::Number::from_f64(self.value.double)
                    .map(Value::Number)
                    // A native VO can publish 0/0 aspect/par in its color-only
                    // target parameters. Preserve that as unknown, not a clock
                    // value or a playback failure. Command validation stays strict.
                    .unwrap_or(Value::Null),
                format::NODE_ARRAY | format::NODE_MAP if !self.value.list.is_null() => {
                    let list = &*self.value.list;
                    if !(0..=100_000).contains(&list.count) {
                        return Err(NodeDecodeError::InvalidCount);
                    }
                    if list.count > 0
                        && (list.values.is_null()
                            || (self.format == format::NODE_MAP && list.keys.is_null()))
                    {
                        return Err(NodeDecodeError::MissingStorage);
                    }
                    let mut array = Vec::new();
                    let mut map = serde_json::Map::new();
                    for index in 0..list.count as usize {
                        let value = (&*list.values.add(index)).copy_json_at(depth + 1)?;
                        if self.format == format::NODE_ARRAY {
                            array.push(value);
                        } else {
                            let key = *list.keys.add(index);
                            if key.is_null() {
                                return Err(NodeDecodeError::MissingKey);
                            }
                            map.insert(CStr::from_ptr(key).to_string_lossy().into_owned(), value);
                        }
                    }
                    if self.format == format::NODE_ARRAY {
                        Value::Array(array)
                    } else {
                        Value::Object(map)
                    }
                }
                _ => return Err(NodeDecodeError::UnsupportedFormat(self.format)),
            })
        }
    }
}

#[repr(C)]
pub struct StartFile {
    pub playlist_entry_id: i64,
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
            kind: render_param::INVALID,
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

#[repr(C)]
pub struct ByteArray {
    pub data: *mut c_void,
    pub size: usize,
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
    pub get_property: unsafe extern "C" fn(*mut c_void, u64, *const c_char, c_int) -> c_int,
    pub set_property:
        unsafe extern "C" fn(*mut c_void, u64, *const c_char, c_int, *mut c_void) -> c_int,
    pub observe: unsafe extern "C" fn(*mut c_void, u64, *const c_char, c_int) -> c_int,
    pub hook_add: unsafe extern "C" fn(*mut c_void, u64, *const c_char, c_int) -> c_int,
    pub hook_continue: unsafe extern "C" fn(*mut c_void, u64) -> c_int,
    pub wait: unsafe extern "C" fn(*mut c_void, f64) -> *const Event,
    pub event_name: unsafe extern "C" fn(c_int) -> *const c_char,
    pub wakeup:
        unsafe extern "C" fn(*mut c_void, Option<unsafe extern "C" fn(*mut c_void)>, *mut c_void),
    pub error: unsafe extern "C" fn(c_int) -> *const c_char,
    pub render_create: unsafe extern "C" fn(*mut *mut c_void, *mut c_void, *mut Param) -> c_int,
    pub render: unsafe extern "C" fn(*mut c_void, *mut Param) -> c_int,
    pub render_set_parameter: Option<unsafe extern "C" fn(*mut c_void, Param) -> c_int>,
    pub render_callback:
        unsafe extern "C" fn(*mut c_void, Option<unsafe extern "C" fn(*mut c_void)>, *mut c_void),
    pub render_report_swap: unsafe extern "C" fn(*mut c_void),
    pub render_free: unsafe extern "C" fn(*mut c_void),
}

impl Api {
    pub fn load(path: &Path) -> Result<Self, EngineError> {
        if !path.is_absolute() || !path.is_file() {
            return Err(EngineError::LibraryMissing(path.to_path_buf()));
        }
        // IINA links the engine for the application's lifetime. Retain the same
        // ownership boundary here: closing a player destroys its core, while
        // module-level codec state and foreign thread-local destructors remain
        // backed by a loaded library until process exit.
        static LOADED: std::sync::OnceLock<
            std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, Api>>,
        > = std::sync::OnceLock::new();
        let path = path.canonicalize().map_err(EngineError::LibraryPath)?;
        let mut loaded = LOADED
            .get_or_init(Default::default)
            .lock()
            .map_err(|_| EngineError::LibraryRegistryUnavailable)?;
        if let Some(api) = loaded.get(&path) {
            return Ok(api.clone());
        }
        let api = Self::load_uncached(&path)?;
        loaded.insert(path, api.clone());
        Ok(api)
    }

    fn load_uncached(path: &Path) -> Result<Self, EngineError> {
        // SAFETY: explicit absolute installation path; never search a media directory.
        // The selected binary's ABI is checked before any player handle is created.
        unsafe {
            #[cfg(windows)]
            let library: Library = libloading::os::windows::Library::load_with_flags(
                path,
                0x00000100 | 0x00000800, // DLL_LOAD_DIR | SYSTEM32
            )
            .map_err(EngineError::LibraryLoad)?
            .into();
            #[cfg(not(windows))]
            let library = Library::new(path).map_err(EngineError::LibraryLoad)?;
            let library = Arc::new(library);
            let version = *library
                .get::<unsafe extern "C" fn() -> u64>(b"mpv_client_api_version\0")
                .map_err(EngineError::LibraryLoad)?;
            let version = version();
            if version >> 16 != 2 || version & 0xffff < 5 {
                return Err(EngineError::UnsupportedApi(version));
            }
            macro_rules! symbol {
                ($name:literal) => {
                    *library
                        .get(concat!($name, "\0").as_bytes())
                        .map_err(EngineError::LibraryLoad)?
                };
            }
            Ok(Self {
                create: symbol!("mpv_create"),
                initialize: symbol!("mpv_initialize"),
                destroy: symbol!("mpv_terminate_destroy"),
                option: symbol!("mpv_set_option_string"),
                command: symbol!("mpv_command_async"),
                get_property: symbol!("mpv_get_property_async"),
                set_property: symbol!("mpv_set_property_async"),
                observe: symbol!("mpv_observe_property"),
                hook_add: symbol!("mpv_hook_add"),
                hook_continue: symbol!("mpv_hook_continue"),
                wait: symbol!("mpv_wait_event"),
                event_name: symbol!("mpv_event_name"),
                wakeup: symbol!("mpv_set_wakeup_callback"),
                error: symbol!("mpv_error_string"),
                render_create: symbol!("mpv_render_context_create"),
                render: symbol!("mpv_render_context_render"),
                render_set_parameter: library
                    .get::<unsafe extern "C" fn(*mut c_void, Param) -> c_int>(
                        b"mpv_render_context_set_parameter\0",
                    )
                    .ok()
                    .map(|symbol| *symbol),
                render_callback: symbol!("mpv_render_context_set_update_callback"),
                render_report_swap: symbol!("mpv_render_context_report_swap"),
                render_free: symbol!("mpv_render_context_free"),
                _library: library,
            })
        }
    }

    pub fn check(&self, call: &'static str, result: c_int) -> Result<(), EngineError> {
        if result >= 0 {
            return Ok(());
        }
        // SAFETY: mpv_error_string returns an immutable process-lifetime C string.
        let message = unsafe { CStr::from_ptr((self.error)(result)) }
            .to_string_lossy()
            .into_owned();
        Err(EngineError::Native { call, message })
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
                format: format::INT64,
            },
            Node {
                value: NodeValue { integer: 1 },
                format: format::INT64,
            },
            Node {
                value: NodeValue { integer: 8 },
                format: format::INT64,
            },
            Node {
                value: NodeValue {
                    string: format.as_ptr(),
                },
                format: format::STRING,
            },
            Node {
                value: NodeValue {
                    byte_array: &byte_array,
                },
                format: format::BYTE_ARRAY,
            },
        ];
        let list = NodeList {
            count: values.len() as c_int,
            values: values.as_ptr(),
            keys: keys.as_ptr(),
        };
        let result = Node {
            value: NodeValue { list: &list },
            format: format::NODE_MAP,
        };

        // SAFETY: all map keys, values, and pixel storage outlive the copy.
        let frame = unsafe { result.copy_rgba_frame() }.unwrap();
        assert_eq!((frame.width, frame.height), (2, 1));
        assert_eq!(&*frame.rgba, &pixels);
    }
}

#[cfg(test)]
mod generated_nodes;

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn used_c_structs_match_the_pinned_headers_on_64_bit_targets() {
        use std::mem::{offset_of, size_of};

        assert_eq!(size_of::<Event>(), 24);
        assert_eq!(
            [
                offset_of!(Event, id),
                offset_of!(Event, error),
                offset_of!(Event, userdata),
                offset_of!(Event, data)
            ],
            [0, 4, 8, 16]
        );
        assert_eq!(size_of::<Property>(), 24);
        assert_eq!(
            [
                offset_of!(Property, name),
                offset_of!(Property, format),
                offset_of!(Property, data)
            ],
            [0, 8, 16]
        );
        assert_eq!(size_of::<EndFile>(), 32);
        assert_eq!(
            [
                offset_of!(EndFile, reason),
                offset_of!(EndFile, error),
                offset_of!(EndFile, playlist_entry_id),
                offset_of!(EndFile, playlist_insert_id),
                offset_of!(EndFile, playlist_insert_num_entries),
            ],
            [0, 4, 8, 16, 24]
        );
        assert_eq!(size_of::<NodeByteArray>(), 16);
        assert_eq!(
            [
                offset_of!(NodeByteArray, data),
                offset_of!(NodeByteArray, size)
            ],
            [0, 8]
        );
        assert_eq!(size_of::<Param>(), 16);
        assert_eq!([offset_of!(Param, kind), offset_of!(Param, data)], [0, 8]);
        assert_eq!(size_of::<Fbo>(), 16);
        assert_eq!(
            [
                offset_of!(Fbo, fbo),
                offset_of!(Fbo, width),
                offset_of!(Fbo, height),
                offset_of!(Fbo, format)
            ],
            [0, 4, 8, 12]
        );
    }

    #[test]
    fn undefined_native_target_numbers_remain_unknown() {
        for double in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let node = Node {
                value: NodeValue { double },
                format: format::DOUBLE,
            };
            // SAFETY: this local node contains an active double, just as a
            // color-only native target's undefined aspect-ratio property does.
            let value = unsafe { node.copy_json() }.unwrap();
            assert_eq!(value, serde_json::Value::Null);
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
            format: format::NODE_ARRAY,
        };
        // SAFETY: list remains live; zero entries permit null values in client.h.
        assert_eq!(unsafe { node.copy_json() }.unwrap(), serde_json::json!([]));
        let unknown = Node {
            value: NodeValue { integer: 0 },
            format: 999,
        };
        // SAFETY: the local node has an unsupported tag, so the copy reads no union member.
        let result = unsafe { unknown.copy_json() };
        assert_eq!(
            result,
            Err(NodeDecodeError::UnsupportedFormat(unknown.format))
        );
    }
}
