//! Own a package settings page for the lifetime of its Settings window.
mod document;

use super::plugin_standalone::shell::Shell;
use crate::PreferencesWindow;
use document::Document;
use riina_core::plugin::Manifest;
use slint::{CloseRequestResponse, ComponentHandle, Timer, TimerMode};
use std::{
    cell::{Cell, RefCell},
    path::Path,
    rc::Rc,
    time::Duration,
};
use uuid::Uuid;
use wry::WebView;

#[derive(Clone, Default)]
pub(crate) struct Presenter(Rc<RefCell<Option<Rc<Window>>>>);

impl Presenter {
    pub(crate) fn open(
        &self,
        owner: &PreferencesWindow,
        root: &Path,
        manifest: &Manifest,
    ) -> Result<(), String> {
        self.close()?;
        let id = Uuid::new_v4();
        let document = Rc::new(Document::new(root, manifest, id)?);
        let title = format!("{} — Settings", manifest.name);
        let controller = Rc::downgrade(&self.0);
        let owner_weak = owner.as_weak();
        let shell = Shell::new(owner, &title, move || {
            let controller = controller.clone();
            let owner = owner_weak.clone();
            let _ = slint::spawn_local(async move {
                if let Some(controller) = controller.upgrade() {
                    let presenter = Presenter(controller);
                    let is_current = presenter
                        .0
                        .borrow()
                        .as_ref()
                        .is_some_and(|page| page.id == id);
                    if is_current
                        && let Err(error) = presenter.close()
                        && let Some(owner) = owner.upgrade()
                    {
                        report(&owner, &error);
                    }
                }
            });
            CloseRequestResponse::KeepWindowShown
        })?;
        let window = Rc::new(Window {
            id,
            browser: RefCell::default(),
            shell,
            timer: Timer::default(),
            scheduled: Cell::default(),
            document,
            owner: owner.as_weak(),
        });
        self.0.replace(Some(window.clone()));
        window.start();
        Ok(())
    }

    pub(crate) fn close(&self) -> Result<(), String> {
        let window = self.0.borrow_mut().take();
        let Some(window) = window else {
            return Ok(());
        };
        window.timer.stop();
        let save = window.document.sync();
        let hide = window.shell.set_visible(false);
        save.and(hide)
    }
}

struct Window {
    id: Uuid,
    // Release the embedded browser before destroying its native parent.
    browser: RefCell<Option<Rc<WebView>>>,
    shell: Shell,
    timer: Timer,
    scheduled: Cell<bool>,
    document: Rc<Document>,
    owner: slint::Weak<PreferencesWindow>,
}

impl Window {
    fn start(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.timer
            .start(TimerMode::Repeated, Duration::from_millis(25), move || {
                let Some(window) = weak.upgrade() else { return };
                if window.scheduled.replace(true) {
                    return;
                }
                let weak = Rc::downgrade(&window);
                // Native browsers may run other UI callbacks during a native call.
                // Keep that work outside Slint's timer registry borrow.
                let result = slint::spawn_local(async move {
                    let Some(window) = weak.upgrade() else { return };
                    if let Err(error) = window.tick() {
                        window.timer.stop();
                        let _ = window.shell.set_visible(false);
                        if let Some(owner) = window.owner.upgrade() {
                            report(&owner, &error);
                        }
                    }
                    window.scheduled.set(false);
                });
                if result.is_err() {
                    window.scheduled.set(false);
                }
            });
    }

    fn tick(&self) -> Result<(), String> {
        if !self.timer.running() {
            return Ok(());
        }
        #[cfg(target_os = "linux")]
        super::pump_plugin_webviews();
        if !self.timer.running() {
            return Ok(());
        }
        let browser = self.browser.borrow().clone();
        if let Some(browser) = browser {
            #[cfg(target_os = "macos")]
            self.shell.refresh_browser_bounds(&browser)?;
            self.document.flush(&browser)?;
        } else if self.shell.can_build() {
            let browser = Rc::new(self.shell.build_webview(self.document.builder())?);
            if !self.timer.running() {
                return Ok(());
            }
            self.browser.replace(Some(browser.clone()));
            self.shell.set_visible(true)?;
            let _ = browser.focus();
        }
        Ok(())
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        if let Err(error) = self.document.sync() {
            crate::diagnostics::warn!("Could not save plugin settings: {error}");
        }
    }
}

fn report(owner: &PreferencesWindow, error: &str) {
    crate::diagnostics::warn!("Plugin settings: {error}");
    owner.set_plugin_update_status(format!("Plugin settings: {error}").into());
}
