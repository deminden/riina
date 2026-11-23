//! Present application-wide plugin UI with any active player as its owner.
use super::developer;
use crate::{
    platform::{plugin_dialogs, plugin_standalone},
    plugin_host::{GlobalHandle, GlobalHost},
};
use std::{cell::RefCell, path::Path, rc::Rc};

#[derive(Default)]
pub(crate) struct ApplicationPlugins {
    pub(super) services: RefCell<Option<Services>>,
}

pub(super) struct Services {
    host: GlobalHost,
    pub(super) standalone: Rc<RefCell<plugin_standalone::Presenter>>,
    pub(super) developer: developer::Scope,
    pub(super) dialogs: plugin_dialogs::ApplicationOwners,
}

impl ApplicationPlugins {
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
