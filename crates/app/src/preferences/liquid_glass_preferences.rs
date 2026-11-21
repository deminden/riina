//! Translate saved per-surface material choices to the two shared UI windows.
use super::model::{PreferenceSection, SectionError};
use crate::{PlayerWindow, PreferencesWindow};
use riina_core::Preferences;
use riina_core::liquid_glass::SurfaceSettings;

impl PreferenceSection for SurfaceSettings {
    fn get(all: &Preferences) -> Self {
        all.liquid_glass
    }

    fn put(self, all: &mut Preferences) {
        all.liquid_glass = self;
    }

    fn read_window(window: &PreferencesWindow) -> Result<Self, SectionError> {
        Ok(current_preferences(window))
    }

    fn write_window(&self, window: &PreferencesWindow) {
        install_preferences(window, *self);
    }

    fn write_player(&self, app: &PlayerWindow) {
        install_player(app, *self);
    }
}

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
