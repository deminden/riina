//! Hold the native player integrations for the lifetime of the session.
use super::{
    MacPictureInPicture, MacPluginMenuObserver, MacScrollEventMonitor, SystemMediaControls,
};
use crate::platform::PluginSidebarPresenter;
use std::{cell::RefCell, rc::Rc};

pub(crate) struct SessionExtras {
    pub(crate) _plugin_sidebar_presenter: Rc<RefCell<PluginSidebarPresenter>>,
    pub(crate) _system_media_controls: Rc<RefCell<SystemMediaControls>>,
    pub(crate) _system_media_controls_timer: Rc<slint::Timer>,
    pub(crate) _picture_in_picture: Rc<RefCell<Option<MacPictureInPicture>>>,
    pub(crate) _picture_in_picture_timer: Rc<slint::Timer>,
    pub(crate) _mac_scroll_event_monitor: Option<MacScrollEventMonitor>,
    pub(crate) _mac_plugin_menu_observer: Option<MacPluginMenuObserver>,
}

impl SessionExtras {
    pub(crate) fn prepare_to_close(&self) {
        self._system_media_controls
            .borrow_mut()
            .synchronize(false, "", None, None);
    }
}
