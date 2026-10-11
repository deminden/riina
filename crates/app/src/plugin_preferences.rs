//! Installed plugin inventory, local package import and permission review.

use crate::{PlayerWindow, PluginPreferencesRow, PreferencesWindow};
use rfd::{FileDialog, MessageButtons, MessageDialog, MessageDialogResult};
use riina_core::plugin::{Manifest, Permission};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone)]
struct InstalledPlugin {
    manifest: Manifest,
    directory: PathBuf,
}

pub(crate) fn connect(
    window: &PreferencesWindow,
    app: &PlayerWindow,
    preferences_path: &Path,
    host: Option<crate::plugin_host::PluginHostHandle>,
) {
    let plugins_directory = preferences_path.with_file_name("Plugins");
    let inventory = std::rc::Rc::new(std::cell::RefCell::new(Vec::<InstalledPlugin>::new()));
    let enabled_path = preferences_path.with_file_name("plugin-enabled.json");
    let enabled = std::rc::Rc::new(std::cell::RefCell::new(load_enabled(&enabled_path)));
    let runtime_available = host.is_some();
    refresh(
        window,
        &plugins_directory,
        &inventory,
        &enabled.borrow(),
        runtime_available,
    );

    let weak = window.as_weak();
    let parent = app.as_weak();
    let plugins = plugins_directory.clone();
    let entries = inventory.clone();
    let enabled_plugins = enabled.clone();
    let preferences_path = preferences_path.to_owned();
    window.on_plugin_request_local_install(move || {
        let Some(window) = weak.upgrade() else { return };
        let Some(archive) = FileDialog::new()
            .add_filter("Riina plugin packages", &["iinaplgz"])
            .pick_file()
        else {
            return;
        };
        let staged = match crate::plugin_packages::stage_archive(&archive, &plugins) {
            Ok(staged) => staged,
            Err(error) => {
                window.set_plugin_update_status(format!("Package rejected: {error}").into());
                return;
            }
        };
        if !confirm_permissions(&staged.manifest) {
            window.set_plugin_update_status("Plugin installation cancelled.".into());
            return;
        }
        let manifest = staged.manifest.clone();
        match staged.install(&plugins) {
            Ok(path) => {
                if let Err(error) = save_granted_permissions(&preferences_path, &manifest) {
                    let _ = fs::remove_dir_all(&path);
                    window.set_plugin_update_status(
                        format!("Could not save plugin permissions: {error}").into(),
                    );
                    return;
                }
                refresh(
                    &window,
                    &plugins,
                    &entries,
                    &enabled_plugins.borrow(),
                    runtime_available,
                );
                window.set_plugin_update_status(
                    format!(
                        "Installed {}. It is disabled until you enable it.",
                        manifest.name
                    )
                    .into(),
                );
                if let Some(app) = parent.upgrade() {
                    app.set_status(format!("Installed plugin {}", manifest.name).into());
                }
            }
            Err(error) => window.set_plugin_update_status(error.into()),
        }
    });

    let weak = window.as_weak();
    window.on_plugin_request_github_install(move || {
        if let Some(window) = weak.upgrade() {
            window.set_plugin_update_status(
                "GitHub installation and update checks are not available yet.".into(),
            );
        }
    });

    let weak = window.as_weak();
    let plugins = plugins_directory.clone();
    let entries = inventory.clone();
    let enabled_plugins = enabled.clone();
    let runtime_available_for_refresh = runtime_available;
    window.on_plugin_refresh(move || {
        if let Some(window) = weak.upgrade() {
            refresh(
                &window,
                &plugins,
                &entries,
                &enabled_plugins.borrow(),
                runtime_available_for_refresh,
            );
            window.set_plugin_update_status(
                "Plugin list refreshed; update checks are not available yet.".into(),
            );
        }
    });

    let weak = window.as_weak();
    let entries = inventory.clone();
    let enabled_plugins = enabled.clone();
    let plugin_host = host.clone();
    let enabled_file = enabled_path.clone();
    let plugins = plugins_directory.clone();
    window.on_plugin_enabled_changed(move |index, should_enable| {
        let Some(window) = weak.upgrade() else { return };
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let Some(plugin) = entries.borrow().get(index).cloned() else {
            return;
        };
        let mut updated = enabled_plugins.borrow().clone();
        let Some(plugin_host) = plugin_host.as_ref() else {
            window.set_plugin_update_status("Plugin host is unavailable.".into());
            refresh(
                &window,
                &plugins,
                &entries,
                &enabled_plugins.borrow(),
                false,
            );
            return;
        };
        if should_enable {
            updated.insert(plugin.manifest.identifier.clone());
            if let Err(error) = save_enabled(&enabled_file, &updated) {
                window.set_plugin_update_status(
                    format!("Could not save plugin state: {error}").into(),
                );
                return;
            }
            if let Err(error) =
                plugin_host.enable(plugin.manifest.clone(), plugin.directory.clone())
            {
                updated.remove(&plugin.manifest.identifier);
                let _ = save_enabled(&enabled_file, &updated);
                window.set_plugin_update_status(format!("Could not enable plugin: {error}").into());
                *enabled_plugins.borrow_mut() = updated;
                refresh(
                    &window,
                    &plugins,
                    &entries,
                    &enabled_plugins.borrow(),
                    runtime_available,
                );
                return;
            }
            *enabled_plugins.borrow_mut() = updated;
            window.set_plugin_update_status(
                format!(
                    "{} is enabled; its runtime is starting.",
                    plugin.manifest.name
                )
                .into(),
            );
        } else {
            updated.remove(&plugin.manifest.identifier);
            if let Err(error) = save_enabled(&enabled_file, &updated) {
                window.set_plugin_update_status(
                    format!("Could not save plugin state: {error}").into(),
                );
                return;
            }
            if let Err(error) = plugin_host.disable(plugin.manifest.identifier.clone()) {
                let previous = enabled_plugins.borrow().clone();
                let _ = save_enabled(&enabled_file, &previous);
                window.set_plugin_update_status(format!("Could not stop plugin: {error}").into());
                refresh(&window, &plugins, &entries, &previous, runtime_available);
                return;
            }
            *enabled_plugins.borrow_mut() = updated;
            window
                .set_plugin_update_status(format!("{} is disabled.", plugin.manifest.name).into());
        }
        refresh(
            &window,
            &plugins,
            &entries,
            &enabled_plugins.borrow(),
            runtime_available,
        );
    });

    let weak = window.as_weak();
    let parent = app.as_weak();
    let entries = inventory.clone();
    window.on_plugin_row_about(move |index| {
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let Some(plugin) = entries.borrow().get(index).cloned() else {
            return;
        };
        let message = format!(
            "{} {}\nBy {}\n{}\n{}",
            plugin.manifest.name,
            plugin.manifest.version,
            plugin.manifest.author.name,
            plugin.manifest.identifier,
            plugin
                .manifest
                .description
                .as_deref()
                .unwrap_or("No Description")
        );
        if let Some(window) = weak.upgrade() {
            window.set_plugin_update_status(message.clone().into());
        }
        if let Some(app) = parent.upgrade() {
            app.set_status(message.into());
        }
    });

    let weak = window.as_weak();
    let entries = inventory.clone();
    window.on_plugin_row_actions(move |index| {
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let Some(plugin) = entries.borrow().get(index).cloned() else {
            return;
        };
        match crate::platform::open_directory(&plugin.directory) {
            Ok(()) => {
                if let Some(window) = weak.upgrade() {
                    window.set_plugin_update_status(
                        format!("Opened the package folder for {}.", plugin.manifest.name).into(),
                    );
                }
            }
            Err(error) => {
                if let Some(window) = weak.upgrade() {
                    window.set_plugin_update_status(error.to_string().into());
                }
            }
        }
    });
}

fn refresh(
    window: &PreferencesWindow,
    directory: &Path,
    inventory: &std::rc::Rc<std::cell::RefCell<Vec<InstalledPlugin>>>,
    enabled: &BTreeSet<String>,
    runtime_available: bool,
) {
    let mut loaded = Vec::new();
    let mut invalid = 0;
    if let Ok(entries) = fs::read_dir(directory) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "iinaplugin")
            {
                match Manifest::load_from_directory(&path) {
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
    let unavailable = loaded
        .iter()
        .filter(|plugin| !runtime_available || !current_runtime_supports(&plugin.manifest))
        .count();
    *inventory.borrow_mut() = loaded;
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

fn plugin_rows(
    plugins: &[InstalledPlugin],
    enabled: &BTreeSet<String>,
    runtime_available: bool,
) -> ModelRc<PluginPreferencesRow> {
    ModelRc::new(VecModel::from(
        plugins
            .iter()
            .map(|plugin| PluginPreferencesRow {
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

fn confirm_permissions(manifest: &Manifest) -> bool {
    if manifest.permissions.is_empty() {
        return true;
    }
    let permissions = manifest
        .permissions
        .iter()
        .map(|permission| {
            let description = match Permission::parse(permission.as_str()) {
                Some(Permission::NetworkRequest) => "Network requests (sensitive)".to_owned(),
                Some(Permission::ShowOsd) => "Show on-screen messages".to_owned(),
                Some(Permission::ShowAlert) => "Show alerts".to_owned(),
                Some(Permission::VideoOverlay) => "Display content over video".to_owned(),
                Some(Permission::FileSystem) => "Access plugin files (sensitive)".to_owned(),
                None => format!("Unknown permission: {permission} (not granted)"),
            };
            format!("• {description}")
        })
        .collect::<Vec<_>>()
        .join("\n");
    MessageDialog::new()
        .set_title(format!("Permissions requested by {}", manifest.name))
        .set_description(format!(
            "Review these permissions before installing:\n\n{permissions}\n\nNetwork requests, file access and overlay rendering are not available yet."
        ))
        .set_buttons(MessageButtons::YesNo)
        .show()
        == MessageDialogResult::Yes
}

fn save_granted_permissions(preferences_path: &Path, manifest: &Manifest) -> Result<(), String> {
    let path = preferences_path.with_file_name("plugin-permissions.json");
    let mut grants: BTreeMap<String, Vec<String>> = match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map_err(|error| error.to_string())?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
        Err(error) => return Err(error.to_string()),
    };
    grants.insert(
        manifest.identifier.clone(),
        manifest
            .known_permissions()
            .map(|permission| match permission {
                Permission::NetworkRequest => "network-request",
                Permission::ShowOsd => "show-osd",
                Permission::ShowAlert => "show-alert",
                Permission::VideoOverlay => "video-overlay",
                Permission::FileSystem => "file-system",
            })
            .map(str::to_owned)
            .collect(),
    );
    let text = serde_json::to_string_pretty(&grants).map_err(|error| error.to_string())?;
    crate::platform::save_preferences(&path, &text).map_err(|error| error.to_string())
}

fn load_enabled(path: &Path) -> BTreeSet<String> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<BTreeSet<String>>(&text).ok())
        .unwrap_or_default()
}

fn save_enabled(path: &Path, enabled: &BTreeSet<String>) -> Result<(), String> {
    let text = serde_json::to_string_pretty(enabled).map_err(|error| error.to_string())?;
    crate::platform::save_preferences(path, &text).map_err(|error| error.to_string())
}

fn current_runtime_supports(manifest: &Manifest) -> bool {
    manifest
        .permissions
        .iter()
        .all(|permission| matches!(permission.as_str(), "show-osd" | "show-alert"))
        && manifest.unknown_permissions().next().is_none()
}
