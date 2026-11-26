//! Standalone native WebViews track the lifetime and document of their own realm.
#[cfg(target_os = "macos")]
#[path = "plugin_standalone/appkit.rs"]
mod appkit;
#[cfg(not(target_os = "linux"))]
#[path = "plugin_standalone/native_window.rs"]
mod native;
#[cfg_attr(target_os = "linux", path = "plugin_standalone/gtk_window.rs")]
#[cfg_attr(not(target_os = "linux"), path = "plugin_standalone/slint_window.rs")]
pub(super) mod shell;
#[cfg(not(target_os = "linux"))]
pub(crate) use native::Events as NativeWindowEvents;

use super::plugin_page::{self as page, Page};
#[cfg(target_os = "linux")]
use super::unix::plugin_shortcuts;
use crate::{
    PlayerWindow,
    plugin_runtime::standalone::{Endpoint, Hub},
};
#[cfg(not(target_os = "linux"))]
use native::Shell;
use riina_core::plugin_window::{Document, Presentation, Properties};
#[cfg(target_os = "linux")]
use shell::Shell;
#[cfg(target_os = "macos")]
use std::cell::Cell;
use std::{
    collections::BTreeMap,
    sync::{Arc, Weak},
};
use wry::WebView;

#[cfg(target_os = "linux")]
pub(crate) type ShortcutOwner = std::rc::Rc<dyn Fn() -> Option<slint::Weak<PlayerWindow>>>;

fn title(name: &str, properties: &Properties) -> String {
    let title = properties.title.as_deref().unwrap_or("Window");
    format!("{title} — {name}")
}

pub(crate) struct Presenter {
    hub: Hub,
    entries: BTreeMap<u64, Entry>,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
    #[cfg(target_os = "linux")]
    application_owner: Option<ShortcutOwner>,
    #[cfg(target_os = "macos")]
    closed: Option<std::rc::Rc<crate::windows::close_observer::CloseObserver>>,
    #[cfg(target_os = "macos")]
    retaining_application: Cell<bool>,
}

impl Presenter {
    pub(crate) fn new(hub: Hub) -> Self {
        Self {
            hub,
            entries: BTreeMap::new(),
            wake: None,
            #[cfg(target_os = "linux")]
            application_owner: None,
            #[cfg(target_os = "macos")]
            closed: None,
            #[cfg(target_os = "macos")]
            retaining_application: Cell::new(false),
        }
    }

    pub(crate) fn update(&mut self, app: &PlayerWindow) {
        self.present(Some(app));
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn bind_wake(&mut self, wake: Arc<dyn Fn() + Send + Sync>) {
        self.wake = Some(wake);
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn bind_application_owner(&mut self, owner: ShortcutOwner) {
        self.application_owner = Some(owner);
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn bind_application_close(
        &mut self,
        closed: std::rc::Rc<crate::windows::close_observer::CloseObserver>,
    ) {
        self.closed = Some(closed);
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn update_from_application(&mut self) {
        self.present(None);
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn has_window_activity(&self) -> bool {
        let active = self.entries.values().any(Entry::is_visible) || self.hub.has_open_window();
        if active {
            self.retaining_application.set(true);
        }
        active
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn hide_for_quit(&mut self) {
        self.entries.clear();
    }

    fn present(&mut self, app: Option<&PlayerWindow>) {
        self.entries
            .retain(|_, entry| entry.owner.upgrade().is_some_and(|owner| owner.is_live()));
        for endpoint in self.hub.live_entries() {
            let entry = self.entries.entry(endpoint.id).or_insert_with(|| Entry {
                owner: Arc::downgrade(&endpoint),
                revision: None,
                pending: None,
                request: None,
                failed: None,
                shell: None,
                window: None,
            });
            if let Err(error) = entry.update(
                app,
                &endpoint,
                self.wake.as_ref(),
                #[cfg(target_os = "linux")]
                self.application_owner.as_ref(),
                #[cfg(target_os = "macos")]
                self.closed.as_ref(),
            ) {
                if let Some(revision) = entry.revision {
                    endpoint.presentation_failed(revision);
                }
                entry.failed = entry.request;
                entry.pending = None;
                entry.shell = None;
                entry.window = None;
                crate::diagnostics::warn!("{} standalone window: {error}", endpoint.name);
                if let Some(app) = app {
                    app.set_status(format!("{} window: {error}", endpoint.name).into());
                }
            }
        }
        #[cfg(target_os = "macos")]
        if !self.has_window_activity()
            && self.retaining_application.replace(false)
            && let Some(closed) = &self.closed
        {
            closed.notify();
        }
    }
}

struct Entry {
    owner: Weak<Endpoint>,
    revision: Option<u64>,
    pending: Option<Presentation>,
    request: Option<(u64, u64)>,
    failed: Option<(u64, u64)>,
    shell: Option<Shell>,
    window: Option<BrowserWindow>,
}

impl Entry {
    #[cfg(target_os = "macos")]
    fn is_visible(&self) -> bool {
        self.window
            .as_ref()
            .is_some_and(|window| window.shell.is_visible())
    }

    fn update(
        &mut self,
        app: Option<&PlayerWindow>,
        endpoint: &Arc<Endpoint>,
        wake: Option<&Arc<dyn Fn() + Send + Sync>>,
        #[cfg(target_os = "linux")] application_owner: Option<&ShortcutOwner>,
        #[cfg(target_os = "macos")] closed: Option<
            &std::rc::Rc<crate::windows::close_observer::CloseObserver>,
        >,
    ) -> Result<(), String> {
        if let Some(next) = endpoint.snapshot(self.revision) {
            self.revision = Some(next.revision);
            self.request = Some((next.document_id, next.open_revision));
            if let Some(window) = &mut self.window {
                window.apply(next, &endpoint.name)?;
            } else {
                self.pending = (next.created && self.request != self.failed).then_some(next);
            }
        }
        if let Some(view) = self.pending.as_ref()
            && endpoint.is_live()
        {
            if self.shell.is_none() {
                let owner = Arc::downgrade(endpoint);
                let title = title(&endpoint.name, &view.properties);
                #[cfg(target_os = "macos")]
                let closed = closed.cloned();
                let on_close = move || {
                    if let Some(owner) = owner.upgrade() {
                        owner.user_closed();
                    }
                    #[cfg(target_os = "macos")]
                    if let Some(closed) = &closed {
                        closed.notify();
                    }
                };
                #[cfg(target_os = "linux")]
                let shell = Shell::new(app, &title, move || {
                    on_close();
                    slint::CloseRequestResponse::HideWindow
                })?;
                #[cfg(not(target_os = "linux"))]
                let shell = Shell::new(app, &title, on_close, wake.cloned())?;
                self.shell = Some(shell);
            }
            // Winit creates a hidden parent in its active event loop.
            if self.shell.as_ref().is_some_and(Shell::can_build)
                && let (Some(shell), Some(view)) = (self.shell.take(), self.pending.take())
            {
                let window = BrowserWindow::new(shell, endpoint, view, wake.cloned())?;
                #[cfg(target_os = "linux")]
                {
                    use slint::ComponentHandle;
                    let application_owner = application_owner.cloned();
                    let player = app.map(ComponentHandle::as_weak);
                    plugin_shortcuts::bind(
                        &window.browser,
                        move || {
                            application_owner
                                .as_ref()
                                .map_or_else(|| player.clone(), |owner| owner())
                        },
                        Some(Arc::downgrade(endpoint)),
                    );
                }
                self.window = Some(window);
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
        endpoint: &Arc<Endpoint>,
        view: Presentation,
        wake: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> Result<Self, String> {
        let page = Page::standalone(view.document_id, endpoint.origin);
        let url = page.url(view.document_id, &view.document)?;
        shell.set_properties(&title(&endpoint.name, &view.properties), &view.properties);
        shell.set_frame(view.frame);
        let owner = Arc::downgrade(endpoint);
        let browser = shell.build_webview(
            page.builder(
                endpoint.package_root().map(std::path::Path::to_path_buf),
                move |document, name, data| {
                    if let Some(owner) = owner.upgrade() {
                        owner.receive(document, name, data);
                    }
                },
                move || {
                    if let Some(wake) = &wake {
                        wake();
                    }
                },
            )
            .with_url(&url)
            .with_focused(false),
        )?;
        shell.set_visible(view.visible)?;
        if view.visible {
            let _ = browser.focus();
        }
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
            let url = self.page.url(next.document_id, &next.document)?;
            self.page.navigating_to(next.document_id);
            self.browser
                .load_url(&url)
                .map_err(|error| error.to_string())?;
        }
        if next.properties != self.view.properties {
            self.shell
                .set_properties(&title(name, &next.properties), &next.properties);
        }
        self.shell.set_frame(next.frame_changes_since(&self.view));
        // A native close and a script reopen can land in one snapshot. Use
        // the shell's visibility and honor an explicit request to raise it.
        let raise = next.visible && next.open_revision != self.view.open_revision;
        if next.visible != self.shell.is_visible() || raise {
            self.shell.set_visible(next.visible)?;
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
