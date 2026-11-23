//! Key profile boundaries shared by playback, menus and the Preferences editor.
mod bindings;
mod builtins;
mod dispatch;
mod editor;
mod keys;
mod menus;

use super::model::{PreferenceSection, SectionError};
use crate::{PlayerWindow, PreferencesWindow};
use riina_core::{Preferences, key_bindings::Settings};
use slint::ComponentHandle;

pub(crate) use builtins::mappings_for;
pub(crate) use dispatch::{
    CurrentMediaLocation, current_media_location, install_player, reveal_current_media,
};
#[cfg(target_os = "macos")]
pub(crate) use dispatch::{dispatch_system_media_key, system_skip_intervals_for_profile};
pub(crate) use editor::{
    activate_profile, current_player, current_window, install_window, mapping_for_row, refresh,
    remove_mapping, save_mapping, search, select_profile,
};
pub(crate) use keys::{recorded_key, shortcut_keys};

impl PreferenceSection for Settings {
    fn get(all: &Preferences) -> Self {
        all.key_bindings.clone()
    }

    fn put(self, all: &mut Preferences) {
        all.key_bindings = self;
    }

    fn read_window(window: &PreferencesWindow) -> Result<Self, SectionError> {
        current_window(window).map_err(SectionError::from)
    }

    fn write_window(&self, window: &PreferencesWindow) {
        if let Err(error) = refresh(window, self) {
            crate::diagnostics::warn!("Could not refresh the key binding settings: {error}");
        }
    }

    fn write_player(&self, app: &PlayerWindow) {
        let player_preferences = app.global::<riina_ui::PlayerPreferences>();
        player_preferences.set_key_binding_current_profile(self.current_profile.clone().into());
        player_preferences
            .set_key_binding_use_system_media_controls(self.use_system_media_controls);
        player_preferences.set_key_binding_display_raw_values(self.display_raw_values);
    }
}

#[cfg(test)]
mod tests;
