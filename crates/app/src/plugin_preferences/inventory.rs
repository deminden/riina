//! Installed inventory, display order and runtime capability presentation.
use super::InstalledPlugin;
use crate::{PluginPreferencesRow, PreferencesWindow};
use riina_core::plugin::Manifest;
use slint::{ModelRc, VecModel};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

pub(super) fn refresh(
    window: &PreferencesWindow,
    directory: &Path,
    inventory: &std::rc::Rc<std::cell::RefCell<Vec<InstalledPlugin>>>,
    enabled: &BTreeSet<String>,
    runtime_available: bool,
) {
    let mut loaded = Vec::new();
    let mut invalid = 0;
    let locale = crate::platform::language::current_locale_identifier();
    if let Ok(entries) = fs::read_dir(directory) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "iinaplugin")
            {
                match Manifest::load_from_directory_with_locale(&path, locale.as_deref()) {
                    Ok(manifest) => loaded.push(InstalledPlugin {
                        manifest,
                        directory: path,
                    }),
                    Err(_) => invalid += 1,
                }
            }
        }
    }
    loaded.sort_by(|left, right| left.manifest.name.cmp(&right.manifest.name));
    let installed = loaded
        .iter()
        .map(|plugin| plugin.manifest.identifier.clone())
        .collect::<Vec<_>>();
    let order_path = directory.with_file_name("plugin-order.json");
    let order = riina_core::plugin_order::normalize(&load_plugin_order(&order_path), &installed);
    let order_positions = order
        .iter()
        .enumerate()
        .map(|(position, identifier)| (identifier.as_str(), position))
        .collect::<BTreeMap<_, _>>();
    loaded.sort_by_key(|plugin| {
        order_positions
            .get(plugin.manifest.identifier.as_str())
            .copied()
            .unwrap_or(usize::MAX)
    });
    let unavailable = loaded
        .iter()
        .filter(|plugin| !runtime_available || !current_runtime_supports(&plugin.manifest))
        .count();
    *inventory.borrow_mut() = loaded;
    window.set_network_ytdl_plugin_active(inventory.borrow().iter().any(|plugin| {
        plugin.manifest.identifier == "io.iina.ytdl"
            && enabled.contains(&plugin.manifest.identifier)
    }));
    window.set_network_ytdl_ui_enabled(
        window.get_network_ytdl_enabled() && !window.get_network_ytdl_plugin_active(),
    );
    window.set_plugin_rows(plugin_rows(&inventory.borrow(), enabled, runtime_available));
    let status = match (invalid, unavailable) {
        (0, 0) => "Plugin updates have not been checked.".to_owned(),
        (0, count) => format!("{count} installed plugin(s) request unavailable capabilities."),
        (count, 0) => format!("{count} installed package(s) could not be loaded."),
        (invalid, unavailable) => format!(
            "{invalid} package(s) could not be loaded; {unavailable} plugin(s) request unavailable capabilities."
        ),
    };
    window.set_plugin_update_status(status.into());
}

pub(super) fn plugin_rows(
    plugins: &[InstalledPlugin],
    enabled: &BTreeSet<String>,
    runtime_available: bool,
) -> ModelRc<PluginPreferencesRow> {
    ModelRc::new(VecModel::from(
        plugins
            .iter()
            .map(|plugin| PluginPreferencesRow {
                identifier: plugin.manifest.identifier.clone().into(),
                name: plugin.manifest.name.clone().into(),
                version: plugin.manifest.version.clone().into(),
                description: plugin
                    .manifest
                    .description
                    .as_deref()
                    .unwrap_or("No Description")
                    .into(),
                enabled: enabled.contains(&plugin.manifest.identifier),
                runtime_ready: runtime_available && current_runtime_supports(&plugin.manifest),
            })
            .collect::<Vec<_>>(),
    ))
}

pub(super) fn load_plugin_order(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Vec<String>>(&text).ok())
        .unwrap_or_default()
}

pub(super) fn save_plugin_order(path: &Path, order: &[String]) -> Result<(), String> {
    let text = serde_json::to_string_pretty(order).map_err(|error| error.to_string())?;
    crate::platform::save_preferences(path, &text).map_err(|error| error.to_string())
}

pub(super) fn load_enabled(path: &Path) -> BTreeSet<String> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<BTreeSet<String>>(&text).ok())
        .unwrap_or_default()
}

pub(super) fn save_enabled(path: &Path, enabled: &BTreeSet<String>) -> Result<(), String> {
    let text = serde_json::to_string_pretty(enabled).map_err(|error| error.to_string())?;
    crate::platform::save_preferences(path, &text).map_err(|error| error.to_string())
}

pub(crate) fn current_runtime_supports(manifest: &Manifest) -> bool {
    manifest
        .permissions
        .iter()
        .all(|permission| runtime_supports_permission(permission.as_str()))
        && manifest.unknown_permissions().next().is_none()
}

pub(super) fn runtime_supports_permission(permission: &str) -> bool {
    matches!(
        permission,
        "show-osd" | "show-alert" | "network-request" | "file-system"
    ) || (cfg!(target_os = "windows") && permission == "video-overlay")
}
