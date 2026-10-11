//! Installed plugin inventory, local package import and permission review.

use crate::{PlayerWindow, PluginPreferencesRow, PreferencesWindow};
use rfd::{FileDialog, MessageButtons, MessageDialog, MessageDialogResult};
use riina_core::plugin::{Manifest, Permission};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone)]
struct InstalledPlugin {
    manifest: Manifest,
    directory: PathBuf,
}

pub(crate) fn connect(window: &PreferencesWindow, app: &PlayerWindow, preferences_path: &Path) {
    let plugins_directory = preferences_path.with_file_name("Plugins");
    let inventory = std::rc::Rc::new(std::cell::RefCell::new(Vec::<InstalledPlugin>::new()));
    refresh(window, &plugins_directory, &inventory);

    let weak = window.as_weak();
    let parent = app.as_weak();
    let plugins = plugins_directory.clone();
    let entries = inventory.clone();
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
                refresh(&window, &plugins, &entries);
                window.set_plugin_update_status(
                    format!("Installed {}. Plugin activation and overlay display are not available yet.", manifest.name).into(),
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
    window.on_plugin_refresh(move || {
        if let Some(window) = weak.upgrade() {
            refresh(&window, &plugins, &entries);
            window.set_plugin_update_status(
                "Plugin list refreshed; update checks are not available yet.".into(),
            );
        }
    });

    let weak = window.as_weak();
    let entries = inventory.clone();
    window.on_plugin_enabled_changed(move |index, _enabled| {
        if let Some(window) = weak.upgrade() {
            if let Ok(index) = usize::try_from(index)
                && let Some(plugin) = entries.borrow().get(index)
            {
                window.set_plugin_update_status(
                    format!(
                        "{} is installed, but its JavaScript runtime is not active yet.",
                        plugin.manifest.name
                    )
                    .into(),
                );
            }
            // The row model remains disabled until a runtime can honor this switch.
            window.set_plugin_rows(plugin_rows(&entries.borrow()));
        }
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
    *inventory.borrow_mut() = loaded;
    window.set_plugin_rows(plugin_rows(&inventory.borrow()));
    window.set_plugin_update_status(if invalid == 0 {
        "Plugin updates have not been checked.".into()
    } else {
        format!("{} installed package(s) could not be loaded.", invalid).into()
    });
}

fn plugin_rows(plugins: &[InstalledPlugin]) -> ModelRc<PluginPreferencesRow> {
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
                enabled: false,
                runtime_ready: false,
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
            "Review these permissions before installing:\n\n{permissions}\n\nPlugin activation and overlay display are not available yet."
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
