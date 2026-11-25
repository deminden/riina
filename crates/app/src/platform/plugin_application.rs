//! Attach application plugin surfaces to the native application lifetime.
#[cfg(target_os = "macos")]
mod windowless;

use super::{plugin_dialogs::ApplicationOwners, plugin_standalone::Presenter};
use crate::{
    PlayerWindow,
    plugin_runtime::{UiEffect, utilities::PluginDialogs},
    windows::close_observer::CloseObserver,
};
use std::rc::Rc;

pub(crate) const SUPPORTS_WINDOWLESS: bool = cfg!(target_os = "macos");

#[derive(Default)]
pub(crate) struct Bindings {
    #[cfg(target_os = "linux")]
    shortcuts: std::cell::OnceCell<super::plugin_standalone::ShortcutOwner>,
    #[cfg(target_os = "macos")]
    closed: Rc<CloseObserver>,
    #[cfg(target_os = "macos")]
    windowless: std::cell::RefCell<Option<Rc<windowless::Driver>>>,
}

impl Bindings {
    pub(crate) fn bind_shortcut_owner(
        &self,
        owner: impl Fn() -> Option<slint::Weak<PlayerWindow>> + 'static,
    ) {
        #[cfg(target_os = "linux")]
        let _ = self.shortcuts.set(Rc::new(owner));
        #[cfg(not(target_os = "linux"))]
        let _ = owner;
    }

    pub(crate) fn attach(&self, dialogs: &ApplicationOwners, standalone: &mut Presenter) {
        #[cfg(target_os = "linux")]
        if let Some(owner) = self.shortcuts.get() {
            standalone.bind_application_owner(owner.clone());
        }
        #[cfg(target_os = "macos")]
        {
            dialogs.bind_application_close(self.closed.clone());
            standalone.bind_application_close(self.closed.clone());
        }
        #[cfg(not(target_os = "macos"))]
        let _ = dialogs;
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        let _ = standalone;
    }

    pub(crate) fn close_observer(&self) -> Option<Rc<CloseObserver>> {
        #[cfg(target_os = "macos")]
        {
            Some(self.closed.clone())
        }
        #[cfg(not(target_os = "macos"))]
        {
            None
        }
    }
}

#[cfg(target_os = "macos")]
impl Bindings {
    pub(crate) fn bind_close(&self, callback: Rc<dyn Fn()>) {
        self.closed.bind(callback);
    }

    pub(crate) fn bind_windowless<F>(&self, present: F) -> impl Fn() + Send + Sync + use<F>
    where
        F: Fn() -> bool + 'static,
    {
        let driver = windowless::Driver::new(present);
        let wake = driver.waker();
        self.windowless.replace(Some(driver));
        wake
    }

    pub(crate) fn request_windowless(&self) {
        if let Some(driver) = self.windowless.borrow().as_ref() {
            driver.request();
        }
    }

    pub(crate) fn stop_windowless(&self) {
        self.windowless.borrow_mut().take();
    }
}

pub(crate) fn present_without_player(dialogs: &PluginDialogs, title: &str, effect: UiEffect) {
    if SUPPORTS_WINDOWLESS
        && let UiEffect::ShowAlert(message) = effect
        && let Err(error) = dialogs.notice(title.to_owned(), message)
    {
        crate::diagnostics::warn!("Could not show plugin alert: {error}");
    }
}
