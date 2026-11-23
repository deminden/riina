//! Keep native plugin surfaces and their ownership on the platform side.
use crate::{
    PlayerWindow,
    platform::{NativePresentation, PluginSidebarPresenter},
    plugin_host::{PluginHostHandle, PluginHostMessage},
};
use std::{cell::RefCell, rc::Rc};

pub(crate) struct PluginPresenters {
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    overlay: Rc<RefCell<super::plugin_overlay::Presenter>>,
    pub(crate) sidebar: Rc<RefCell<PluginSidebarPresenter>>,
}

impl PluginPresenters {
    pub(crate) fn new(app: &PlayerWindow) -> Self {
        #[cfg(target_os = "linux")]
        let overlay = {
            use slint::ComponentHandle;
            let presenter: Rc<RefCell<super::plugin_overlay::Presenter>> = Rc::default();
            let overlay = Rc::downgrade(&presenter);
            app.global::<riina_ui::PlayerControlActions>()
                .on_plugin_overlay_pointer(move |phase, button, x, y| {
                    overlay.upgrade().is_some_and(|presenter| {
                        presenter.try_borrow_mut().is_ok_and(|mut presenter| {
                            presenter.pointer(phase, button, (f64::from(x), f64::from(y)))
                        })
                    })
                });
            presenter
        };
        Self {
            #[cfg(target_os = "linux")]
            overlay,
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            overlay: Rc::default(),
            sidebar: Rc::new(RefCell::new(PluginSidebarPresenter::new(app))),
        }
    }

    pub(crate) fn overlay_delivery(
        &self,
        native: Option<&NativePresentation>,
        host: &PluginHostHandle,
    ) -> PluginOverlayDelivery {
        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
        {
            #[cfg(not(target_os = "windows"))]
            let _ = native;
            PluginOverlayDelivery {
                presenter: Rc::downgrade(&self.overlay),
                #[cfg(target_os = "windows")]
                parent: native.map(NativePresentation::webview_parent_window),
                host: host.clone(),
            }
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            let _ = (native, host);
            PluginOverlayDelivery {}
        }
    }
}

#[derive(Clone)]
pub(crate) struct PluginOverlayDelivery {
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    presenter: std::rc::Weak<RefCell<super::plugin_overlay::Presenter>>,
    #[cfg(target_os = "windows")]
    parent: Option<std::sync::Arc<slint::winit_030::winit::window::Window>>,
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    host: PluginHostHandle,
}

impl PluginOverlayDelivery {
    pub(crate) fn apply(&self, message: PluginHostMessage) -> Result<(), String> {
        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
        {
            if let Some(presenter) = self.presenter.upgrade() {
                let mut presenter = presenter.borrow_mut();
                if presenter.pending.len() >= 512 {
                    return Err("Plugin overlay presenter has pending updates".into());
                }
                presenter.pending.push(message);
            }
            Ok(())
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            let _ = message;
            Err("Plugin video overlays are unavailable on this platform".into())
        }
    }

    pub(crate) fn remove(&self, identifier: &str) {
        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
        if let Some(presenter) = self.presenter.upgrade() {
            let mut presenter = presenter.borrow_mut();
            presenter.pending.retain(|message| match message {
                PluginHostMessage::Overlay { route, .. } => route.identifier != identifier,
                PluginHostMessage::RemoveOverlay(plugin_identifier) => {
                    plugin_identifier != identifier
                }
                _ => true,
            });
            presenter
                .pending
                .push(PluginHostMessage::RemoveOverlay(identifier.into()));
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        let _ = identifier;
    }

    pub(crate) fn update(&self, app: &PlayerWindow) {
        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
        {
            use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
            let Some(presenter) = self.presenter.upgrade() else {
                return;
            };
            #[cfg(target_os = "windows")]
            if let Some(parent) = self.parent.as_deref() {
                let bounds = super::plugin_overlay::bounds(
                    0.0,
                    0.0,
                    app.get_window_width(),
                    app.get_window_height(),
                );
                self.update_surface(&presenter, parent, bounds, true, app);
                return;
            }
            let bounds = super::plugin_overlay::bounds(
                app.get_video_origin_x(),
                app.get_video_origin_y(),
                app.get_video_width(),
                app.get_video_height(),
            );
            let dialog = app.global::<riina_ui::PlayerDialogState>();
            let allowed = app.get_video_width() > 0.0
                && app.get_video_height() > 0.0
                && !app
                    .global::<riina_ui::PlayerMusicModeState>()
                    .get_music_mode()
                && !app.global::<riina_ui::PlayerCropState>().get_crop_editing()
                && !dialog.get_dialog_open()
                && !dialog.get_jump_time_dialog_visible()
                && !dialog.get_url_dialog_visible()
                && !dialog.get_plugin_interaction_visible();
            app.window().with_winit_window(|parent| {
                self.update_surface(&presenter, parent, bounds, allowed, app)
            });
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        let _ = app;
    }

    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    fn update_surface(
        &self,
        presenter: &RefCell<super::plugin_overlay::Presenter>,
        parent: &slint::winit_030::winit::window::Window,
        bounds: wry::Rect,
        allowed: bool,
        app: &PlayerWindow,
    ) {
        let Ok(mut presenter) = presenter.try_borrow_mut() else {
            return;
        };
        for message in std::mem::take(&mut presenter.pending) {
            if let Err(error) = presenter.dispatch(parent, &self.host, message, bounds) {
                crate::diagnostics::report(app, error);
            }
        }
        if let Err(error) = presenter.update(bounds, allowed) {
            crate::diagnostics::report(app, error);
        }
        #[cfg(target_os = "linux")]
        if let Some(frames) = presenter.frames() {
            use slint::ComponentHandle;
            app.global::<riina_ui::PlayerRenderedVideo>()
                .set_plugin_overlays(slint::ModelRc::new(slint::VecModel::from(frames)));
        }
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
                            crate::diagnostics::report(
                                app,
                                format!("Could not observe the Plugin menu: {error}"),
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
