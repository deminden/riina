//! Arrange native presentation and menu ownership before the shared player runs.

use crate::{RendererRequest, StartupArguments, preference_load::LoadedPreferences};
use riina_core::Preferences;
use slint::{BackendSelector, PlatformError};

pub(crate) fn run_player(
    arguments: StartupArguments,
    loaded: LoadedPreferences,
) -> Result<(), Box<dyn std::error::Error>> {
    let StartupArguments {
        renderer,
        media,
        resume_position,
        owner_center,
    } = arguments;
    #[cfg(target_os = "windows")]
    // Native D3D11 preserves the Windows HDR presentation path. Its control
    // overlay restores compositor alpha after Slint realizes the window.
    if renderer != RendererRequest::SharedGl {
        return super::windows::native_window::run(media, resume_position, owner_center, loaded);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = owner_center;
        if renderer == RendererRequest::NativeWindow {
            return Err("Native video presentation is currently available on Windows; use --shared-gl on this platform".into());
        }
    }
    crate::run_shared_player(media, resume_position, loaded)
}

pub(crate) fn select_shared_backend(preferences: &Preferences) -> Result<(), PlatformError> {
    let backend = BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg".into());
    #[cfg(target_os = "linux")]
    let backend =
        backend.with_winit_custom_application_handler(super::wayland_drop::Handler::default());
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

    pub(crate) fn normalize(&self) {
        #[cfg(target_os = "macos")]
        self.observer.normalize();
    }
}
