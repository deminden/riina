//! File API bindings; PluginFiles owns path and permission validation.
use crate::plugin_files::{PluginFileHandle, PluginFiles};
use rquickjs::{Ctx, Object, function::Func};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
};
const MAX_OPEN_FILE_HANDLES: usize = 64;

pub(super) fn install_plugin_file_api<'js>(
    ctx: Ctx<'js>,
    globals: Object<'js>,
    file_system: PluginFiles,
    file_system_granted: bool,
) -> rquickjs::Result<()> {
    let native = Object::new(ctx.clone())?;
    let handles = Arc::new(Mutex::new(BTreeMap::<u32, PluginFileHandle>::new()));
    let next_handle = Arc::new(AtomicU32::new(1));
    let files = file_system.clone();
    let handles_for_open = handles.clone();
    let next_handle_for_open = next_handle.clone();
    native.set(
        "__open",
        Func::from(move |path: String, mode: String| -> rquickjs::Result<u32> {
            let handle = files
                .open_handle(&path, &mode, file_system_granted)
                .map_err(plugin_file_js_error)?;
            let id = next_handle_for_open.fetch_add(1, Ordering::Relaxed);
            let mut handles = handles_for_open
                .lock()
                .map_err(|_| plugin_file_js_error("File handle table is unavailable".into()))?;
            if handles.len() >= MAX_OPEN_FILE_HANDLES {
                return Err(plugin_file_js_error(
                    "Plugin file handle limit reached".into(),
                ));
            }
            handles.insert(id, handle);
            Ok(id)
        }),
    )?;
    let handles_for_offset = handles.clone();
    native.set(
        "__offset",
        Func::from(move |id: u32| -> rquickjs::Result<u64> {
            handles_for_offset
                .lock()
                .map_err(|_| plugin_file_js_error("File handle table is unavailable".into()))?
                .get_mut(&id)
                .ok_or_else(|| plugin_file_js_error("File handle is closed".into()))?
                .offset()
                .map_err(plugin_file_js_error)
        }),
    )?;
    let handles_for_seek = handles.clone();
    native.set(
        "__seek",
        Func::from(move |id: u32, offset: u64| -> rquickjs::Result<()> {
            handles_for_seek
                .lock()
                .map_err(|_| plugin_file_js_error("File handle table is unavailable".into()))?
                .get_mut(&id)
                .ok_or_else(|| plugin_file_js_error("File handle is closed".into()))?
                .seek_to(offset)
                .map_err(plugin_file_js_error)
        }),
    )?;
    let handles_for_seek_end = handles.clone();
    native.set(
        "__seekEnd",
        Func::from(move |id: u32| -> rquickjs::Result<()> {
            handles_for_seek_end
                .lock()
                .map_err(|_| plugin_file_js_error("File handle table is unavailable".into()))?
                .get_mut(&id)
                .ok_or_else(|| plugin_file_js_error("File handle is closed".into()))?
                .seek_to_end()
                .map_err(plugin_file_js_error)
        }),
    )?;
    let handles_for_read = handles.clone();
    native.set(
        "__readHandle",
        Func::from(move |id: u32, length: i64| -> rquickjs::Result<String> {
            let length = usize::try_from(length)
                .map_err(|_| plugin_file_js_error("Read length must be non-negative".into()))?;
            let bytes = handles_for_read
                .lock()
                .map_err(|_| plugin_file_js_error("File handle table is unavailable".into()))?
                .get_mut(&id)
                .ok_or_else(|| plugin_file_js_error("File handle is closed".into()))?
                .read(length)
                .map_err(plugin_file_js_error)?;
            serde_json::to_string(&bytes).map_err(|error| plugin_file_js_error(error.to_string()))
        }),
    )?;
    let handles_for_read_end = handles.clone();
    native.set(
        "__readHandleEnd",
        Func::from(move |id: u32| -> rquickjs::Result<String> {
            let bytes = handles_for_read_end
                .lock()
                .map_err(|_| plugin_file_js_error("File handle table is unavailable".into()))?
                .get_mut(&id)
                .ok_or_else(|| plugin_file_js_error("File handle is closed".into()))?
                .read_to_end()
                .map_err(plugin_file_js_error)?;
            serde_json::to_string(&bytes).map_err(|error| plugin_file_js_error(error.to_string()))
        }),
    )?;
    let handles_for_write = handles.clone();
    native.set(
        "__writeHandle",
        Func::from(
            move |id: u32, encoded_bytes: String| -> rquickjs::Result<()> {
                let bytes: Vec<u8> = serde_json::from_str(&encoded_bytes)
                    .map_err(|error| plugin_file_js_error(error.to_string()))?;
                handles_for_write
                    .lock()
                    .map_err(|_| plugin_file_js_error("File handle table is unavailable".into()))?
                    .get_mut(&id)
                    .ok_or_else(|| plugin_file_js_error("File handle is closed".into()))?
                    .write(&bytes)
                    .map_err(plugin_file_js_error)
            },
        ),
    )?;
    let handles_for_write_text = handles.clone();
    native.set(
        "__writeHandleText",
        Func::from(move |id: u32, content: String| -> rquickjs::Result<()> {
            handles_for_write_text
                .lock()
                .map_err(|_| plugin_file_js_error("File handle table is unavailable".into()))?
                .get_mut(&id)
                .ok_or_else(|| plugin_file_js_error("File handle is closed".into()))?
                .write(content.as_bytes())
                .map_err(plugin_file_js_error)
        }),
    )?;
    let handles_for_close = handles.clone();
    native.set(
        "__close",
        Func::from(move |id: u32| -> rquickjs::Result<()> {
            handles_for_close
                .lock()
                .map_err(|_| plugin_file_js_error("File handle table is unavailable".into()))?
                .remove(&id)
                .ok_or_else(|| plugin_file_js_error("File handle is closed".into()))?;
            Ok(())
        }),
    )?;
    let files = file_system.clone();
    native.set(
        "__exists",
        Func::from(move |path: String| -> rquickjs::Result<bool> {
            files
                .exists(&path, file_system_granted)
                .map_err(plugin_file_js_error)
        }),
    )?;
    let files = file_system.clone();
    native.set(
        "__showInFinder",
        Func::from(move |path: String| -> rquickjs::Result<()> {
            let path = files
                .existing_path(&path, file_system_granted)
                .map_err(plugin_file_js_error)?;
            crate::platform::reveal_file(&path)
                .map_err(|error| plugin_file_js_error(format!("Could not reveal file: {error}")))
        }),
    )?;
    let files = file_system.clone();
    native.set(
        "__read",
        Func::from(
            move |path: String, options: String| -> rquickjs::Result<String> {
                files
                    .read_text(&path, &options, file_system_granted)
                    .map_err(plugin_file_js_error)
            },
        ),
    )?;
    let files = file_system.clone();
    native.set(
        "__write",
        Func::from(
            move |path: String, content: String| -> rquickjs::Result<()> {
                files
                    .write_text(&path, &content, file_system_granted)
                    .map_err(plugin_file_js_error)
            },
        ),
    )?;
    let files = file_system.clone();
    native.set(
        "__delete",
        Func::from(move |path: String| -> rquickjs::Result<()> {
            files.delete(&path).map_err(plugin_file_js_error)
        }),
    )?;
    let files = file_system.clone();
    native.set(
        "__trash",
        Func::from(move |path: String| -> rquickjs::Result<()> {
            files
                .trash(&path, file_system_granted)
                .map_err(plugin_file_js_error)
        }),
    )?;
    let files = file_system.clone();
    native.set(
        "__move",
        Func::from(
            move |source: String, destination: String| -> rquickjs::Result<()> {
                files
                    .move_path(&source, &destination, file_system_granted)
                    .map_err(plugin_file_js_error)
            },
        ),
    )?;
    native.set(
        "__list",
        Func::from(
            move |path: String, options: String| -> rquickjs::Result<String> {
                file_system
                    .list(&path, &options, file_system_granted)
                    .map_err(plugin_file_js_error)
            },
        ),
    )?;
    globals.set("__riinaFile", native)?;
    ctx.eval::<(), _>(
        r#"(() => {
            const native = __riinaFile;
            iina.file = {
                exists: path => native.__exists(String(path)),
                showInFinder: path => native.__showInFinder(String(path)),
                read: (path, options = {}) => native.__read(String(path), JSON.stringify(options)),
                write: (path, content) => native.__write(String(path), String(content)),
                delete: path => native.__delete(String(path)),
                trash: path => native.__trash(String(path)),
                move: (source, destination) => native.__move(String(source), String(destination)),
                list: (path, options = {}) => JSON.parse(native.__list(String(path), JSON.stringify(options))),
                handle: (path, mode) => {
                    const id = native.__open(String(path), String(mode));
                    return {
                        offset: () => native.__offset(id),
                        seekTo: offset => native.__seek(id, Number(offset)),
                        seekToEnd: () => native.__seekEnd(id),
                        read: length => new Uint8Array(JSON.parse(native.__readHandle(id, Number(length)))),
                        readToEnd: () => new Uint8Array(JSON.parse(native.__readHandleEnd(id))),
                        write: data => {
                            if (typeof data === 'string') return native.__writeHandleText(id, data);
                            if (data && data.length > 4194304) throw new Error('Handle writes are limited to 4 MiB per call');
                            const bytes = Array.from(data);
                            if (bytes.length > 4194304) throw new Error('Handle writes are limited to 4 MiB per call');
                            return native.__writeHandle(id, JSON.stringify(bytes));
                        },
                        close: () => native.__close(id)
                    };
                }
            };
            globalThis.__riinaFile = undefined;
        })();"#,
    )?;
    Ok(())
}

fn plugin_file_js_error(message: String) -> rquickjs::Error {
    rquickjs::Error::new_from_js_message("plugin file", "an allowed plugin path", message)
}
