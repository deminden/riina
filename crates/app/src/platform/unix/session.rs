//! Linux owns the native webview presenter alongside the shared GL player.
use crate::platform::{PictureInPictureSession, PluginSidebarPresenter, SystemMediaSession};
use std::{cell::RefCell, rc::Rc};

pub(crate) struct SessionExtras {
    pub(crate) _plugin_sidebar_presenter: Rc<RefCell<PluginSidebarPresenter>>,
    pub(crate) _system_media: SystemMediaSession,
    pub(crate) _picture_in_picture: PictureInPictureSession,
}

impl SessionExtras {
    pub(crate) fn prepare_to_close(&self) {
        self._system_media.prepare_to_close();
    }
}
