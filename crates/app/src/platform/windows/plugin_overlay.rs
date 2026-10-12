//! Permission-gated plugin webviews layered over the native video host.

use crate::plugin_host::{PluginHostHandle, PluginHostMessage};
use riina_core::plugin::OverlayEffect;
use serde::Deserialize;
use slint::winit_030::winit::window::Window;
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use url::Url;
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    Graphics::Gdi::ScreenToClient,
    UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{GetClientRect, GetCursorPos, HTTRANSPARENT, WM_NCHITTEST},
    },
};
use wry::{
    NewWindowResponse, PageLoadEvent, Rect, WebView, WebViewBuilder, WebViewExtWindows,
    dpi::{LogicalPosition, LogicalSize},
    http::{Request, Response, StatusCode},
};

const OVERLAY_SCHEME: &str = "riina-plugin";
const MAX_IPC_BYTES: usize = 1024 * 1024 + 512;
const MAX_IPC_NAME_BYTES: usize = 256;
const MAX_QUEUED_MESSAGES: usize = 128;
const MAX_QUEUED_MESSAGE_BYTES: usize = 4 * 1024 * 1024;

const BRIDGE_SCRIPT: &str = r#"
window.iina = {
  listeners: Object.create(null),
  _emit(name, data) {
    const callback = this.listeners[name];
    if (typeof callback === "function") callback.call(null, data === undefined ? undefined : JSON.parse(data));
  },
  _simpleModeSetStyle(value) {
    const node = document.getElementById("style");
    if (node) node.textContent = value;
  },
  _simpleModeSetContent(value) {
    const node = document.getElementById("content");
    if (node) node.innerHTML = value;
  },
  onMessage(name, callback) { this.listeners[name] = callback; },
  postMessage(name, data) {
    window.ipc.postMessage(JSON.stringify({ name, data }));
  }
};
"#;

const SIMPLE_MODE_DOCUMENT: &str = r#"<!doctype html>
<html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Overlay</title><style>
html,body { margin:0; min-height:100%; }
body { font:13px -apple-system,BlinkMacSystemFont,"Helvetica Neue",sans-serif; color:white;
text-shadow:0 1px 0 #000,0 -1px 0 #000,-1px 0 0 #000,1px 0 0 #000; }
</style><style id="style"></style></head><body><div id="content"></div></body></html>"#;

#[derive(Default)]
struct PendingDocument {
    ready: bool,
    style: Option<String>,
    content: Option<String>,
    opacity: Option<f64>,
    messages: Vec<(String, serde_json::Value)>,
    message_bytes: usize,
}

struct HitTestState {
    enabled: AtomicBool,
    pending: AtomicBool,
    clickable_at_pointer: AtomicBool,
}

struct HitTestContext {
    state: Arc<HitTestState>,
}

struct ClickThrough {
    hwnd: HWND,
    context: Option<Box<HitTestContext>>,
}

impl ClickThrough {
    fn install(hwnd: HWND, state: Arc<HitTestState>) -> Result<Self, String> {
        let context = Box::new(HitTestContext { state });
        let reference = (&*context as *const HitTestContext) as usize;
        // SAFETY: the boxed context remains stable until the subclass is removed.
        if !unsafe { SetWindowSubclass(hwnd, Some(overlay_hit_test_proc), reference, reference) }
            .as_bool()
        {
            return Err("Could not install plugin overlay hit testing".into());
        }
        Ok(Self {
            hwnd,
            context: Some(context),
        })
    }
}

impl Drop for ClickThrough {
    fn drop(&mut self) {
        if let Some(context) = self.context.take() {
            let reference = (&*context as *const HitTestContext) as usize;
            // SAFETY: the callback context remains owned until removal.
            let removed =
                unsafe { RemoveWindowSubclass(self.hwnd, Some(overlay_hit_test_proc), reference) }
                    .as_bool();
            if !removed {
                // A live window must never retain a pointer to freed subclass data.
                Box::leak(context);
            }
        }
    }
}

unsafe extern "system" fn overlay_hit_test_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _subclass_id: usize,
    reference: usize,
) -> LRESULT {
    if message == WM_NCHITTEST {
        // SAFETY: SetWindowSubclass stores the stable Box pointer owned above.
        let context = unsafe { &*(reference as *const HitTestContext) };
        if !context.state.enabled.load(Ordering::Acquire)
            || !context.state.clickable_at_pointer.load(Ordering::Acquire)
        {
            return LRESULT(HTTRANSPARENT as isize);
        }
    }
    // SAFETY: all messages outside the plugin's clickable DOM regions preserve
    // the normal WebView2 subclass chain.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

struct OverlayEntry {
    _click_through: ClickThrough,
    webview: WebView,
    root: PathBuf,
    pending: Rc<RefCell<PendingDocument>>,
    opacity: Rc<Cell<f64>>,
    hit_test: Arc<HitTestState>,
    visible: bool,
    clickable: bool,
    simple_mode: bool,
    bounds: Option<Rect>,
}

#[derive(Default)]
pub(crate) struct PluginOverlayPresenter {
    entries: BTreeMap<String, OverlayEntry>,
    clickable_before_load: BTreeMap<String, bool>,
}

impl PluginOverlayPresenter {
    pub(crate) fn apply(
        &mut self,
        parent: &Window,
        host: &PluginHostHandle,
        plugin_identifier: String,
        plugin_directory: PathBuf,
        effect: OverlayEffect,
    ) -> Result<(), String> {
        match effect {
            OverlayEffect::SetClickable(clickable) => {
                self.clickable_before_load
                    .insert(plugin_identifier.clone(), clickable);
                if let Some(entry) = self.entries.get_mut(&plugin_identifier) {
                    entry.clickable = clickable;
                    entry.hit_test.enabled.store(clickable, Ordering::Release);
                    if !clickable {
                        entry
                            .hit_test
                            .clickable_at_pointer
                            .store(false, Ordering::Release);
                    }
                }
            }
            OverlayEffect::LoadPluginFile(path) => {
                let root = canonical_plugin_root(&plugin_directory)?;
                let url = plugin_file_url(&path)?;
                if let Some(entry) = self.entries.get_mut(&plugin_identifier) {
                    if entry.root != root {
                        self.entries.remove(&plugin_identifier);
                        self.create_entry(parent, host, &plugin_identifier, root, &url)?;
                    } else {
                        entry.simple_mode = false;
                        reset_document(&entry.pending, entry.opacity.get());
                        entry.webview.load_url(&url).map_err(webview_error)?;
                    }
                } else {
                    self.create_entry(parent, host, &plugin_identifier, root, &url)?;
                }
            }
            OverlayEffect::EnterSimpleMode => {
                let root = canonical_plugin_root(&plugin_directory)?;
                if let Some(entry) = self.entries.get_mut(&plugin_identifier) {
                    entry.simple_mode = true;
                    reset_document(&entry.pending, entry.opacity.get());
                    entry
                        .webview
                        .load_html(SIMPLE_MODE_DOCUMENT)
                        .map_err(webview_error)?;
                } else {
                    self.create_entry(
                        parent,
                        host,
                        &plugin_identifier,
                        root,
                        SIMPLE_MODE_DOCUMENT,
                    )?;
                }
            }
            effect => {
                let Some(entry) = self.entries.get_mut(&plugin_identifier) else {
                    return Ok(());
                };
                match effect {
                    OverlayEffect::Show => {
                        entry.visible = true;
                        entry.webview.set_visible(true).map_err(webview_error)?;
                    }
                    OverlayEffect::Hide => {
                        entry.visible = false;
                        entry.webview.set_visible(false).map_err(webview_error)?;
                        entry
                            .hit_test
                            .clickable_at_pointer
                            .store(false, Ordering::Release);
                    }
                    OverlayEffect::SetOpacity(opacity) => {
                        entry.opacity.set(opacity.clamp(0.0, 1.0));
                        queue_or_apply(entry, |pending| {
                            pending.opacity = Some(entry.opacity.get());
                        })?;
                    }
                    OverlayEffect::SetStyle(style) => {
                        if entry.simple_mode {
                            queue_or_apply(entry, |pending| pending.style = Some(style))?;
                        }
                    }
                    OverlayEffect::SetContent(content) => {
                        if entry.simple_mode {
                            queue_or_apply(entry, |pending| pending.content = Some(content))?;
                        }
                    }
                    OverlayEffect::PostMessage { name, data } => {
                        queue_message(entry, name, data)?;
                    }
                    OverlayEffect::DeliverMessage { name, data } => {
                        queue_message(entry, name, data)?;
                    }
                    OverlayEffect::ListenMessage(_)
                    | OverlayEffect::LoadPluginFile(_)
                    | OverlayEffect::EnterSimpleMode
                    | OverlayEffect::SetClickable(_) => {}
                }
            }
        }
        Ok(())
    }

    pub(crate) fn remove(&mut self, plugin_identifier: &str) {
        self.entries.remove(plugin_identifier);
        self.clickable_before_load.remove(plugin_identifier);
    }

    pub(crate) fn update(&mut self, width: f32, height: f32, scale: f64) {
        for entry in self.entries.values_mut() {
            let bounds = Rect {
                position: LogicalPosition::new(0.0, 0.0).into(),
                size: LogicalSize::new(f64::from(width.max(1.0)), f64::from(height.max(1.0)))
                    .into(),
            };
            if entry.bounds != Some(bounds) && entry.webview.set_bounds(bounds).is_ok() {
                entry.bounds = Some(bounds);
            }
            flush_pending(entry);
            if entry.visible && entry.clickable {
                update_dom_hit_test(entry, scale);
            }
        }
    }

    fn create_entry(
        &mut self,
        parent: &Window,
        host: &PluginHostHandle,
        plugin_identifier: &str,
        root: PathBuf,
        initial_document: &str,
    ) -> Result<(), String> {
        let pending = Rc::new(RefCell::new(PendingDocument::default()));
        let load_state = pending.clone();
        let opacity = Rc::new(Cell::new(1.0));
        let load_opacity = opacity.clone();
        let resource_root = root.clone();
        let ipc_host = host.clone();
        let ipc_identifier = plugin_identifier.to_owned();
        let initial_url = initial_document.starts_with("riina-plugin://");
        let mut builder = WebViewBuilder::new()
            .with_bounds(Rect {
                position: LogicalPosition::new(0.0, 0.0).into(),
                size: LogicalSize::new(1.0, 1.0).into(),
            })
            .with_transparent(true)
            .with_visible(false)
            .with_initialization_script(BRIDGE_SCRIPT)
            .with_custom_protocol(OVERLAY_SCHEME.into(), move |_id, request| {
                resource_response(&resource_root, request)
            })
            .with_ipc_handler(move |request: Request<String>| {
                if request.body().len() > MAX_IPC_BYTES {
                    return;
                }
                let Ok(message) = serde_json::from_str::<OverlayIpcMessage>(request.body()) else {
                    return;
                };
                if message.name.is_empty() || message.name.len() > MAX_IPC_NAME_BYTES {
                    return;
                }
                let _ = ipc_host.deliver_overlay_message(
                    ipc_identifier.clone(),
                    message.name,
                    message.data.unwrap_or(serde_json::Value::Null),
                );
            })
            .with_on_page_load_handler(move |event, _| match event {
                PageLoadEvent::Started => {
                    let mut state = load_state.borrow_mut();
                    state.ready = false;
                    state.opacity = Some(load_opacity.get());
                }
                PageLoadEvent::Finished => load_state.borrow_mut().ready = true,
            })
            .with_navigation_handler(|url| {
                url.starts_with("riina-plugin://localhost/")
                    || url.starts_with("http://riina-plugin.localhost/")
            })
            .with_new_window_req_handler(|_, _| NewWindowResponse::Deny);
        builder = if initial_url {
            builder.with_url(initial_document)
        } else {
            builder.with_html(initial_document)
        };
        let webview = builder.build_as_child(parent).map_err(webview_error)?;
        let hit_test = Arc::new(HitTestState {
            enabled: AtomicBool::new(
                self.clickable_before_load
                    .get(plugin_identifier)
                    .copied()
                    .unwrap_or(false),
            ),
            pending: AtomicBool::new(false),
            clickable_at_pointer: AtomicBool::new(false),
        });
        let click_through = ClickThrough::install(webview.hwnd(), hit_test.clone())?;
        self.entries.insert(
            plugin_identifier.to_owned(),
            OverlayEntry {
                _click_through: click_through,
                webview,
                root,
                pending,
                opacity,
                hit_test,
                visible: false,
                clickable: self
                    .clickable_before_load
                    .get(plugin_identifier)
                    .copied()
                    .unwrap_or(false),
                simple_mode: !initial_url,
                bounds: None,
            },
        );
        Ok(())
    }
}

#[derive(Deserialize)]
struct OverlayIpcMessage {
    name: String,
    data: Option<serde_json::Value>,
}

fn reset_document(state: &Rc<RefCell<PendingDocument>>, opacity: f64) {
    *state.borrow_mut() = PendingDocument {
        opacity: Some(opacity),
        ..PendingDocument::default()
    };
}

fn queue_or_apply(
    entry: &OverlayEntry,
    update: impl FnOnce(&mut PendingDocument),
) -> Result<(), String> {
    let mut pending = entry.pending.borrow_mut();
    update(&mut pending);
    if pending.ready {
        drop(pending);
        flush_pending(entry);
    }
    Ok(())
}

fn queue_message(
    entry: &OverlayEntry,
    name: String,
    data: serde_json::Value,
) -> Result<(), String> {
    let mut pending = entry.pending.borrow_mut();
    let message_bytes = name.len() + serde_json::to_vec(&data).map_or(0, |bytes| bytes.len());
    if pending.messages.len() >= MAX_QUEUED_MESSAGES
        || pending.message_bytes.saturating_add(message_bytes) > MAX_QUEUED_MESSAGE_BYTES
    {
        return Ok(());
    }
    pending.messages.push((name, data));
    pending.message_bytes += message_bytes;
    if pending.ready {
        drop(pending);
        flush_pending(entry);
    }
    Ok(())
}

fn flush_pending(entry: &OverlayEntry) {
    let (ready, style, content, opacity, messages) = {
        let mut pending = entry.pending.borrow_mut();
        if !pending.ready {
            return;
        }
        let messages = std::mem::take(&mut pending.messages);
        pending.message_bytes = 0;
        (
            pending.ready,
            pending.style.take(),
            pending.content.take(),
            pending.opacity.take(),
            messages,
        )
    };
    if !ready {
        return;
    }
    if let Some(style) = style {
        let _ = entry.webview.evaluate_script(&format!(
            "window.iina && window.iina._simpleModeSetStyle({});",
            js_string(&style)
        ));
    }
    if let Some(content) = content {
        let _ = entry.webview.evaluate_script(&format!(
            "window.iina && window.iina._simpleModeSetContent({});",
            js_string(&content)
        ));
    }
    if let Some(opacity) = opacity {
        let _ = entry.webview.evaluate_script(&format!(
            "document.documentElement.style.opacity = {};",
            opacity
        ));
    }
    for (name, data) in messages {
        let Ok(data_json) = serde_json::to_string(&data) else {
            continue;
        };
        let _ = entry.webview.evaluate_script(&format!(
            "window.iina && window.iina._emit({}, {});",
            js_string(&name),
            js_string(&data_json)
        ));
    }
}

fn update_dom_hit_test(entry: &OverlayEntry, scale: f64) {
    if entry.hit_test.pending.load(Ordering::Acquire) {
        return;
    }
    let hwnd = entry.webview.hwnd();
    let mut point = POINT::default();
    let mut client = RECT::default();
    // SAFETY: both calls write into valid local structures for the live child HWND.
    let pointer_over_window = unsafe {
        GetCursorPos(&mut point).is_ok()
            && ScreenToClient(hwnd, &mut point).as_bool()
            && GetClientRect(hwnd, &mut client).is_ok()
    } && point.x >= client.left
        && point.x < client.right
        && point.y >= client.top
        && point.y < client.bottom;
    if !pointer_over_window {
        entry
            .hit_test
            .clickable_at_pointer
            .store(false, Ordering::Release);
        return;
    }
    let scale = scale.max(0.1);
    let x = f64::from(point.x) / scale;
    let y = f64::from(point.y) / scale;
    let script = format!(
        "(()=>{{const e=document.elementFromPoint({x},{y});return !!(e&&e.dataset&&Object.prototype.hasOwnProperty.call(e.dataset,'clickable'));}})()"
    );
    entry.hit_test.pending.store(true, Ordering::Release);
    let hit_test = entry.hit_test.clone();
    if entry
        .webview
        .evaluate_script_with_callback(&script, move |result| {
            let clickable = serde_json::from_str::<bool>(&result).unwrap_or(false);
            hit_test
                .clickable_at_pointer
                .store(clickable, Ordering::Release);
            hit_test.pending.store(false, Ordering::Release);
        })
        .is_err()
    {
        entry.hit_test.pending.store(false, Ordering::Release);
        entry
            .hit_test
            .clickable_at_pointer
            .store(false, Ordering::Release);
    }
}

fn canonical_plugin_root(root: &Path) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|error| error.to_string())?;
    if !root.is_dir() {
        return Err("Plugin overlay package is not a directory".into());
    }
    Ok(root)
}

fn plugin_file_url(path: &Path) -> Result<String, String> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("Overlay files must use a safe relative plugin path".into());
    }
    let mut url = Url::parse("riina-plugin://localhost/").map_err(|error| error.to_string())?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| "Could not build a plugin overlay URL".to_owned())?;
        segments.pop_if_empty();
        for component in path.components() {
            let Component::Normal(value) = component else {
                return Err("Overlay files must use a safe relative plugin path".into());
            };
            let value = value
                .to_str()
                .ok_or_else(|| "Overlay file names must be valid Unicode".to_owned())?;
            segments.push(value);
        }
    }
    Ok(url.into())
}

fn resource_response(root: &Path, request: Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let body = || {
        let url = Url::parse(&format!("riina-plugin://localhost{}", request.uri().path()))
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        let mut path = PathBuf::new();
        let Some(segments) = url.path_segments() else {
            return Err(StatusCode::BAD_REQUEST);
        };
        for segment in segments {
            let decoded = percent_encoding::percent_decode_str(segment)
                .decode_utf8()
                .map_err(|_| StatusCode::BAD_REQUEST)?;
            if decoded.is_empty()
                || Path::new(decoded.as_ref())
                    .components()
                    .any(|part| !matches!(part, Component::Normal(_)))
                || decoded.contains(['/', '\\', '\0'])
            {
                return Err(StatusCode::FORBIDDEN);
            }
            path.push(decoded.as_ref());
        }
        let candidate = root.join(path);
        let file = candidate
            .canonicalize()
            .map_err(|_| StatusCode::NOT_FOUND)?;
        if !file.starts_with(root) || !file.is_file() {
            return Err(StatusCode::FORBIDDEN);
        }
        let bytes = fs::read(&file).map_err(|_| StatusCode::NOT_FOUND)?;
        Ok((file, bytes))
    };
    match body() {
        Ok((path, bytes)) => Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", content_type(&path))
            .header(
                "Content-Security-Policy",
                "default-src 'self' data: blob:; img-src 'self' data: blob:; media-src 'self' data: blob:; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; font-src 'self' data:; connect-src 'self'; frame-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self'",
            )
            .body(Cow::Owned(bytes))
            .unwrap_or_else(|_| Response::new(Cow::Owned(Vec::new()))),
        Err(status) => Response::builder()
            .status(status)
            .header("Content-Type", "text/plain; charset=utf-8")
            .body(Cow::Owned(Vec::new()))
            .unwrap_or_else(|_| Response::new(Cow::Owned(Vec::new()))),
    }
}

fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "wasm" => "application/wasm",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn js_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

fn webview_error(error: impl std::fmt::Display) -> String {
    format!("Plugin overlay webview failed: {error}")
}

impl PluginOverlayPresenter {
    pub(crate) fn apply_host_message(
        &mut self,
        parent: &Window,
        host: &PluginHostHandle,
        message: PluginHostMessage,
    ) -> Result<bool, String> {
        match message {
            PluginHostMessage::Overlay {
                plugin_identifier,
                plugin_directory,
                effect,
            } => {
                self.apply(parent, host, plugin_identifier, plugin_directory, effect)?;
                Ok(true)
            }
            PluginHostMessage::RemoveOverlay(identifier) => {
                self.remove(&identifier);
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}
