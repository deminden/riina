//! Keep plugin version checks and staged updates tied to the Settings window.
use super::{
    InstalledPlugin, PluginSettings, current_runtime_supports,
    inventory::refresh,
    updates::{apply_plugin_update, check_update_status, confirm_update_permissions},
};
use crate::{PreferencesWindow, plugin_host::GlobalHandle, plugin_packages::StagedPackage};
use rfd::{MessageButtons, MessageDialog, MessageDialogResult};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

type StagedUpdate = Result<(StagedPackage, InstalledPlugin), String>;
type CompletedUpdate = Result<(), String>;

pub(super) struct Requests {
    pub(super) available: Arc<Mutex<BTreeMap<String, String>>>,
    staged: Arc<Mutex<BTreeMap<u64, StagedUpdate>>>,
    completed: Arc<Mutex<BTreeMap<u64, CompletedUpdate>>>,
    next_id: Arc<AtomicU64>,
    generation: Arc<AtomicU64>,
}

impl Requests {
    pub(super) fn new() -> Self {
        Self {
            available: Arc::default(),
            staged: Arc::default(),
            completed: Arc::default(),
            next_id: Arc::new(AtomicU64::new(1)),
            generation: Arc::new(AtomicU64::new(0)),
        }
    }
}

pub(super) fn register_refresh(
    window: &PreferencesWindow,
    settings: &PluginSettings,
    requests: &Requests,
) {
    let weak = window.as_weak();
    let plugins = settings.plugins.clone();
    let entries = settings.inventory.clone();
    let enabled_plugins = settings.enabled.clone();
    let runtime_available_for_refresh = settings.runtime_available;
    let generations = requests.generation.clone();
    let available_updates_for_refresh = requests.available.clone();
    window.on_plugin_refresh(move || {
        if let Some(window) = weak.upgrade() {
            refresh(
                &window,
                &plugins,
                &entries,
                &enabled_plugins.borrow(),
                runtime_available_for_refresh,
            );
            let plugins_to_check = entries.borrow().clone();
            let generation = generations.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
            riina_core::sync::lock(&available_updates_for_refresh).clear();
            window.set_plugin_update_status("Checking for plugin updates…".into());
            let weak_result = weak.clone();
            let generations_result = generations.clone();
            let available_updates_result = available_updates_for_refresh.clone();
            let started = crate::background::spawn_named("plugin-update-check", move || {
                let (status, versions) = check_update_status(&plugins_to_check);
                if generations_result.load(Ordering::Relaxed) != generation {
                    return;
                }
                *riina_core::sync::lock(&available_updates_result) = versions;
                // The result has no recipient once its window or event loop closes.
                let _ = crate::background::to_ui(weak_result, move |window| {
                    if generations_result.load(Ordering::Relaxed) == generation {
                        window.set_plugin_update_status(status.into());
                    }
                });
            });
            if let Err(error) = started {
                window.set_plugin_update_status(
                    format!("Could not start update check: {error}").into(),
                );
            }
        }
    });
}

pub(super) fn register_staged(
    window: &PreferencesWindow,
    settings: &PluginSettings,
    requests: &Requests,
    host: Option<&GlobalHandle>,
) {
    let weak = window.as_weak();
    let staged_updates_for_stage = requests.staged.clone();
    let update_results_for_stage = requests.completed.clone();
    let enabled_for_stage = settings.enabled.clone();
    let plugins_for_stage = settings.plugins.clone();
    let preferences_for_stage = settings.preferences_path.clone();
    let plugin_host_for_stage = host.cloned();
    window.on_plugin_update_staged(move |id| {
        let Some(window) = weak.upgrade() else { return };
        let Ok(id) = u64::try_from(id) else { return };
        window.set_plugin_github_installing(false);
        let staged = riina_core::sync::lock(&staged_updates_for_stage).remove(&id);
        let Some(staged) = staged else {
            window.set_plugin_update_status("Plugin update completed without a staged package.".into());
            return;
        };
        let (staged, previous) = match staged {
            Ok(staged) => staged,
            Err(error) => {
                window.set_plugin_update_status(format!("Could not download plugin update: {error}").into());
                return;
            }
        };
        if staged.manifest.identifier != previous.manifest.identifier {
            window.set_plugin_update_status(
                "The downloaded package identifier does not match the installed plugin.".into(),
            );
            return;
        }
        if !confirm_update_permissions(&staged.manifest, &previous.manifest) {
            window.set_plugin_update_status("Plugin update cancelled.".into());
            return;
        }
        let should_activate = enabled_for_stage
            .borrow()
            .contains(&previous.manifest.identifier)
            && plugin_host_for_stage.is_some();
        if should_activate && !current_runtime_supports(&staged.manifest) {
            window.set_plugin_update_status(
                "The update requests capabilities unavailable in this build; the installed version remains active.".into(),
            );
            return;
        }
        let plugins = plugins_for_stage.clone();
        let preferences = preferences_for_stage.clone();
        let host = plugin_host_for_stage.clone();
        let results = update_results_for_stage.clone();
        let weak_result = weak.clone();
        let id_for_result = id;
        let plugin_name = previous.manifest.name.clone();
        window.set_plugin_github_installing(true);
        let started = crate::background::spawn_named("plugin-update-apply", move || {
                let result = apply_plugin_update(
                    staged,
                    previous.clone(),
                    &plugins,
                    &preferences,
                    host.as_ref(),
                    should_activate,
                );
                riina_core::sync::lock(&results).insert(id_for_result, result);
                // The result has no recipient once its window or event loop closes.
                let _ = crate::background::to_ui(weak_result, move |window| {
                    window.invoke_plugin_update_completed(id_for_result as i32);
                });
            });
        if let Err(error) = started {
            window.set_plugin_github_installing(false);
            window.set_plugin_update_status(format!("Could not start plugin update: {error}").into());
        } else {
            window.set_plugin_update_status(format!("Updating {plugin_name}…").into());
        }
    });
}

pub(super) fn register_completion(
    window: &PreferencesWindow,
    settings: &PluginSettings,
    requests: &Requests,
) {
    let runtime_available = settings.runtime_available;
    let weak = window.as_weak();
    let update_results = requests.completed.clone();
    let available_updates_for_completion = requests.available.clone();
    let entries = settings.inventory.clone();
    let enabled_plugins = settings.enabled.clone();
    let plugins = settings.plugins.clone();
    window.on_plugin_update_completed(move |id| {
        let Some(window) = weak.upgrade() else { return };
        window.set_plugin_github_installing(false);
        let Ok(id) = u64::try_from(id) else { return };
        let result = riina_core::sync::lock(&update_results).remove(&id);
        match result {
            Some(Ok(())) => {
                riina_core::sync::lock(&available_updates_for_completion).clear();
                refresh(
                    &window,
                    &plugins,
                    &entries,
                    &enabled_plugins.borrow(),
                    runtime_available,
                );
                window.set_plugin_update_status("Plugin updated successfully.".into());
            }
            Some(Err(error)) => window.set_plugin_update_status(error.into()),
            None => {
                window.set_plugin_update_status("Plugin update finished without a result.".into())
            }
        }
    });
}

pub(super) fn register_information_update(
    window: &PreferencesWindow,
    settings: &PluginSettings,
    requests: &Requests,
) {
    let weak = window.as_weak();
    let entries = settings.inventory.clone();
    let updates = requests.available.clone();
    let staged_updates = requests.staged.clone();
    let next_id = requests.next_id.clone();
    let plugins = settings.plugins.clone();
    window.on_plugin_information_update(move || {
        let Some(window) = weak.upgrade() else { return };
        if !window.get_plugin_information_open() || window.get_plugin_information_checking()
            || window.get_plugin_github_installing()
        {
            return;
        }
        let information = window.get_plugin_information();
        let selected = entries.borrow().iter().find(|entry| {
            entry.manifest.identifier == information.identifier.as_str()
        }).cloned();
        let Some(plugin) = selected.filter(InstalledPlugin::can_update) else { return };
        let (Some(repository), Some(installed_version)) = (
            plugin.manifest.github_repo.clone(), plugin.manifest.github_version,
        ) else { return };
        let latest = riina_core::sync::lock(&updates).get(&plugin.manifest.identifier).cloned();
        let Some(latest) = latest else {
            check_selected_update(&window, updates.clone(), plugin.manifest.identifier, repository, installed_version);
            return;
        };
        let response = window.window().with_winit_window(|owner| {
            MessageDialog::new()
                .set_title(format!("Update {}?", plugin.manifest.name))
                .set_description(format!(
                    "Version {latest} is available. The installed version is {}.",
                    plugin.manifest.version,
                ))
                .set_buttons(MessageButtons::YesNo)
                .set_parent(owner)
                .show()
        });
        if response != Some(MessageDialogResult::Yes) {
            return;
        }

        let id = next_id.fetch_add(1, Ordering::Relaxed);
        let staged = staged_updates.clone();
        let weak = window.as_weak();
        let plugins = plugins.clone();
        window.set_plugin_information_open(false);
        window.invoke_close_plugin_settings();
        window.set_plugin_github_installing(true);
        window.set_plugin_update_status(format!("Downloading update for {}…", plugin.manifest.name).into());
        if let Err(error) = crate::background::spawn_named("plugin-update-download", move || {
            let result = crate::plugin_github::stage_repository(&repository, &plugins)
                .and_then(|staged| {
                    if staged.manifest.identifier != plugin.manifest.identifier {
                        return Err("The downloaded package identifier does not match the installed plugin.".into());
                    }
                    Ok((staged, plugin))
                });
            riina_core::sync::lock(&staged).insert(id, result);
            let _ = crate::background::to_ui(weak, move |window| {
                window.invoke_plugin_update_staged(id as i32);
            });
        }) {
            window.set_plugin_github_installing(false);
            window.set_plugin_update_status(format!("Could not start plugin update: {error}").into());
        }
    });
}

fn check_selected_update(
    window: &PreferencesWindow,
    updates: Arc<Mutex<BTreeMap<String, String>>>,
    identifier: String,
    repository: String,
    installed_version: u64,
) {
    window.set_plugin_information_checking(true);
    window.set_plugin_information_status("Checking for updates…".into());
    let owner = window.as_weak();
    if let Err(error) = crate::background::spawn_named("plugin-update-check-run", move || {
        let version = crate::plugin_github::check_new_version(&repository, installed_version);
        let _ = crate::background::to_ui(owner, move |window| {
            window.set_plugin_information_checking(false);
            let mut information = window.get_plugin_information();
            if !window.get_plugin_information_open()
                || information.identifier.as_str() != identifier
            {
                return;
            }
            let message = match version {
                Ok(Some(version)) => {
                    riina_core::sync::lock(&updates).insert(identifier, version.clone());
                    information.update_available = version.as_str().into();
                    window.set_plugin_information(information);
                    format!("Version {version} is available.")
                }
                Ok(None) => "This plugin is up to date.".into(),
                Err(error) => error,
            };
            window.set_plugin_information_status(message.into());
        });
    }) {
        window.set_plugin_information_checking(false);
        window
            .set_plugin_information_status(format!("Could not start update check: {error}").into());
    }
}
