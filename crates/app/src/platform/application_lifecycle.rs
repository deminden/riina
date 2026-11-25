//! Native policy for an application with no visible player.
use crate::player_ui::ApplicationServices;
use std::rc::Rc;

pub(crate) const HAS_IDLE_MENU: bool = cfg!(target_os = "macos");

pub(crate) fn bind_auxiliary_close(services: &ApplicationServices, callback: Rc<dyn Fn()>) {
    #[cfg(target_os = "macos")]
    services.bind_auxiliary_close(callback);
    #[cfg(not(target_os = "macos"))]
    services.filters.closed.bind(callback);
}

pub(crate) fn auxiliary_is_visible(services: &ApplicationServices) -> bool {
    #[cfg(target_os = "macos")]
    {
        services.auxiliary_is_visible()
    }
    #[cfg(not(target_os = "macos"))]
    {
        services.history.is_visible() || services.filters.is_visible()
    }
}

pub(crate) fn hide_auxiliary_tools(services: &ApplicationServices) {
    services.history.hide();
    services.filters.hide();
    #[cfg(target_os = "macos")]
    services.hide_auxiliary_tools();
}

pub(crate) fn after_last_player_closed(services: &ApplicationServices) {
    #[cfg(target_os = "macos")]
    services.plugins.request_windowless();
    #[cfg(not(target_os = "macos"))]
    let _ = services;
}

pub(crate) fn show_idle_menu() {
    #[cfg(target_os = "macos")]
    super::browser_activation::show_idle_menu();
}
