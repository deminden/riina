//! Translate saved per-surface material choices to the two shared UI windows.
use crate::{PlayerWindow, PreferencesWindow};
use riina_core::liquid_glass::SurfaceSettings;

pub(crate) fn current_player(window: &PlayerWindow) -> SurfaceSettings {
    SurfaceSettings {
        osd: window.get_liquid_glass_osd(),
        osc: window.get_liquid_glass_osc(),
        sidebar: window.get_liquid_glass_sidebar(),
    }
}

pub(crate) fn current_preferences(window: &PreferencesWindow) -> SurfaceSettings {
    SurfaceSettings {
        osd: window.get_liquid_glass_osd(),
        osc: window.get_liquid_glass_osc(),
        sidebar: window.get_liquid_glass_sidebar(),
    }
}

pub(crate) fn install_player(window: &PlayerWindow, settings: SurfaceSettings) {
    window.set_liquid_glass_osd(settings.osd);
    window.set_liquid_glass_osc(settings.osc);
    window.set_liquid_glass_sidebar(settings.sidebar);
}

pub(crate) fn install_preferences(window: &PreferencesWindow, settings: SurfaceSettings) {
    window.set_liquid_glass_osd(settings.osd);
    window.set_liquid_glass_osc(settings.osc);
    window.set_liquid_glass_sidebar(settings.sidebar);
}
