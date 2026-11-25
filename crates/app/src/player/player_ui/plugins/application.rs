//! Present application-wide plugin UI with any active player as its owner.
use super::developer;
use crate::{
    PlayerWindow,
    platform::{plugin_application, plugin_dialogs, plugin_standalone},
    plugin_host::{GlobalHandle, GlobalHost},
    plugins::plugin_players::Ticket,
};
use std::{cell::RefCell, path::Path, rc::Rc, sync::Arc};

#[derive(Default)]
pub(crate) struct ApplicationPlugins {
    pub(super) services: RefCell<Option<Services>>,
    pub(super) managed: Rc<RefCell<ManagedWindows>>,
    platform: plugin_application::Bindings,
}

pub(super) struct Services {
    pub(super) host: GlobalHost,
    pub(super) standalone: Rc<RefCell<plugin_standalone::Presenter>>,
    pub(super) developer: developer::Scope,
    pub(super) dialogs: plugin_dialogs::ApplicationOwners,
}

impl ApplicationPlugins {
    pub(crate) fn bind_shortcut_owner(
        &self,
        owner: impl Fn() -> Option<slint::Weak<PlayerWindow>> + 'static,
    ) {
        self.platform.bind_shortcut_owner(owner);
    }

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
        let mut standalone = plugin_standalone::Presenter::new(handle.ui.windows.clone());
        self.platform.attach(&dialogs, &mut standalone);
        let mut developer = developer::Scope::new(handle.ui.developer.clone());
        developer.set_application_close(self.platform.close_observer());
        *services = Some(Services {
            host,
            standalone: Rc::new(RefCell::new(standalone)),
            developer,
            dialogs,
        });
        Ok(handle)
    }
}

#[cfg(target_os = "macos")]
impl ApplicationPlugins {
    pub(crate) fn bind_application_close(&self, callback: Rc<dyn Fn()>) {
        self.platform.bind_close(callback);
    }

    pub(crate) fn bind_windowless(
        self: &Rc<Self>,
        global: GlobalHandle,
        is_windowless: impl Fn() -> bool + 'static,
    ) {
        let owner = Rc::downgrade(self);
        let host = global.clone();
        let wake = self.platform.bind_windowless(move || {
            let Some(service) = owner.upgrade().filter(|_| is_windowless()) else {
                return false;
            };
            service.present_windowless(&host)
        });
        global.set_ui_waker(wake);
    }

    pub(crate) fn request_windowless(&self) {
        self.platform.request_windowless();
    }

    fn present_windowless(&self, global: &GlobalHandle) -> bool {
        crate::platform::browser_activation::refresh_idle_plugins();
        if let Ok(mut managed) = self.managed.try_borrow_mut() {
            managed.update_for_owner(None, global);
        }
        let views = self
            .services
            .borrow()
            .as_ref()
            .map(|services| (services.standalone.clone(), services.developer.clone()));
        let Some((standalone, developer)) = views else {
            return false;
        };
        let console_visible = developer.present_windowless();
        let Ok(mut standalone) = standalone.try_borrow_mut() else {
            return true;
        };
        standalone.update_from_application();
        standalone.has_window_activity() || console_visible
    }

    pub(crate) fn global_handle(&self) -> Option<GlobalHandle> {
        self.services
            .borrow()
            .as_ref()
            .map(|services| services.host.handle())
    }

    pub(crate) fn open_developer_tool(&self, realm: u64) -> Result<(), String> {
        let developer = self
            .services
            .borrow()
            .as_ref()
            .map(|services| services.developer.clone())
            .ok_or("The global plugin host is unavailable")?;
        developer.open_windowless(realm)?;
        self.request_windowless();
        Ok(())
    }

    pub(crate) fn auxiliary_is_visible(&self) -> bool {
        self.services.borrow().as_ref().is_some_and(|services| {
            services.dialogs.application_dialog_visible()
                || services.developer.is_visible()
                || services
                    .standalone
                    .try_borrow()
                    .map_or(true, |presenter| presenter.has_window_activity())
        })
    }

    pub(crate) fn hide_for_quit(&self) {
        self.platform.stop_windowless();
        if let Some(services) = self.services.borrow().as_ref() {
            services.host.handle().ui.dialogs.stop();
            services.developer.close_all();
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
