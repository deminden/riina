//! Present application-wide plugin UI with any active player as its owner.
use super::developer;
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
        launcher: impl Fn(&PlayerWindow, Arc<Ticket>) -> Result<ClosePlayer, String> + 'static,
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
        *services = Some(Services {
            host,
            standalone: Rc::new(RefCell::new(plugin_standalone::Presenter::new(
                handle.ui.windows.clone(),
            ))),
            developer: developer::Scope::new(handle.ui.developer.clone()),
            dialogs,
        });
        Ok(handle)
    }
}

type ClosePlayer = Box<dyn FnOnce()>;
type LaunchPlayer = Box<dyn Fn(&PlayerWindow, Arc<Ticket>) -> Result<ClosePlayer, String>>;

#[derive(Default)]
pub(super) struct ManagedWindows {
    launcher: Option<LaunchPlayer>,
    active: Vec<(Arc<Ticket>, Option<ClosePlayer>)>,
}

impl ManagedWindows {
    pub(super) fn update(&mut self, app: &PlayerWindow, global: &GlobalHandle) {
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
                    crate::diagnostics::report(app, error);
                }
            }
        }
    }
}
