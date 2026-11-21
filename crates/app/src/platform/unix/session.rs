//! Linux owns the native webview presenter alongside the shared GL player.
use crate::platform::PluginSidebarPresenter;
use std::{cell::RefCell, rc::Rc};

pub(crate) struct SessionExtras {
    pub(crate) _plugin_sidebar_presenter: Rc<RefCell<PluginSidebarPresenter>>,
}

impl SessionExtras {
    pub(crate) fn prepare_to_close(&self) {}
}
