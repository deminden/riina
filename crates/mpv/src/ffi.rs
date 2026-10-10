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

impl Node {
    /// Copy an event-owned node before the next wait_event invalidates its storage.
    /// Unsupported formats fail rather than interpreting an inactive union member.
    pub unsafe fn copy_json(&self) -> Result<serde_json::Value, String> {
        unsafe { self.copy_json_at(0) }
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
