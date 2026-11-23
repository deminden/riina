//! Player-owned native WebViews for permission-gated video overlays.
mod hit_test;
#[cfg(target_os = "linux")]
mod linux;
#[cfg_attr(target_os = "linux", path = "plugin_overlay/gtk.rs")]
#[cfg_attr(target_os = "macos", path = "plugin_overlay/appkit.rs")]
#[cfg_attr(target_os = "windows", path = "plugin_overlay/win32.rs")]
mod native;

use super::plugin_page::{self as page, Page};
use crate::plugin_host::{OverlayRoute, PluginHostHandle, PluginHostMessage};
use hit_test::HitTest;
use native::Surface;
use riina_core::{plugin::OverlayEffect, plugin_window::Document};
use slint::winit_030::winit::window::Window;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use wry::{
    Rect, WebView,
    dpi::{LogicalPosition, LogicalSize},
};

const MAX_QUEUED_MESSAGES: usize = 128;
const MAX_QUEUED_MESSAGE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Default)]
pub(super) struct Presenter {
    entries: BTreeMap<String, Entry>,
    clickable_before_load: BTreeMap<String, bool>,
    pub(super) pending: Vec<PluginHostMessage>,
    #[cfg(target_os = "linux")]
    composition: linux::Composition,
}

struct Entry {
    native: Surface,
    root: PathBuf,
    realm: u64,
    page: Page,
    document: Document,
    revision: u64,
    rendered_page: Option<u64>,
    rendered_document: Option<Document>,
    rendered_opacity: Option<f64>,
    opacity: f64,
    visible: bool,
    clickable: bool,
    shown: bool,
    bounds: Option<Rect>,
    hit_test: Arc<HitTest>,
    queue: MessageQueue,
    #[cfg(target_os = "linux")]
    stacking_order: u64,
}

#[derive(Default)]
struct MessageQueue {
    messages: Vec<(String, serde_json::Value)>,
    bytes: usize,
}

impl MessageQueue {
    fn push(&mut self, name: String, data: serde_json::Value) -> Result<(), String> {
        let size = name.len()
            + serde_json::to_vec(&data)
                .map_err(|error| error.to_string())?
                .len();
        if self.messages.len() >= MAX_QUEUED_MESSAGES
            || self.bytes.saturating_add(size) > MAX_QUEUED_MESSAGE_BYTES
        {
            return Err("Plugin overlay page's message queue is full".into());
        }
        self.bytes += size;
        self.messages.push((name, data));
        Ok(())
    }
}

impl Presenter {
    pub(super) fn dispatch(
        &mut self,
        parent: &Window,
        host: &PluginHostHandle,
        message: PluginHostMessage,
        bounds: Rect,
    ) -> Result<(), String> {
        match message {
            PluginHostMessage::Overlay {
                route,
                plugin_directory,
                effect,
            } => self.apply(parent, host, route, plugin_directory, effect, bounds),
            PluginHostMessage::RemoveOverlay(identifier) => {
                self.entries.remove(&identifier);
                #[cfg(target_os = "linux")]
                self.composition.mark_changed();
                self.clickable_before_load.remove(&identifier);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn apply(
        &mut self,
        parent: &Window,
        host: &PluginHostHandle,
        route: OverlayRoute,
        directory: PathBuf,
        effect: OverlayEffect,
        bounds: Rect,
    ) -> Result<(), String> {
        let identifier = &route.identifier;
        let document = match effect {
            OverlayEffect::SetClickable(clickable) => {
                self.clickable_before_load
                    .insert(identifier.clone(), clickable);
                if let Some(entry) = self
                    .entries
                    .get_mut(identifier)
                    .filter(|entry| entry.realm == route.realm)
                {
                    entry.clickable = clickable;
                    entry.hit_test.clear();
                }
                return Ok(());
            }
            OverlayEffect::LoadPluginFile(path) => Document::Package(path),
            OverlayEffect::EnterSimpleMode => Document::Simple {
                style: None,
                content: None,
            },
            effect => {
                return self
                    .entries
                    .get_mut(identifier)
                    .filter(|entry| entry.realm == route.realm)
                    .map_or(Ok(()), |entry| entry.apply(effect));
            }
        };
        let root = canonical_plugin_root(&directory)?;
        if let Some(entry) = self.entries.get_mut(identifier)
            && entry.root == root
            && entry.realm == route.realm
        {
            return entry.navigate(document);
        }
        let clickable = self
            .clickable_before_load
            .get(identifier)
            .copied()
            .unwrap_or_default();
        let entry = Entry::new(
            parent,
            host,
            route.clone(),
            root,
            document,
            bounds,
            clickable,
        )?;
        #[cfg(target_os = "linux")]
        let entry = self.composition.stack(entry);
        self.entries.insert(route.identifier, entry);
        Ok(())
    }

    pub(super) fn update(&mut self, bounds: Rect, allowed: bool) -> Result<(), String> {
        let mut error = None;
        for entry in self.entries.values_mut() {
            #[cfg(target_os = "linux")]
            if entry.bounds != Some(bounds) {
                self.composition.mark_changed();
            }
            if let Err(reason) = entry.update(bounds, allowed) {
                error = Some(reason);
            }
        }
        error.map_or(Ok(()), Err)
    }
}

impl Entry {
    fn new(
        parent: &Window,
        host: &PluginHostHandle,
        route: OverlayRoute,
        root: PathBuf,
        document: Document,
        bounds: Rect,
        clickable: bool,
    ) -> Result<Self, String> {
        let revision = 1;
        let page = Page::overlay(revision);
        let url = page.url(revision, &document)?;
        let sender = host.clone();
        let loaded = host.clone();
        let realm = route.realm;
        let receive_route = route.clone();
        let builder = page
            .builder(
                Some(root.clone()),
                &url,
                move |_, name, data| {
                    let _ = sender.deliver_overlay_message(receive_route.clone(), name, data);
                },
                move || {
                    let _ = loaded.notify_video_overlay_loaded(route.clone());
                },
            )
            .with_bounds(bounds)
            .with_transparent(true)
            .with_visible(false)
            .with_focused(false);
        let hit_test = Arc::new(HitTest::default());
        let native = Surface::build(builder, parent, bounds, hit_test.clone())?;
        Ok(Self {
            native,
            root,
            realm,
            page,
            document,
            revision,
            rendered_page: None,
            rendered_document: None,
            rendered_opacity: None,
            opacity: 1.0,
            visible: false,
            clickable,
            shown: false,
            bounds: Some(bounds),
            hit_test,
            queue: MessageQueue::default(),
            #[cfg(target_os = "linux")]
            stacking_order: 0,
        })
    }

    fn navigate(&mut self, document: Document) -> Result<(), String> {
        let revision = self.revision.wrapping_add(1);
        let url = self.page.url(revision, &document)?;
        self.hit_test.clear();
        self.queue = MessageQueue::default();
        #[cfg(target_os = "linux")]
        self.native.invalidate();
        self.page.navigating_to(revision);
        self.revision = revision;
        self.document = document;
        self.browser()
            .load_url(&url)
            .map_err(|error| error.to_string())
    }

    fn apply(&mut self, effect: OverlayEffect) -> Result<(), String> {
        match effect {
            OverlayEffect::Show => self.visible = true,
            OverlayEffect::Hide => {
                self.visible = false;
                self.hit_test.clear();
            }
            OverlayEffect::SetOpacity(opacity) => self.opacity = opacity,
            OverlayEffect::SetStyle(value) => {
                if let Document::Simple { style, .. } = &mut self.document {
                    *style = Some(value.into());
                }
                self.hit_test.clear();
            }
            OverlayEffect::SetContent(value) => {
                if let Document::Simple { content, .. } = &mut self.document {
                    *content = Some(value.into());
                }
                self.hit_test.clear();
            }
            OverlayEffect::PostMessage { name, data }
            | OverlayEffect::DeliverMessage { name, data } => self.queue.push(name, data)?,
            OverlayEffect::ListenMessage(_)
            | OverlayEffect::LoadPluginFile(_)
            | OverlayEffect::EnterSimpleMode
            | OverlayEffect::SetClickable(_) => {}
        }
        Ok(())
    }

    fn update(&mut self, bounds: Rect, allowed: bool) -> Result<(), String> {
        if self.bounds != Some(bounds) {
            self.native.set_bounds(bounds)?;
            self.bounds = Some(bounds);
            self.hit_test.clear();
        }
        let visible = self.visible && allowed;
        if self.shown != visible {
            #[cfg(not(target_os = "linux"))]
            self.browser()
                .set_visible(visible)
                .map_err(|error| error.to_string())?;
            #[cfg(target_os = "linux")]
            if !visible {
                self.native.invalidate();
            }
            self.shown = visible;
            self.hit_test.clear();
        }
        let Some(page) = self.page.ready_page() else {
            return Ok(());
        };
        if self.rendered_page != Some(page) {
            self.rendered_page = Some(page);
            self.rendered_document = None;
            self.rendered_opacity = None;
        }
        if self.rendered_document.as_ref() != Some(&self.document) {
            page::update_simple_document(
                self.browser(),
                self.revision,
                self.rendered_document.as_ref(),
                &self.document,
            )?;
            self.rendered_document = Some(self.document.clone());
        }
        if self.rendered_opacity != Some(self.opacity) {
            page::evaluate(
                self.browser(),
                self.revision,
                &format!("document.documentElement.style.opacity = {};", self.opacity),
            )?;
            self.rendered_opacity = Some(self.opacity);
        }
        for (name, data) in std::mem::take(&mut self.queue).messages {
            page::deliver(self.browser(), self.revision, &name, &data)?;
        }
        if visible {
            #[cfg(target_os = "linux")]
            self.native.capture();
            #[cfg(not(target_os = "linux"))]
            if self.clickable {
                self.hit_test
                    .sample(self.browser(), self.revision, self.native.pointer());
            }
        }
        Ok(())
    }

    fn browser(&self) -> &WebView {
        self.native.browser()
    }
}

pub(super) fn bounds(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect {
        position: LogicalPosition::new(f64::from(x), f64::from(y)).into(),
        size: LogicalSize::new(f64::from(width.max(1.0)), f64::from(height.max(1.0))).into(),
    }
}

fn canonical_plugin_root(root: &Path) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|error| error.to_string())?;
    if !root.is_dir() {
        return Err("Plugin overlay package is not a directory".into());
    }
    Ok(root)
}
