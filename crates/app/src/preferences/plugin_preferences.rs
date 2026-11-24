//! Own the installed-plugin Settings controllers and their shared inventory.
mod about;
mod installation;
mod inventory;
mod permissions;
mod removal;
mod row_actions;
mod update_actions;
mod updates;

use crate::{
    PlayerWindow, PreferencesWindow, plugin_host::GlobalHandle, plugin_packages::InstalledPlugin,
};
pub(crate) use inventory::current_runtime_supports;
use inventory::{load_enabled, refresh};
use riina_mpv::CommandSender;
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    collections::BTreeSet,
    path::{Path, PathBuf},
    rc::Rc,
};

#[derive(Clone)]
struct PluginSettings {
    plugins: PathBuf,
    preferences_path: PathBuf,
    inventory: Rc<RefCell<Vec<InstalledPlugin>>>,
    enabled_path: PathBuf,
    enabled: Rc<RefCell<BTreeSet<String>>>,
    runtime_available: bool,
}

#[derive(Clone)]
struct Playback {
    window: slint::Weak<PlayerWindow>,
    commands: CommandSender,
}

impl Playback {
    fn ytdl_changed(&self, enabled: bool) {
        if let Some(app) = self.window.upgrade() {
            app.global::<riina_ui::PlayerPreferences>()
                .set_network_ytdl_plugin_active(enabled);
            if let Err(error) = crate::network_ytdl_preferences::apply_plugin_override(
                &self.commands,
                &app,
                enabled,
            ) {
                app.set_status(error.into());
            }
        }
    }
}

pub(crate) fn connect(
    window: &PreferencesWindow,
    preferences_path: &Path,
    host: Option<GlobalHandle>,
    playback: Option<(&PlayerWindow, CommandSender)>,
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
    let playback = playback.map(|(app, commands)| Playback {
        window: app.as_weak(),
        commands,
    });
    installation::connect(window, &settings, host.as_ref(), playback.clone());
    let requests = update_actions::Requests::new();
    update_actions::register_refresh(window, &settings, &requests);
    row_actions::register_reorder(window, &settings);
    update_actions::register_staged(window, &settings, &requests, host.as_ref());
    update_actions::register_completion(window, &settings, &requests);
    row_actions::register_enabled(window, &settings, host.as_ref(), playback.clone());
    removal::register(window, &settings, host.as_ref(), playback);
    update_actions::register_information_update(window, &settings, &requests);
    row_actions::register_open_folder(window, &settings);
    let pages = row_actions::register_settings_page(window, &settings);
    about::register(window, &settings, &requests, pages);
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
