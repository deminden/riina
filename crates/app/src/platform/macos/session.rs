//! Hold the native player integrations for the lifetime of the session.
use crate::platform::{
    PictureInPictureSession, PlayerScrollMonitor, PluginMenuObservation, PluginPresenters,
    SystemMediaSession,
};

pub(crate) struct SessionExtras {
    _plugin_presenters: PluginPresenters,
    _system_media: SystemMediaSession,
    _picture_in_picture: PictureInPictureSession,
    _scroll_monitor: PlayerScrollMonitor,
    _plugin_menu_observation: PluginMenuObservation,
}

impl SessionExtras {
    pub(crate) fn new(
        presenters: PluginPresenters,
        system_media: SystemMediaSession,
        picture_in_picture: PictureInPictureSession,
        menu_observation: PluginMenuObservation,
        scroll_monitor: PlayerScrollMonitor,
    ) -> Self {
        Self {
            _plugin_presenters: presenters,
            _system_media: system_media,
            _picture_in_picture: picture_in_picture,
            _scroll_monitor: scroll_monitor,
            _plugin_menu_observation: menu_observation,
        }
    }

    pub(crate) fn prepare_to_close(&self) {
        self._system_media.prepare_to_close();
    }
}
