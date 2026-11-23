//! Standalone native WebViews track the lifetime and document of their own realm.
mod page;
#[cfg_attr(target_os = "linux", path = "plugin_standalone/gtk_window.rs")]
#[cfg_attr(not(target_os = "linux"), path = "plugin_standalone/slint_window.rs")]
pub(super) mod shell;

use crate::{
    PlayerWindow,
    plugin_runtime::standalone::{Endpoint, Hub},
};
use page::Page;
use riina_core::plugin_window::{Document, Presentation, Properties};
use shell::Shell;
use std::{
    collections::BTreeMap,
    sync::{Arc, Weak},
};
use wry::WebView;

fn title(name: &str, properties: &Properties) -> String {
    let title = properties.title.as_deref().unwrap_or("Window");
    format!("{title} — {name}")
}

pub(crate) struct Presenter {
    hub: Hub,
    entries: BTreeMap<u64, Entry>,
}

impl Presenter {
    pub(crate) fn new(hub: Hub) -> Self {
        Self {
            hub,
            entries: BTreeMap::new(),
        }
    }

    pub(crate) fn update(&mut self, app: &PlayerWindow) {
        self.entries
            .retain(|_, entry| entry.owner.upgrade().is_some_and(|owner| owner.is_live()));
        for endpoint in self.hub.live_entries() {
            let entry = self.entries.entry(endpoint.id).or_insert_with(|| Entry {
                owner: Arc::downgrade(&endpoint),
                revision: None,
                pending: None,
                shell: None,
                window: None,
            });
            if let Err(error) = entry.update(app, &endpoint) {
                if let Some(revision) = entry.revision {
                    endpoint.presentation_failed(revision);
                }
                entry.shell = None;
                entry.window = None;
                crate::diagnostics::warn!("{} standalone window: {error}", endpoint.name);
                app.set_status(format!("{} window: {error}", endpoint.name).into());
            }
        }
    }
}

struct Entry {
    owner: Weak<Endpoint>,
    revision: Option<u64>,
    pending: Option<Presentation>,
    shell: Option<Shell>,
    window: Option<BrowserWindow>,
}

impl Entry {
    fn update(&mut self, app: &PlayerWindow, endpoint: &Arc<Endpoint>) -> Result<(), String> {
        if let Some(next) = endpoint.snapshot(self.revision) {
            self.revision = Some(next.revision);
            if let Some(window) = &mut self.window {
                window.apply(next, &endpoint.name)?;
            } else {
                if !next.visible {
                    self.shell = None;
                }
                self.pending = next.visible.then_some(next);
            }
        }
        if let Some(view) = self.pending.as_ref()
            && endpoint.is_live()
        {
            if self.shell.is_none() {
                let owner = Arc::downgrade(endpoint);
                let title = title(&endpoint.name, &view.properties);
                self.shell = Some(Shell::new(app, &title, move || {
                    if let Some(owner) = owner.upgrade() {
                        owner.user_closed();
                    }
                    slint::CloseRequestResponse::HideWindow
                })?);
            }
            // Slint realizes a new Winit window after returning to the event
            // loop. Keep its shell alive until the native WebView can attach.
            if self.shell.as_ref().is_some_and(Shell::can_build)
                && let (Some(shell), Some(view)) = (self.shell.take(), self.pending.take())
            {
                self.window = Some(BrowserWindow::new(shell, app, endpoint, view)?);
            }
        }
        if let Some(window) = &mut self.window {
            window.flush(endpoint)?;
        }
        Ok(())
    }
}

struct BrowserWindow {
    // Keep the native parent alive until Wry releases its WebView.
    browser: WebView,
    shell: Shell,
    page: Page,
    previous_page: Option<u64>,
    rendered: Option<Document>,
    view: Presentation,
}

impl BrowserWindow {
    fn new(
        shell: Shell,
        app: &PlayerWindow,
        endpoint: &Arc<Endpoint>,
        view: Presentation,
    ) -> Result<Self, String> {
        let page = Page::new(view.document_id, endpoint.origin);
        let url = page.url(&view)?;
        shell.set_properties(&title(&endpoint.name, &view.properties), &view.properties);
        shell.set_frame(view.frame, Default::default());
        let browser = shell.build_webview(page.builder(endpoint, &url))?;
        #[cfg(target_os = "linux")]
        {
            use slint::ComponentHandle;
            super::unix::plugin_shortcuts::bind(
                &browser,
                app.as_weak(),
                Some(Arc::downgrade(endpoint)),
            );
        }
        #[cfg(not(target_os = "linux"))]
        let _ = app;
        shell.set_visible(view.visible)?;
        #[cfg(target_os = "macos")]
        let _ = browser.focus();
        Ok(Self {
            browser,
            shell,
            page,
            previous_page: None,
            rendered: None,
            view,
        })
    }

    fn apply(&mut self, next: Presentation, name: &str) -> Result<(), String> {
        if self.view.document_id != next.document_id {
            let url = self.page.url(&next)?;
            self.page.navigating_to(next.document_id);
            self.browser
                .load_url(&url)
                .map_err(|error| error.to_string())?;
        }
        if next.properties != self.view.properties {
            self.shell
                .set_properties(&title(name, &next.properties), &next.properties);
        }
        if next.frame != self.view.frame {
            self.shell.set_frame(next.frame, self.view.frame);
        }
        if next.visible != self.view.visible {
            self.shell.set_visible(next.visible)?;
            #[cfg(target_os = "macos")]
            if next.visible {
                let _ = self.browser.focus();
            }
        }
        self.view = next;
        Ok(())
    }

    fn flush(&mut self, endpoint: &Endpoint) -> Result<(), String> {
        if !endpoint.is_live() {
            return Ok(());
        }
        #[cfg(target_os = "macos")]
        self.shell.refresh_browser_bounds(&self.browser)?;
        let Some(page) = self.page.ready_page() else {
            return Ok(());
        };
        if self.previous_page != Some(page) {
            self.previous_page = Some(page);
            self.rendered = None;
        }
        if self.rendered.as_ref() != Some(&self.view.document) {
            page::update_simple_document(
                &self.browser,
                self.view.document_id,
                self.rendered.as_ref(),
                &self.view.document,
            )?;
            self.rendered = Some(self.view.document.clone());
        }
        for (name, data) in endpoint.take_messages(self.view.document_id) {
            page::deliver(&self.browser, self.view.document_id, &name, &data)?;
        }
        Ok(())
    }
}
