//! Own the installed-plugin Settings controllers and their shared inventory.
mod installation;
mod inventory;
mod permissions;
mod row_actions;
mod update_actions;
mod updates;

use crate::{PlayerWindow, PreferencesWindow, plugin_host::PluginHostHandle};
pub(crate) use inventory::current_runtime_supports;
use inventory::{load_enabled, refresh};
use riina_core::plugin::Manifest;
use std::{
    cell::RefCell,
    collections::BTreeSet,
    path::{Path, PathBuf},
    rc::Rc,
};

#[derive(Clone)]
struct InstalledPlugin {
    manifest: Manifest,
    directory: PathBuf,
}

struct PluginSettings {
    plugins: PathBuf,
    preferences_path: PathBuf,
    inventory: Rc<RefCell<Vec<InstalledPlugin>>>,
    enabled_path: PathBuf,
    enabled: Rc<RefCell<BTreeSet<String>>>,
    runtime_available: bool,
}

pub(crate) fn connect(
    window: &PreferencesWindow,
    app: &PlayerWindow,
    preferences_path: &Path,
    host: Option<PluginHostHandle>,
    player: riina_mpv::CommandSender,
) {
    let enabled_path = preferences_path.with_file_name("plugin-enabled.json");
    let settings = PluginSettings {
        plugins: preferences_path.with_file_name("Plugins"),
        preferences_path: preferences_path.to_owned(),
        inventory: Rc::default(),
        enabled: Rc::new(RefCell::new(load_enabled(&enabled_path))),
        enabled_path,
        runtime_available: host.is_some(),
    };
    refresh(
        window,
        &settings.plugins,
        &settings.inventory,
        &settings.enabled.borrow(),
        settings.runtime_available,
    );
    window.on_plugin_id_transfer(slint::DataTransfer::from);
    window.on_plugin_id_from_transfer(|transfer| {
        transfer.plain_text().unwrap_or_default().to_string().into()
    });
    installation::local(window, app, &settings);
    installation::github(window, app, &settings);
    let requests = update_actions::Requests::new();
    update_actions::register_refresh(window, &settings, &requests);
    row_actions::register_reorder(window, &settings);
    update_actions::register_staged(window, &settings, &requests, host.as_ref());
    update_actions::register_completion(window, &settings, &requests);
    row_actions::register_enabled(window, app, &settings, host.as_ref(), player);
    update_actions::register_about(window, app, &settings, &requests);
    row_actions::register_open_folder(window, &settings);
    row_actions::register_settings_page(window, &settings);
}

#[cfg(test)]
mod tests {
    use super::inventory::runtime_supports_permission;

    #[test]
    fn plugin_runtime_advertises_reviewed_permissions() {
        assert!(runtime_supports_permission("network-request"));
        assert!(runtime_supports_permission("file-system"));
        assert!(runtime_supports_permission("process-execution"));
        assert!(runtime_supports_permission("open-external"));
        assert!(!runtime_supports_permission("unknown-permission"));
    }
}
