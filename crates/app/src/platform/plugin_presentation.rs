//! Keep native plugin surfaces and their ownership on the platform side.
use crate::{
    PlayerWindow,
    platform::{NativePresentation, PluginSidebarPresenter},
    plugin_host::{PluginHostHandle, PluginHostMessage},
};
use std::{cell::RefCell, rc::Rc};

#[derive(Default)]
pub(crate) struct PluginPresenters {
    #[cfg(target_os = "windows")]
    overlay: Rc<RefCell<super::PluginOverlayPresenter>>,
    pub(crate) sidebar: Rc<RefCell<PluginSidebarPresenter>>,
}

impl PluginPresenters {
    pub(crate) fn overlay_delivery(
        &self,
        native: Option<&NativePresentation>,
        host: &PluginHostHandle,
    ) -> PluginOverlayDelivery {
        #[cfg(target_os = "windows")]
        {
            PluginOverlayDelivery {
                presenter: self.overlay.clone(),
                parent: native.map(NativePresentation::webview_parent_window),
                host: host.clone(),
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = (native, host);
            PluginOverlayDelivery {}
        }
    }
}

pub(crate) struct PluginOverlayDelivery {
    #[cfg(target_os = "windows")]
    presenter: Rc<RefCell<super::PluginOverlayPresenter>>,
    #[cfg(target_os = "windows")]
    parent: Option<std::sync::Arc<slint::winit_030::winit::window::Window>>,
    #[cfg(target_os = "windows")]
    host: PluginHostHandle,
}

impl PluginOverlayDelivery {
    pub(crate) fn apply(&self, message: PluginHostMessage) -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            let parent = self
                .parent
                .as_ref()
                .ok_or("Plugin video overlays require native video presentation")?;
            self.presenter
                .borrow_mut()
                .apply_host_message(parent, &self.host, message)
                .map(|_| ())
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = message;
            Err("Plugin video overlays are unavailable on this platform".into())
        }
    }

    pub(crate) fn remove(&self, identifier: &str) {
        #[cfg(target_os = "windows")]
        self.presenter.borrow_mut().remove(identifier);
        #[cfg(not(target_os = "windows"))]
        let _ = identifier;
    }

    pub(crate) fn update(&self, app: &PlayerWindow) {
        #[cfg(target_os = "windows")]
        {
            use slint::ComponentHandle;
            self.presenter.borrow_mut().update(
                app.get_window_width(),
                app.get_window_height(),
                f64::from(app.window().scale_factor()),
            );
        }
        #[cfg(not(target_os = "windows"))]
        let _ = app;
    }
}

pub(crate) struct PluginMenuObservation {
    #[cfg(target_os = "macos")]
    _observer: Option<super::MacPluginMenuObserver>,
}

impl PluginMenuObservation {
    pub(crate) fn install(app: &PlayerWindow, host: Option<&PluginHostHandle>) -> Self {
        #[cfg(target_os = "macos")]
        {
            Self {
                _observer: host.and_then(|host| {
                    match super::MacPluginMenuObserver::new(app, host.clone()) {
                        Ok(observer) => Some(observer),
                        Err(error) => {
                            app.set_status(
                                format!("Could not observe the Plugin menu: {error}").into(),
                            );
                            None
                        }
                    }
                }),
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = (app, host);
            Self {}
        }
    }
}

pub(crate) fn pump_plugin_webviews() {
    #[cfg(target_os = "linux")]
    super::pump_linux_webview_events();
}
