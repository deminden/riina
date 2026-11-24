//! Present application-wide plugin UI with any active player as its owner.
use super::developer;
#[cfg(target_os = "macos")]
use super::windowless::Driver;
use crate::{
    PlayerWindow,
    platform::{plugin_dialogs, plugin_standalone},
    plugin_host::{GlobalHandle, GlobalHost},
    plugins::plugin_players::Ticket,
};
use std::{cell::RefCell, path::Path, rc::Rc, sync::Arc};

#[derive(Default)]
pub(crate) struct ApplicationPlugins {
    pub(super) services: RefCell<Option<Services>>,
    pub(super) managed: Rc<RefCell<ManagedWindows>>,
    #[cfg(target_os = "macos")]
    windowless: RefCell<Option<Rc<Driver>>>,
    #[cfg(target_os = "macos")]
    pub(crate) closed: Rc<crate::windows::close_observer::CloseObserver>,
}

pub(super) struct Services {
    pub(super) host: GlobalHost,
    pub(super) standalone: Rc<RefCell<plugin_standalone::Presenter>>,
    pub(super) developer: developer::Scope,
    pub(super) dialogs: plugin_dialogs::ApplicationOwners,
}

impl ApplicationPlugins {
    pub(crate) fn bind_player_launcher(
        &self,
        launcher: impl Fn(Option<&PlayerWindow>, Arc<Ticket>) -> Result<ClosePlayer, String> + 'static,
    ) {
        self.managed.borrow_mut().launcher = Some(Box::new(launcher));
    }

    pub(crate) fn host(&self, profile: &Path) -> Result<GlobalHandle, String> {
        let mut services = self.services.borrow_mut();
        if let Some(services) = services.as_ref() {
            return Ok(services.host.handle());
        }
        let host = GlobalHost::start(profile)?;
        let handle = host.handle();
        let dialogs = plugin_dialogs::ApplicationOwners::default();
        dialogs.install(&handle.ui.dialogs);
        let standalone = plugin_standalone::Presenter::new(handle.ui.windows.clone());
        #[cfg(target_os = "macos")]
        let standalone = {
            dialogs.bind_application_close(self.closed.clone());
            let mut standalone = standalone;
            standalone.bind_application_close(self.closed.clone());
            standalone
        };
        *services = Some(Services {
            host,
            standalone: Rc::new(RefCell::new(standalone)),
            developer: developer::Scope::new(handle.ui.developer.clone()),
            dialogs,
        });
        Ok(handle)
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn bind_windowless(
        self: &Rc<Self>,
        global: GlobalHandle,
        is_windowless: impl Fn() -> bool + 'static,
    ) {
        let owner = Rc::downgrade(self);
        let host = global.clone();
        let driver = Driver::new(move || {
            let Some(service) = owner.upgrade().filter(|_| is_windowless()) else {
                return false;
            };
            service.present_windowless(&host)
        });
        global.set_ui_waker(driver.waker());
        self.windowless.replace(Some(driver));
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn request_windowless(&self) {
        if let Some(driver) = self.windowless.borrow().as_ref() {
            driver.request();
        }
    }

    #[cfg(target_os = "macos")]
    fn present_windowless(&self, global: &GlobalHandle) -> bool {
        if let Ok(mut managed) = self.managed.try_borrow_mut() {
            managed.update_for_owner(None, global);
        }
        let standalone = self
            .services
            .borrow()
            .as_ref()
            .map(|services| services.standalone.clone());
        let Some(standalone) = standalone else {
            return false;
        };
        let Ok(mut standalone) = standalone.try_borrow_mut() else {
            return true;
        };
        standalone.update_from_application();
        standalone.is_visible()
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn auxiliary_is_visible(&self) -> bool {
        self.services.borrow().as_ref().is_some_and(|services| {
            services.dialogs.application_dialog_visible()
                || services
                    .standalone
                    .try_borrow()
                    .map_or(true, |presenter| presenter.is_visible())
        })
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn hide_for_quit(&self) {
        self.windowless.borrow_mut().take();
        if let Some(services) = self.services.borrow().as_ref() {
            services.host.handle().ui.dialogs.stop();
            if let Ok(mut standalone) = services.standalone.try_borrow_mut() {
                standalone.hide_for_quit();
            }
        }
    }
}

type ClosePlayer = Box<dyn FnOnce()>;
type LaunchPlayer = Box<dyn Fn(Option<&PlayerWindow>, Arc<Ticket>) -> Result<ClosePlayer, String>>;

#[derive(Default)]
pub(super) struct ManagedWindows {
    launcher: Option<LaunchPlayer>,
    active: Vec<(Arc<Ticket>, Option<ClosePlayer>)>,
}

impl ManagedWindows {
    pub(super) fn update(&mut self, app: &PlayerWindow, global: &GlobalHandle) {
        self.update_for_owner(Some(app), global);
    }

    fn update_for_owner(&mut self, app: Option<&PlayerWindow>, global: &GlobalHandle) {
        self.active.retain_mut(|(ticket, close)| {
            if ticket.is_open() {
                return true;
            }
            if let Some(close) = close.take() {
                close();
            }
            false
        });
        for ticket in global
            .take_player_launches()
            .into_iter()
            .filter(|ticket| ticket.is_open())
        {
            let result = self
                .launcher
                .as_ref()
                .ok_or_else(|| "Plugin player creation is unavailable".to_owned())
                .and_then(|launch| launch(app, ticket.clone()));
            match result {
                Ok(close) => self.active.push((ticket, Some(close))),
                Err(error) => {
                    ticket.close();
                    if let Some(app) = app {
                        crate::diagnostics::report(app, error);
                    } else {
                        crate::diagnostics::warn!("{error}");
                    }
                }
            }
        }
    }
}
