//! Key profile boundaries shared by playback, menus and the Preferences editor.
mod bindings;
mod builtins;
mod dispatch;
mod editor;
mod keys;
mod menus;

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

#[cfg(test)]
mod tests;
