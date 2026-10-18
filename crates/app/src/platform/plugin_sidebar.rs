//! Confined plugin sidebar documents hosted as child WebViews.

use super::plugin_resources::{plugin_file_url, resource_response};
use crate::plugin_host::{PluginHostHandle, PluginHostMessage};
use riina_core::plugin_sidebar::PluginSidebarEffect;
use slint::winit_030::winit::window::Window;
use std::{
    cell::Cell,
    collections::BTreeMap,
    path::{Path, PathBuf},
    rc::Rc,
};
use wry::{
    NewWindowResponse, PageLoadEvent, Rect, WebView, WebViewBuilder,
    dpi::{LogicalPosition, LogicalSize},
    http::Request,
};

const PLUGIN_SCHEME: &str = "riina-plugin";
const MAX_IPC_BYTES: usize = 1024 * 1024 + 512;
const MAX_IPC_NAME_BYTES: usize = 256;
const MAX_QUEUED_MESSAGES: usize = 128;
const MAX_QUEUED_MESSAGE_BYTES: usize = 4 * 1024 * 1024;

const BRIDGE_SCRIPT: &str = r#"
window.iina = {
  listeners: Object.create(null),
  onMessage(name, callback) { this.listeners[name] = callback; },
  postMessage(name, data) { window.ipc.postMessage(JSON.stringify({ name, data })); },
  _emit(name, data) {
    const callback = this.listeners[name];
    if (typeof callback === "function") callback.call(null, JSON.parse(data));
  }
};
"#;

struct SidebarEntry {
    webview: WebView,
    root: PathBuf,
    ready: Rc<Cell<bool>>,
    visible: bool,
    bounds: Option<Rect>,
    messages: Vec<(String, serde_json::Value)>,
    message_bytes: usize,
}

#[derive(Default)]
pub(crate) struct PluginSidebarPresenter {
    entries: BTreeMap<String, SidebarEntry>,
    selected: Option<String>,
}

impl PluginSidebarPresenter {
    pub(crate) fn apply_host_message(
        &mut self,
        parent: &Window,
        host: &PluginHostHandle,
        message: PluginHostMessage,
    ) -> Result<(), String> {
        let PluginHostMessage::Sidebar {
            plugin_identifier,
            plugin_directory,
            effect,
        } = message
        else {
            return Ok(());
        };
        self.apply(parent, host, plugin_identifier, plugin_directory, effect)
    }

    fn apply(
        &mut self,
        parent: &Window,
        host: &PluginHostHandle,
        identifier: String,
        directory: PathBuf,
        effect: PluginSidebarEffect,
    ) -> Result<(), String> {
        match effect {
            PluginSidebarEffect::LoadFile(path) => {
                let root = canonical_plugin_root(&directory)?;
                let url = plugin_file_url(&path)?;
                if let Some(entry) = self.entries.get_mut(&identifier) {
                    if entry.root != root {
                        self.entries.remove(&identifier);
                        self.create_entry(parent, host, &identifier, root, &url)?;
                    } else {
                        entry.ready.set(false);
                        entry.messages.clear();
                        entry.message_bytes = 0;
                        entry.webview.load_url(&url).map_err(webview_error)?;
                    }
                } else {
                    self.create_entry(parent, host, &identifier, root, &url)?;
                }
            }
            PluginSidebarEffect::Show | PluginSidebarEffect::Hide => {}
            PluginSidebarEffect::PostMessage { name, data } => {
                if let Some(entry) = self.entries.get_mut(&identifier) {
                    queue_message(entry, name, data);
                }
            }
            PluginSidebarEffect::ListenMessage(_) | PluginSidebarEffect::DeliverMessage { .. } => {}
        }
        Ok(())
    }

    pub(crate) fn select(&mut self, identifier: Option<String>) {
        self.selected = identifier;
        self.update_visibility();
    }

    pub(crate) fn remove(&mut self, identifier: &str) {
        self.entries.remove(identifier);
        if self.selected.as_deref() == Some(identifier) {
            self.selected = None;
        }
    }

    pub(crate) fn update(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        selected: Option<String>,
    ) {
        self.selected = selected;
        let bounds = Rect {
            position: LogicalPosition::new(f64::from(x), f64::from(y)).into(),
            size: LogicalSize::new(f64::from(width.max(1.0)), f64::from(height.max(1.0))).into(),
        };
        for (identifier, entry) in &mut self.entries {
            if entry.bounds != Some(bounds) && entry.webview.set_bounds(bounds).is_ok() {
                entry.bounds = Some(bounds);
            }
            entry.visible = self.selected.as_deref() == Some(identifier);
            let _ = entry.webview.set_visible(entry.visible);
            if entry.ready.get() {
                flush_messages(entry);
            }
        }
    }

    fn update_visibility(&mut self) {
        for (identifier, entry) in &mut self.entries {
            entry.visible = self.selected.as_deref() == Some(identifier);
            let _ = entry.webview.set_visible(entry.visible);
        }
    }

    fn create_entry(
        &mut self,
        parent: &Window,
        host: &PluginHostHandle,
        identifier: &str,
        root: PathBuf,
        url: &str,
    ) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        prepare_linux_webview(parent)?;
        let load_state = Rc::new(Cell::new(false));
        let page_ready = load_state.clone();
        let overlay_loaded_host = host.clone();
        let resource_root = root.clone();
        let ipc_host = host.clone();
        let ipc_identifier = identifier.to_owned();
        let webview = WebViewBuilder::new()
            .with_bounds(Rect {
                position: LogicalPosition::new(0.0, 0.0).into(),
                size: LogicalSize::new(1.0, 1.0).into(),
            })
            .with_transparent(true)
            .with_visible(false)
            .with_initialization_script(BRIDGE_SCRIPT)
            .with_custom_protocol(PLUGIN_SCHEME.into(), move |_id, request| {
                resource_response(&resource_root, request)
            })
            .with_ipc_handler(move |request: Request<String>| {
                if request.body().len() > MAX_IPC_BYTES {
                    return;
                }
                let Ok(message) = serde_json::from_str::<IpcMessage>(request.body()) else {
                    return;
                };
                if message.name.is_empty() || message.name.len() > MAX_IPC_NAME_BYTES {
                    return;
                }
                let _ = ipc_host.deliver_sidebar_message(
                    ipc_identifier.clone(),
                    message.name,
                    message.data.unwrap_or(serde_json::Value::Null),
                );
            })
            .with_on_page_load_handler(move |event, _| {
                let loaded = matches!(event, PageLoadEvent::Finished);
                page_ready.set(loaded);
                if loaded {
                    let _ = overlay_loaded_host.notify_plugin_overlay_loaded();
                }
            })
            .with_navigation_handler(|url| {
                url.starts_with("riina-plugin://localhost/")
                    || url.starts_with("http://riina-plugin.localhost/")
            })
            .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
            .with_url(url)
            .build_as_child(parent)
            .map_err(webview_error)?;
        let visible = self.selected.as_deref() == Some(identifier);
        webview.set_visible(visible).map_err(webview_error)?;
        self.entries.insert(
            identifier.to_owned(),
            SidebarEntry {
                webview,
                root,
                ready: load_state,
                visible,
                bounds: None,
                messages: Vec::new(),
                message_bytes: 0,
            },
        );
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn prepare_linux_webview(parent: &Window) -> Result<(), String> {
    use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = parent
        .window_handle()
        .map_err(|error| format!("Could not inspect the Linux player window: {error}"))?;
    if !matches!(handle.as_raw(), RawWindowHandle::Xlib(_)) {
        return Err("Plugin sidebar WebViews currently require an X11 player window".into());
    }
    if gtk::is_initialized() {
        if !gtk::is_initialized_main_thread() {
            return Err("Plugin sidebar GTK must run on the player UI thread".into());
        }
    } else {
        gtk::init()
            .map_err(|error| format!("Could not initialize GTK for plugin sidebar: {error}"))?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub(crate) fn pump_linux_webview_events() {
    if gtk::is_initialized_main_thread() {
        while gtk::events_pending() {
            gtk::main_iteration_do(false);
        }
    }
}

#[derive(serde::Deserialize)]
struct IpcMessage {
    name: String,
    data: Option<serde_json::Value>,
}

fn queue_message(entry: &mut SidebarEntry, name: String, data: serde_json::Value) {
    let bytes = name.len() + serde_json::to_vec(&data).map_or(usize::MAX, |value| value.len());
    if entry.messages.len() >= MAX_QUEUED_MESSAGES
        || entry.message_bytes.saturating_add(bytes) > MAX_QUEUED_MESSAGE_BYTES
    {
        return;
    }
    entry.messages.push((name, data));
    entry.message_bytes += bytes;
    flush_messages(entry);
}

fn flush_messages(entry: &mut SidebarEntry) {
    if !entry.ready.get() {
        return;
    }
    for (name, data) in std::mem::take(&mut entry.messages) {
        let Ok(data) = serde_json::to_string(&data) else {
            continue;
        };
        let _ = entry.webview.evaluate_script(&format!(
            "window.iina && window.iina._emit({}, {});",
            js_string(&name),
            js_string(&data)
        ));
    }
    entry.message_bytes = 0;
}

fn canonical_plugin_root(root: &Path) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|error| error.to_string())?;
    if !root.is_dir() {
        return Err("Plugin sidebar package is not a directory".into());
    }
    Ok(root)
}

fn js_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

fn webview_error(error: impl std::fmt::Display) -> String {
    format!("Plugin sidebar WebView failed: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    use wry::http::StatusCode;

    fn temporary_root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "riina-plugin-sidebar-webview-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn sidebar_document_urls_reject_escape_paths() {
        assert!(plugin_file_url(Path::new("sidebar/index.html")).is_ok());
        assert!(plugin_file_url(Path::new("../outside.html")).is_err());
        assert!(plugin_file_url(Path::new("/outside.html")).is_err());
    }

    #[test]
    fn package_protocol_serves_only_canonical_files_inside_the_package() {
        let root = temporary_root();
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("index.html"), "<p>sidebar</p>").unwrap();
        let canonical_root = root.canonicalize().unwrap();

        let request = Request::builder()
            .uri("/index.html")
            .body(Vec::new())
            .unwrap();
        let response = resource_response(&canonical_root, request);
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("Content-Type").unwrap(),
            "text/html; charset=utf-8"
        );
        assert_eq!(response.body().as_ref(), b"<p>sidebar</p>");

        let escape = Request::builder()
            .uri("/%2e%2e/outside.html")
            .body(Vec::new())
            .unwrap();
        let response = resource_response(&canonical_root, escape);
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        fs::remove_dir_all(root).unwrap();
    }
}
