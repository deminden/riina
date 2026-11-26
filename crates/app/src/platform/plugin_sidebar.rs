//! Confined plugin sidebar documents owned by their player.
#[cfg(target_os = "linux")]
mod input_method;
mod view;
#[cfg(target_os = "linux")]
mod wayland;
#[cfg(target_os = "linux")]
pub(crate) use wayland::route_sidebar_window_event;

use super::plugin_page::{self as page, Page};
use crate::{
    PlayerWindow,
    plugin_host::{PlayerPluginRoute, PluginHostHandle, PluginHostMessage},
};
use riina_core::{plugin_sidebar::PluginSidebarEffect, plugin_window::Document};
use slint::winit_030::winit::window::Window;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use view::View;
use wry::{
    Rect,
    dpi::{LogicalPosition, LogicalSize},
};

const MAX_IPC_NAME_BYTES: usize = 256;
const MAX_QUEUED_MESSAGES: usize = 128;
const MAX_QUEUED_MESSAGE_BYTES: usize = 4 * 1024 * 1024;

struct SidebarEntry {
    view: View,
    root: PathBuf,
    realm: u64,
    page: Page,
    document: u64,
    visible: bool,
    bounds: Option<Rect>,
    messages: Vec<(String, serde_json::Value)>,
    message_bytes: usize,
}

impl SidebarEntry {
    fn set_bounds(&mut self, bounds: Rect) {
        if self.bounds != Some(bounds) && self.view.set_bounds(bounds).is_ok() {
            self.bounds = Some(bounds);
        }
    }

    fn set_visible(&mut self, visible: bool) {
        #[cfg(target_os = "linux")]
        if self.visible == visible && self.view.composited().is_some() {
            return;
        }
        if self.view.set_visible(visible).is_ok() {
            if cfg!(target_os = "linux") && visible && !self.visible {
                // GTK lays out visible children. A resize while the view was
                // hidden can leave its child at the old allocation.
                self.bounds = None;
            }
            self.visible = visible;
        }
    }
}

pub(crate) struct PluginSidebarPresenter {
    entries: BTreeMap<String, SidebarEntry>,
    selected: Option<String>,
    #[cfg(target_os = "linux")]
    owner: slint::Weak<PlayerWindow>,
    #[cfg(target_os = "linux")]
    composited: Option<String>,
}

impl PluginSidebarPresenter {
    pub(crate) fn new(app: &PlayerWindow) -> Self {
        #[cfg(target_os = "linux")]
        use slint::ComponentHandle;
        #[cfg(not(target_os = "linux"))]
        let _ = app;
        Self {
            entries: BTreeMap::new(),
            selected: None,
            #[cfg(target_os = "linux")]
            owner: app.as_weak(),
            #[cfg(target_os = "linux")]
            composited: None,
        }
    }

    pub(crate) fn viewport(app: &PlayerWindow) -> Rect {
        Rect {
            position: LogicalPosition::new(
                f64::from(app.get_plugin_sidebar_view_x()),
                f64::from(app.get_plugin_sidebar_view_y()),
            )
            .into(),
            size: LogicalSize::new(
                f64::from(app.get_plugin_sidebar_view_width().max(1.0)),
                f64::from(app.get_plugin_sidebar_view_height().max(1.0)),
            )
            .into(),
        }
    }

    pub(crate) fn apply_host_message(
        &mut self,
        parent: &Window,
        host: &PluginHostHandle,
        message: PluginHostMessage,
        viewport: Rect,
    ) -> Result<(), String> {
        let PluginHostMessage::Sidebar {
            route,
            plugin_directory,
            effect,
        } = message
        else {
            return Ok(());
        };
        self.apply(parent, host, route, plugin_directory, effect, viewport)
    }

    fn apply(
        &mut self,
        parent: &Window,
        host: &PluginHostHandle,
        route: PlayerPluginRoute,
        directory: PathBuf,
        effect: PluginSidebarEffect,
        viewport: Rect,
    ) -> Result<(), String> {
        match effect {
            PluginSidebarEffect::LoadFile(path) => {
                let root = canonical_plugin_root(&directory)?;
                let document = Document::Package(path);
                if let Some(entry) = self.entries.get_mut(&route.identifier)
                    && entry.root == root
                    && entry.realm == route.realm
                {
                    let revision = entry.document.wrapping_add(1);
                    let url = entry.page.url(revision, &document)?;
                    entry.page.navigating_to(revision);
                    entry.document = revision;
                    entry.messages.clear();
                    entry.message_bytes = 0;
                    #[cfg(target_os = "linux")]
                    if let Some(view) = entry.view.composited() {
                        view.surface.invalidate();
                    }
                    entry.view.browser().load_url(&url).map_err(webview_error)?;
                } else {
                    self.create_entry(parent, host, route, root, &document, viewport)?;
                }
            }
            PluginSidebarEffect::Show | PluginSidebarEffect::Hide => {}
            PluginSidebarEffect::PostMessage { name, data } => {
                if let Some(entry) = self.entries.get_mut(&route.identifier)
                    && entry.realm == route.realm
                {
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
        #[cfg(target_os = "linux")]
        self.update_composited();
    }

    pub(crate) fn close(&mut self) {
        self.entries.clear();
        self.selected = None;
        #[cfg(target_os = "linux")]
        self.update_composited();
        #[cfg(target_os = "linux")]
        if gtk::is_initialized_main_thread()
            && let Some(display) = gtk::gdk::Display::default()
        {
            // GTK and Winit use separate X11 connections. Finish child
            // requests before Winit can destroy the player window.
            display.sync();
        }
    }

    pub(crate) fn update(&mut self, bounds: Rect, selected: Option<String>) {
        self.selected = selected;
        let allowed = self.sidebar_can_show();
        for (identifier, entry) in &mut self.entries {
            let visible = allowed && self.selected.as_deref() == Some(identifier);
            entry.set_bounds(bounds);
            // A later update reapplies the desired visibility after a child window change.
            entry.set_visible(visible);
            if cfg!(target_os = "linux") && visible {
                entry.set_bounds(bounds);
            }
            flush_messages(entry);
        }
        #[cfg(target_os = "linux")]
        self.update_composited();
    }

    fn update_visibility(&mut self) {
        let allowed = self.sidebar_can_show();
        for (identifier, entry) in &mut self.entries {
            entry.set_visible(allowed && self.selected.as_deref() == Some(identifier));
        }
        #[cfg(target_os = "linux")]
        self.update_composited();
    }

    fn sidebar_can_show(&self) -> bool {
        #[cfg(target_os = "linux")]
        {
            // Native children paint over the Slint surface. Keep their page
            // alive while the player owns an in-window modal editor.
            self.owner
                .upgrade()
                .is_some_and(|app| sidebar_can_show(&app))
        }
        #[cfg(not(target_os = "linux"))]
        {
            true
        }
    }

    fn create_entry(
        &mut self,
        parent: &Window,
        host: &PluginHostHandle,
        route: PlayerPluginRoute,
        root: PathBuf,
        document: &Document,
        viewport: Rect,
    ) -> Result<(), String> {
        let revision = 1;
        let page = Page::sidebar(revision);
        let url = page.url(revision, document)?;
        let loaded_host = host.clone();
        let loaded_route = route.clone();
        let ipc_host = host.clone();
        let ipc_route = route.clone();
        let builder = page
            .builder(
                Some(root.clone()),
                move |_, name, data| {
                    if !name.is_empty() && name.len() <= MAX_IPC_NAME_BYTES {
                        let _ = ipc_host.deliver_sidebar_message(ipc_route.clone(), name, data);
                    }
                },
                move || {
                    let _ = loaded_host.notify_plugin_page_loaded(loaded_route.clone());
                },
            )
            // On X11 the GTK child can retain its initial allocation until a
            // later resize. Create it with the current sidebar viewport.
            .with_bounds(viewport)
            .with_transparent(true)
            .with_visible(false);
        let view = View::new(
            builder,
            parent,
            viewport,
            &url,
            #[cfg(target_os = "linux")]
            self.owner.clone(),
        )?;
        #[cfg(target_os = "linux")]
        view.bind_shortcuts(self.owner.clone());
        let visible =
            self.sidebar_can_show() && self.selected.as_deref() == Some(route.identifier.as_str());
        view.set_visible(visible)?;
        self.entries.insert(
            route.identifier,
            SidebarEntry {
                view,
                root,
                realm: route.realm,
                page,
                document: revision,
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
fn sidebar_can_show(app: &PlayerWindow) -> bool {
    use slint::ComponentHandle;

    let dialog = app.global::<riina_ui::PlayerDialogState>();
    !dialog.get_dialog_open()
        && !dialog.get_jump_time_dialog_visible()
        && !dialog.get_url_dialog_visible()
        && !dialog.get_plugin_interaction_visible()
}

#[cfg(target_os = "linux")]
pub(crate) fn pump_linux_webview_events() {
    if gtk::is_initialized_main_thread() {
        while gtk::events_pending() {
            gtk::main_iteration_do(false);
        }
    }
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
    if entry.page.ready_page().is_none() {
        return;
    }
    for (name, data) in std::mem::take(&mut entry.messages) {
        let _ = page::deliver(entry.view.browser(), entry.document, &name, &data);
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

fn webview_error(error: impl std::fmt::Display) -> String {
    format!("Plugin sidebar WebView failed: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::plugin_resources::{plugin_file_url, resource_response};
    use riina_test_support::TempDir;
    use std::fs;
    use wry::http::{Request, StatusCode};

    #[test]
    fn sidebar_document_urls_reject_escape_paths() {
        assert!(plugin_file_url(Path::new("sidebar/index.html")).is_ok());
        assert!(plugin_file_url(Path::new("../outside.html")).is_err());
        assert!(plugin_file_url(Path::new("/outside.html")).is_err());
    }

    #[test]
    fn package_protocol_serves_only_canonical_files_inside_the_package() {
        let directory = TempDir::new("plugin-sidebar").unwrap();
        let root = directory.path();
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
    }
}
