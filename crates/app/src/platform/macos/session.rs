//! Hold the native player integrations for the lifetime of the session.
use super::{MacPluginMenuObserver, MacScrollEventMonitor};
use crate::platform::{PictureInPictureSession, PluginSidebarPresenter, SystemMediaSession};
use std::{cell::RefCell, rc::Rc};

pub(crate) struct SessionExtras {
    pub(crate) _plugin_sidebar_presenter: Rc<RefCell<PluginSidebarPresenter>>,
    pub(crate) _system_media: SystemMediaSession,
    pub(crate) _picture_in_picture: PictureInPictureSession,
    pub(crate) _mac_scroll_event_monitor: Option<MacScrollEventMonitor>,
    pub(crate) _mac_plugin_menu_observer: Option<MacPluginMenuObserver>,
}

impl SessionExtras {
    pub(crate) fn prepare_to_close(&self) {
        self._system_media.prepare_to_close();
    }
}
