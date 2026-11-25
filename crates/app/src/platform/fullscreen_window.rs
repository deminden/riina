//! Request and observe fullscreen on the window that owns the video surface.
use crate::{NativeVideoHost, PlayerWindow};
use slint::winit_030::winit::event::WindowEvent;
use std::time::Duration;

pub(crate) struct FullscreenWindow {
    host: Option<slint::Weak<NativeVideoHost>>,
    #[cfg(target_os = "linux")]
    desktop: super::unix::fullscreen::Observation,
    #[cfg(target_os = "macos")]
    _notifications: super::macos::fullscreen::FullscreenNotifications,
}

impl FullscreenWindow {
    pub(crate) fn new(_app: &PlayerWindow, host: Option<slint::Weak<NativeVideoHost>>) -> Self {
        Self {
            host,
            #[cfg(target_os = "linux")]
            desktop: Default::default(),
            #[cfg(target_os = "macos")]
            _notifications: super::macos::fullscreen::FullscreenNotifications::new(_app),
        }
    }

    pub(crate) fn awaits_confirmation(&self) -> bool {
        self.host.is_none() && cfg!(any(target_os = "linux", target_os = "macos"))
    }

    pub(crate) fn request(&self, window: &slint::Window, active: bool) -> bool {
        if let Some(host) = &self.host {
            return host
                .upgrade()
                .is_some_and(|host| host.invoke_set_fullscreen_window(active));
        }
        #[cfg(target_os = "linux")]
        return self.desktop.request(window, active);
        #[cfg(target_os = "macos")]
        return super::macos::fullscreen::request_fullscreen(window, active).is_some();
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            window.set_fullscreen(active);
            true
        }
    }

    pub(crate) fn geometry_observed(&self, _window: &slint::Window, _active: bool, _repair: bool) {
        #[cfg(target_os = "linux")]
        self.desktop.geometry_observed(_window, _active, _repair);
    }

    pub(crate) fn confirmed(&self, _window: &slint::Window) -> Option<bool> {
        #[cfg(target_os = "linux")]
        return self.desktop.confirmed(_window);
        #[cfg(not(target_os = "linux"))]
        None
    }

    pub(crate) fn window_event(
        &self,
        _event: &WindowEvent,
        _app: &slint::Weak<PlayerWindow>,
        _may_repair: &(impl Fn(&PlayerWindow) -> bool + Clone + 'static),
    ) {
        #[cfg(target_os = "linux")]
        self.desktop.window_event(_event, _app, _may_repair);
    }
}

pub(crate) const CONFIRMATION_INTERVAL: Duration =
    Duration::from_millis(if cfg!(target_os = "linux") { 50 } else { 100 });

pub(crate) fn observed_fullscreen(_window: &slint::Window) -> Option<bool> {
    #[cfg(target_os = "linux")]
    return super::unix::fullscreen::observed_fullscreen(_window);
    #[cfg(not(target_os = "linux"))]
    None
}
