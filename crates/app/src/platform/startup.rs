//! Arrange native presentation and menu ownership before the shared player runs.

use crate::{
    browser_links::Inbox,
    player_manager::PlayerManager,
    preference_load::LoadedPreferences,
    startup_arguments::{RendererRequest, StartupArguments},
};
use riina_core::{ActionAfterLaunch, Preferences};
use slint::{BackendSelector, PlatformError};

pub(crate) fn new_shared_player() -> Result<crate::PlayerWindow, PlatformError> {
    #[cfg(target_os = "linux")]
    {
        super::playlist_menu::new_player()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let app = crate::PlayerWindow::new()?;
        // The shared renderer owns its own menu and playback surface.
        app.set_native_video(false);
        Ok(app)
    }
}

pub(crate) fn run_player(
    arguments: StartupArguments,
    loaded: LoadedPreferences,
    browser: &crate::browser_links::Inbox,
) -> Result<(), Box<dyn std::error::Error>> {
    let renderer = arguments.renderer;
    #[cfg(target_os = "windows")]
    // Native D3D11 preserves the Windows HDR presentation path. Its control
    // overlay restores compositor alpha after Slint realizes the window.
    if renderer != RendererRequest::SharedGl {
        let owner_center = arguments.owner_center;
        return super::windows::native_window::run(
            arguments.players(),
            owner_center,
            loaded,
            browser,
        );
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = arguments.owner_center;
        if renderer == RendererRequest::NativeWindow {
            return Err("Native video presentation is currently available on Windows; use --shared-gl on this platform".into());
        }
    }
    crate::run_shared_player(arguments.players(), loaded, browser)
}

pub(crate) fn select_shared_backend(preferences: &Preferences) -> Result<(), PlatformError> {
    let backend = BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg".into());
    #[cfg(target_os = "linux")]
    let backend =
        backend.with_winit_custom_application_handler(super::wayland_drop::Handler::default());
    #[cfg(not(target_os = "linux"))]
    let backend =
        backend.with_winit_custom_application_handler(super::plugin_standalone::NativeWindowEvents);
    #[cfg(target_os = "windows")]
    // Slint only derives native alpha-window attributes from its scene on
    // macOS. Create Windows auxiliary windows alpha-capable up front so a
    // later DWM material request can reveal Mica or Acrylic. Slint keeps
    // windows with its native menu opaque after this hook.
    let backend = {
        use slint::winit_030::winit::platform::windows::WindowAttributesExtWindows;
        backend.with_winit_window_attributes_hook(|attributes| {
            attributes.with_transparent(true).with_drag_and_drop(false)
        })
    };
    #[cfg(target_os = "macos")]
    let backend = {
        use slint::winit_030::winit::platform::macos::WindowAttributesExtMacOS;

        let accepts_first_mouse = preferences.controls.accepts_first_mouse;
        backend.with_winit_window_attributes_hook(move |attributes| {
            attributes.with_accepts_first_mouse(accepts_first_mouse)
        })
    };
    #[cfg(not(target_os = "macos"))]
    let _ = preferences;
    backend.select()
}

pub(crate) struct ApplicationMenu {
    #[cfg(target_os = "macos")]
    observer: super::macos::MacApplicationMenuObserver,
}

impl ApplicationMenu {
    pub(crate) fn new() -> Self {
        Self {
            #[cfg(target_os = "macos")]
            observer: super::macos::MacApplicationMenuObserver::new(),
        }
    }

    #[cfg(target_os = "macos")]
    fn labels(&self) -> std::rc::Rc<super::macos::menu_labels::MenuLabels> {
        self.observer.labels()
    }

    pub(crate) fn normalize(&self) {
        #[cfg(target_os = "macos")]
        self.observer.normalize();
    }
}

pub(crate) fn prepare_application(manager: &PlayerManager, preferences: &Preferences) {
    #[cfg(target_os = "macos")]
    match super::preferences_path() {
        Ok(path) => {
            manager
                .services()
                .configure_application_updates(&path, preferences);
            if let Err(error) = manager.prepare_application_plugins(&path) {
                crate::diagnostics::warn!("Could not load application plugins: {error}");
            }
        }
        Err(error) => {
            crate::diagnostics::warn!("Could not load the application update schedule: {error}")
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (manager, preferences);
}

pub(crate) struct ApplicationRegistration {
    #[cfg(target_os = "macos")]
    _events: Option<super::browser_activation::ApplicationEvents>,
}

impl ApplicationRegistration {
    pub(crate) fn new(
        manager: &PlayerManager,
        menu: &ApplicationMenu,
        browser: &Inbox,
        idle_launch: Option<ActionAfterLaunch>,
        preferences: &Preferences,
    ) -> Self {
        #[cfg(not(target_os = "macos"))]
        let _ = (manager, menu, browser, idle_launch, preferences);
        Self {
            #[cfg(target_os = "macos")]
            _events: super::browser_activation::ApplicationEvents::new(
                browser.clone(),
                manager.clone(),
                idle_launch,
                preferences,
                menu.labels(),
            ),
        }
    }

    pub(crate) fn needs_initial_player(requests_playback: bool) -> bool {
        !super::application_lifecycle::HAS_IDLE_MENU || requests_playback
    }

    pub(crate) fn run(&self) -> Result<(), PlatformError> {
        // Closing the last visible window can queue a replacement Welcome window.
        // The manager ends the loop once it has resolved that transition.
        slint::run_event_loop_until_quit()
    }
}
