//! Installed inventory, display order and runtime capability presentation.
use super::InstalledPlugin;
use crate::{PluginPreferencesRow, PreferencesWindow};
use riina_core::plugin::Manifest;
use slint::ComponentHandle;
use slint::{ModelRc, VecModel};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::Path,
};

pub(super) fn refresh(
    window: &PreferencesWindow,
    directory: &Path,
    inventory: &std::rc::Rc<std::cell::RefCell<Vec<InstalledPlugin>>>,
    enabled: &BTreeSet<String>,
    runtime_available: bool,
) {
    let form = window.global::<crate::NetworkSettingsState>();
    let locale = crate::platform::language::current_locale_identifier();
    let catalog = crate::plugin_packages::discover(directory, locale.as_deref());
    let invalid = catalog.invalid;
    let mut loaded = catalog.plugins;
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
    form.set_network_ytdl_plugin_active(inventory.borrow().iter().any(|plugin| {
        plugin.manifest.identifier == "io.iina.ytdl"
            && enabled.contains(&plugin.manifest.identifier)
    }));
    form.set_network_ytdl_ui_enabled(
        form.get_network_ytdl_enabled() && !form.get_network_ytdl_plugin_active(),
    );
    window.set_plugin_rows(plugin_rows(&inventory.borrow(), enabled, runtime_available));
    let status = match (invalid, unavailable) {
        (0, 0) => window
            .global::<riina_ui::PluginReviewText>()
            .get_updates_unchecked()
            .to_string(),
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
                development: plugin.is_development(),
                identifier: plugin.manifest.identifier.clone().into(),
                name: plugin.manifest.name.clone().into(),
                version: plugin.manifest.version.clone().into(),
                description: plugin
                    .manifest
                    .description
                    .as_deref()
                    .unwrap_or_default()
                    .into(),
                enabled: enabled.contains(&plugin.manifest.identifier),
                runtime_ready: runtime_available && current_runtime_supports(&plugin.manifest),
                has_settings: plugin.manifest.preferences_page.is_some(),
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
    read_enabled(path).unwrap_or_default()
}

pub(super) fn read_enabled(path: &Path) -> Result<BTreeSet<String>, String> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map_err(|error| error.to_string()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(BTreeSet::new()),
        Err(error) => Err(error.to_string()),
    }
}

pub(super) fn set_enabled(path: &Path, identifier: &str, value: bool) -> Result<bool, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path.with_extension("json.lock"))
        .map_err(|error| error.to_string())?;
    lock.lock().map_err(|error| error.to_string())?;
    let mut enabled = read_enabled(path)?;
    let was_enabled = enabled.contains(identifier);
    if was_enabled != value {
        if value {
            enabled.insert(identifier.to_owned());
        } else {
            enabled.remove(identifier);
        }
        let text = serde_json::to_string_pretty(&enabled).map_err(|error| error.to_string())?;
        crate::platform::save_preferences(path, &text).map_err(|error| error.to_string())?;
    }
    Ok(was_enabled)
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
        "show-osd"
            | "show-alert"
            | "network-request"
            | "network-listener"
            | "file-system"
            | "process-execution"
            | "open-external"
    ) || (cfg!(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "linux"
    )) && permission == "video-overlay")
}
