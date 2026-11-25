//! Request and observe fullscreen on the window that owns the video surface.
use crate::{NativeVideoHost, PlayerWindow};
use slint::ComponentHandle;
use slint::winit_030::winit::event::WindowEvent;
use std::time::Duration;

pub(crate) enum Completion {
    Immediate,
    Desktop,
}

pub(crate) struct FullscreenWindow {
    host: Option<slint::Weak<NativeVideoHost>>,
    #[cfg(target_os = "linux")]
    desktop: super::unix::fullscreen::Observation,
    #[cfg(target_os = "macos")]
    _notifications: super::macos::fullscreen::FullscreenNotifications,
    #[cfg(target_os = "macos")]
    legacy: std::rc::Rc<super::macos::fullscreen::Legacy>,
}

impl FullscreenWindow {
    pub(crate) fn new(_app: &PlayerWindow, host: Option<slint::Weak<NativeVideoHost>>) -> Self {
        Self {
            host,
            #[cfg(target_os = "linux")]
            desktop: Default::default(),
            #[cfg(target_os = "macos")]
            _notifications: super::macos::fullscreen::FullscreenNotifications::new(_app),
            #[cfg(target_os = "macos")]
            legacy: super::macos::fullscreen::Legacy::install(_app),
        }
    }

    pub(crate) fn awaits_confirmation(&self) -> bool {
        self.host.is_none() && cfg!(any(target_os = "linux", target_os = "macos"))
    }

    pub(crate) fn request(&self, app: &PlayerWindow, active: bool) -> Option<Completion> {
        if let Some(host) = &self.host {
            return host
                .upgrade()
                .filter(|host| host.invoke_set_fullscreen_window(active))
                .map(|_| Completion::Immediate);
        }
        #[cfg(target_os = "macos")]
        if let Some(accepted) = self.legacy.request(app, active) {
            return accepted.then_some(Completion::Immediate);
        }
        self.request_desktop(app.window(), active)
            .then_some(if self.awaits_confirmation() {
                Completion::Desktop
            } else {
                Completion::Immediate
            })
    }

    fn request_desktop(&self, window: &slint::Window, active: bool) -> bool {
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
        #[cfg(target_os = "macos")]
        if !_active {
            self.legacy.update_collection();
        }
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
        #[cfg(target_os = "macos")]
        self.legacy.window_event(_event);
    }

    pub(crate) fn retire(&self) {
        #[cfg(target_os = "macos")]
        self.legacy.retire();
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

pub(crate) fn owns_window_level(_window: &slint::winit_030::winit::window::Window) -> bool {
    #[cfg(target_os = "macos")]
    {
        use slint::winit_030::winit::platform::macos::WindowExtMacOS;
        _window.simple_fullscreen()
    }
    #[cfg(not(target_os = "macos"))]
    false
}
