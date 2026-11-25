//! Native policy for an application with no visible player.
use crate::player_ui::ApplicationServices;
use std::rc::Rc;

pub(crate) const HAS_IDLE_MENU: bool = cfg!(target_os = "macos");

pub(crate) fn bind_auxiliary_close(services: &ApplicationServices, callback: Rc<dyn Fn()>) {
    if HAS_IDLE_MENU {
        services.about.closed.bind(callback.clone());
        services.logs.closed.bind(callback.clone());
        services.open_url.closed.bind(callback.clone());
        services.updates.closed.bind(callback.clone());
        services.highlights.closed.bind(callback.clone());
        #[cfg(target_os = "macos")]
        {
            services.settings.profile().closed.bind(callback.clone());
            services.plugins.bind_application_close(callback.clone());
        }
    }
    services.filters.closed.bind(callback);
}

pub(crate) fn auxiliary_is_visible(services: &ApplicationServices) -> bool {
    if services.history.is_visible() || services.filters.is_visible() {
        return true;
    }
    if HAS_IDLE_MENU
        && (services.about.is_visible()
            || services.logs.is_visible()
            || services.open_url.is_visible()
            || services.updates.is_visible()
            || services.highlights.is_visible())
    {
        return true;
    }
    #[cfg(target_os = "macos")]
    {
        services.settings.profile().is_visible() || services.plugins.auxiliary_is_visible()
    }
    #[cfg(not(target_os = "macos"))]
    false
}

pub(crate) fn hide_auxiliary_tools(services: &ApplicationServices) {
    services.history.hide();
    services.filters.hide();
    if HAS_IDLE_MENU {
        services.about.hide();
        services.logs.hide();
        services.open_url.hide_for_quit();
        services.updates.hide_for_quit();
        services.highlights.hide();
        #[cfg(target_os = "macos")]
        {
            services.settings.profile().hide();
            services.plugins.hide_for_quit();
        }
    }
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

#[cfg(target_os = "macos")]
pub(crate) fn show_about(
    services: &ApplicationServices,
    preferences: &riina_core::Preferences,
) -> Result<(), slint::PlatformError> {
    use slint::ComponentHandle;

    let about = &services.about;
    let window = about.window_for(None)?;
    about.stop_refresh();
    about.probe.use_application(&window);
    window.set_dark_theme(
        preferences
            .effective_theme_material()
            .dark_for(preferences.dark),
    );
    crate::about_window::reveal(&window)?;
    window.set_dark_theme(profile_window_theme(window.window(), preferences));
    about.probe.request(&window);
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn show_highlights(
    services: &ApplicationServices,
    preferences: &riina_core::Preferences,
) -> Result<(), slint::PlatformError> {
    use slint::ComponentHandle;

    let dark = preferences
        .effective_theme_material()
        .dark_for(preferences.dark);
    let window = services.highlights.show(None, None, dark)?;
    window.set_dark_theme(profile_window_theme(window.window(), preferences));
    Ok(())
}

#[cfg(target_os = "macos")]
fn profile_window_theme(window: &slint::Window, preferences: &riina_core::Preferences) -> bool {
    use slint::winit_030::{WinitWindowAccessor, winit::window::Theme};

    let dark = window
        .with_winit_window(slint::winit_030::winit::window::Window::theme)
        .flatten()
        .map_or(preferences.dark, |theme| theme == Theme::Dark);
    preferences.effective_theme_material().dark_for(dark)
}

#[cfg(test)]
mod tests {
    use super::*;
    use slint::ComponentHandle;

    #[test]
    fn about_participates_in_the_idle_menu_lifecycle() -> Result<(), slint::PlatformError> {
        i_slint_backend_testing::init_no_event_loop();
        let services = ApplicationServices::default();
        let window = services.about.window_for(None)?;
        assert!(!auxiliary_is_visible(&services));

        window.show()?;
        assert_eq!(auxiliary_is_visible(&services), HAS_IDLE_MENU);
        hide_auxiliary_tools(&services);
        assert_eq!(window.window().is_visible(), !HAS_IDLE_MENU);
        assert!(!auxiliary_is_visible(&services));
        window.hide()
    }
}
