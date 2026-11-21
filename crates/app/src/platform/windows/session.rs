//! Native overlay resources remain owned until the playback worker stops.
use crate::platform::{PictureInPictureSession, PluginOverlayPresenter, PluginSidebarPresenter};
use std::{cell::RefCell, rc::Rc};

pub(crate) struct SessionExtras {
    pub(crate) _plugin_overlay_presenter: Rc<RefCell<PluginOverlayPresenter>>,
    pub(crate) _plugin_sidebar_presenter: Rc<RefCell<PluginSidebarPresenter>>,
    pub(crate) _picture_in_picture: PictureInPictureSession,
}

impl SessionExtras {
    pub(crate) fn prepare_to_close(&self) {}
}
