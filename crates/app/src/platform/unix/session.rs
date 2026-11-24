//! Linux owns the native webview presenter alongside the shared GL player.
use crate::platform::{
    PictureInPictureSession, PlayerScrollMonitor, PluginMenuObservation, PluginPresenters,
    SystemMediaSession,
};

pub(crate) struct SessionExtras {
    _plugin_presenters: PluginPresenters,
    _system_media: SystemMediaSession,
    picture_in_picture: PictureInPictureSession,
}

impl SessionExtras {
    pub(crate) fn new(
        presenters: PluginPresenters,
        system_media: SystemMediaSession,
        picture_in_picture: PictureInPictureSession,
        _menu_observation: PluginMenuObservation,
        _scroll_monitor: PlayerScrollMonitor,
    ) -> Self {
        Self {
            _plugin_presenters: presenters,
            _system_media: system_media,
            picture_in_picture,
        }
    }

    pub(crate) fn prepare_to_close(&self) {
        self.picture_in_picture.prepare_to_close();
        self._system_media.prepare_to_close();
    }
}
