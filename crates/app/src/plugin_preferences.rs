//! Installed plugin inventory, local package import and permission review.

mod inventory;
mod permissions;
mod updates;

use crate::{PlayerWindow, PreferencesWindow};
pub(crate) use inventory::current_runtime_supports;
use inventory::{load_enabled, load_plugin_order, refresh, save_enabled, save_plugin_order};
use permissions::{confirm_permissions, save_granted_permissions};
use rfd::{FileDialog, MessageButtons, MessageDialog, MessageDialogResult};
use riina_core::plugin::Manifest;
use slint::ComponentHandle;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use updates::{apply_plugin_update, check_update_status, confirm_update_permissions};

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
    player: riina_mpv::CommandSender,
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
    window.on_plugin_id_transfer(slint::DataTransfer::from);
    window.on_plugin_id_from_transfer(|transfer| {
        transfer.plain_text().unwrap_or_default().to_string().into()
    });

    let weak = window.as_weak();
    let parent = app.as_weak();
    let github_parent = app.as_weak();
    let github_preferences_path = preferences_path.to_owned();
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
            window.set_plugin_github_repository("".into());
            window.set_plugin_github_sheet_open(true);
        }
    });

    let github_results = Arc::new(Mutex::new(BTreeMap::new()));
    let github_next_id = Arc::new(AtomicU64::new(1));
    let weak = window.as_weak();
    let plugins = plugins_directory.clone();
    let results = github_results.clone();
    let next_id = github_next_id.clone();
    window.on_plugin_github_install_requested(move |source| {
        let Some(window) = weak.upgrade() else { return };
        if window.get_plugin_github_installing() {
            return;
        }
        let id = next_id.fetch_add(1, Ordering::Relaxed);
        let source = source.to_string();
        let plugins = plugins.clone();
        let results = results.clone();
        let weak_result = weak.clone();
        window.set_plugin_github_installing(true);
        window.set_plugin_update_status(format!("Downloading {source}…").into());
        let started = std::thread::Builder::new()
            .name("plugin-github-install".into())
            .spawn(move || {
                let result = crate::plugin_github::stage_repository(&source, &plugins);
                if let Ok(mut pending) = results.lock() {
                    pending.insert(id, result);
                }
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(window) = weak_result.upgrade() {
                        window.invoke_plugin_github_install_completed(id as i32);
                    }
                });
            });
        if let Err(error) = started {
            window.set_plugin_github_installing(false);
            window.set_plugin_update_status(
                format!("Could not start GitHub download: {error}").into(),
            );
        }
    });

    let weak = window.as_weak();
    let plugins = plugins_directory.clone();
    let entries = inventory.clone();
    let enabled_plugins = enabled.clone();
    let preferences_path_for_github = github_preferences_path.clone();
    let github_parent = github_parent.clone();
    let results = github_results;
    window.on_plugin_github_install_completed(move |id| {
        let Some(window) = weak.upgrade() else { return };
        let Ok(id) = u64::try_from(id) else { return };
        let staged = results
            .lock()
            .ok()
            .and_then(|mut pending| pending.remove(&id));
        window.set_plugin_github_installing(false);
        let Some(staged) = staged else {
            window.set_plugin_update_status(
                "GitHub download completed without a package result.".into(),
            );
            return;
        };
        let staged = match staged {
            Ok(staged) => staged,
            Err(error) => {
                window.set_plugin_update_status(
                    format!("GitHub plugin could not be installed: {error}").into(),
                );
                return;
            }
        };
        if !confirm_permissions(&staged.manifest) {
            window.set_plugin_github_sheet_open(false);
            window.set_plugin_update_status("Plugin installation cancelled.".into());
            return;
        }
        let manifest = staged.manifest.clone();
        match staged.install(&plugins) {
            Ok(path) => {
                if let Err(error) =
                    save_granted_permissions(&preferences_path_for_github, &manifest)
                {
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
                window.set_plugin_github_sheet_open(false);
                window.set_plugin_update_status(
                    format!(
                        "Installed {}. It is disabled until you enable it.",
                        manifest.name
                    )
                    .into(),
                );
                if let Some(app) = github_parent.upgrade() {
                    app.set_status(format!("Installed plugin {}", manifest.name).into());
                }
            }
            Err(error) => window.set_plugin_update_status(error.into()),
        }
    });

    let weak = window.as_weak();
    let plugins = plugins_directory.clone();
    let entries = inventory.clone();
    let enabled_plugins = enabled.clone();
    let runtime_available_for_refresh = runtime_available;
    let available_updates = Arc::new(Mutex::new(BTreeMap::<String, String>::new()));
    let staged_updates = Arc::new(Mutex::new(BTreeMap::<
        u64,
        Result<(crate::plugin_packages::StagedPackage, InstalledPlugin), String>,
    >::new()));
    let update_results = Arc::new(Mutex::new(BTreeMap::<u64, Result<(), String>>::new()));
    let update_next_id = Arc::new(AtomicU64::new(1));
    let update_check_generation = Arc::new(AtomicU64::new(0));
    let generations = update_check_generation.clone();
    let available_updates_for_refresh = available_updates.clone();
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
            if let Ok(mut available) = available_updates_for_refresh.lock() {
                available.clear();
            }
            window.set_plugin_update_status("Checking for plugin updates…".into());
            let weak_result = weak.clone();
            let generations_result = generations.clone();
            let available_updates_result = available_updates_for_refresh.clone();
            let started = std::thread::Builder::new()
                .name("plugin-update-check".into())
                .spawn(move || {
                    let (status, versions) = check_update_status(&plugins_to_check);
                    if generations_result.load(Ordering::Relaxed) != generation {
                        return;
                    }
                    if let Ok(mut available) = available_updates_result.lock() {
                        *available = versions;
                    }
                    let _ = slint::invoke_from_event_loop(move || {
                        if generations_result.load(Ordering::Relaxed) == generation
                            && let Some(window) = weak_result.upgrade()
                        {
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
    let weak = window.as_weak();
    let entries = inventory.clone();
    let enabled_plugins = enabled.clone();
    let order_file = plugins_directory.with_file_name("plugin-order.json");
    let plugins = plugins_directory.clone();
    window.on_plugin_reordered(move |dragged, target| {
        let Some(window) = weak.upgrade() else { return };
        let installed = entries
            .borrow()
            .iter()
            .map(|plugin| plugin.manifest.identifier.clone())
            .collect::<Vec<_>>();
        let mut order =
            riina_core::plugin_order::normalize(&load_plugin_order(&order_file), &installed);
        if !riina_core::plugin_order::move_before(&mut order, dragged.as_str(), target.as_str()) {
            return;
        }
        if let Err(error) = save_plugin_order(&order_file, &order) {
            window.set_plugin_update_status(format!("Could not save plugin order: {error}").into());
            return;
        }
        refresh(
            &window,
            &plugins,
            &entries,
            &enabled_plugins.borrow(),
            runtime_available,
        );
        crate::player_ui::reorder_all_open_plugin_menus(&order);
        window.set_plugin_update_status("Plugin order saved.".into());
    });

    let weak = window.as_weak();
    let staged_updates_for_stage = staged_updates.clone();
    let update_results_for_stage = update_results.clone();
    let enabled_for_stage = enabled.clone();
    let plugins_for_stage = plugins_directory.clone();
    let preferences_for_stage = github_preferences_path.clone();
    let plugin_host_for_stage = host.clone();
    window.on_plugin_update_staged(move |id| {
        let Some(window) = weak.upgrade() else { return };
        let Ok(id) = u64::try_from(id) else { return };
        window.set_plugin_github_installing(false);
        let staged = staged_updates_for_stage
            .lock()
            .ok()
            .and_then(|mut pending| pending.remove(&id));
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
        let started = std::thread::Builder::new()
            .name("plugin-update-apply".into())
            .spawn(move || {
                let result = apply_plugin_update(
                    staged,
                    previous.clone(),
                    &plugins,
                    &preferences,
                    host.as_ref(),
                    should_activate,
                );
                if let Ok(mut pending) = results.lock() {
                    pending.insert(id_for_result, result);
                }
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(window) = weak_result.upgrade() {
                        window.invoke_plugin_update_completed(id_for_result as i32);
                    }
                });
            });
        if let Err(error) = started {
            window.set_plugin_github_installing(false);
            window.set_plugin_update_status(format!("Could not start plugin update: {error}").into());
        } else {
            window.set_plugin_update_status(format!("Updating {plugin_name}…").into());
        }
    });

    let weak = window.as_weak();
    let update_results = update_results.clone();
    let available_updates_for_completion = available_updates.clone();
    let entries = inventory.clone();
    let enabled_plugins = enabled.clone();
    let plugins = plugins_directory.clone();
    window.on_plugin_update_completed(move |id| {
        let Some(window) = weak.upgrade() else { return };
        window.set_plugin_github_installing(false);
        let Ok(id) = u64::try_from(id) else { return };
        let result = update_results
            .lock()
            .ok()
            .and_then(|mut pending| pending.remove(&id));
        match result {
            Some(Ok(())) => {
                if let Ok(mut available) = available_updates_for_completion.lock() {
                    available.clear();
                }
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

    let weak = window.as_weak();
    let entries = inventory.clone();
    let enabled_plugins = enabled.clone();
    let plugin_host = host.clone();
    let enabled_file = enabled_path.clone();
    let plugins = plugins_directory.clone();
    let plugin_state_parent = app.as_weak();
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
        if plugin.manifest.identifier == "io.iina.ytdl"
            && let Some(app) = plugin_state_parent.upgrade()
        {
            app.set_network_ytdl_plugin_active(should_enable);
            if let Err(error) =
                crate::network_ytdl_preferences::apply_plugin_override(&player, &app, should_enable)
            {
                app.set_status(error.into());
            }
        }
    });

    let weak = window.as_weak();
    let parent = app.as_weak();
    let entries = inventory.clone();
    let updates = available_updates.clone();
    let staged_updates_for_about = staged_updates.clone();
    let next_update_id = update_next_id.clone();
    let plugins_for_about = plugins_directory.clone();
    window.on_plugin_row_about(move |index| {
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let Some(plugin) = entries.borrow().get(index).cloned() else {
            return;
        };
        let latest = updates
            .lock()
            .ok()
            .and_then(|versions| versions.get(&plugin.manifest.identifier).cloned());
        if let Some(latest) = latest {
            if let Some(window) = weak.upgrade()
                && window.get_plugin_github_installing()
            {
                return;
            }
            let response = MessageDialog::new()
                .set_title(format!("Update {}?", plugin.manifest.name))
                .set_description(format!(
                    "Version {latest} is available. The installed version is {}. Download and apply the update now?",
                    plugin.manifest.version
                ))
                .set_buttons(MessageButtons::YesNo)
                .show();
            if response != MessageDialogResult::Yes {
                return;
            }
            let Some(repository) = plugin.manifest.github_repo.clone() else {
                if let Some(window) = weak.upgrade() {
                    window.set_plugin_update_status(
                        "This plugin does not identify a GitHub repository for updates.".into(),
                    );
                }
                return;
            };
            let id = next_update_id.fetch_add(1, Ordering::Relaxed);
            let staged = staged_updates_for_about.clone();
            let weak_result = weak.clone();
            let plugins = plugins_for_about.clone();
            let plugin_for_update = plugin.clone();
            if let Some(window) = weak.upgrade() {
                window.set_plugin_github_installing(true);
            }
            let started = std::thread::Builder::new()
                .name("plugin-update-download".into())
                .spawn(move || {
                    let result = crate::plugin_github::stage_repository(&repository, &plugins)
                        .and_then(|staged| {
                            if staged.manifest.identifier != plugin_for_update.manifest.identifier {
                                return Err("The downloaded package identifier does not match the installed plugin.".into());
                            }
                            Ok(staged)
                        })
                        .map(|staged| (staged, plugin_for_update));
                    if let Ok(mut pending) = staged.lock() {
                        pending.insert(id, result);
                    }
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(window) = weak_result.upgrade() {
                            window.invoke_plugin_update_staged(id as i32);
                        }
                    });
                });
            if let Some(window) = weak.upgrade() {
                match started {
                    Ok(_) => window.set_plugin_update_status(
                        format!("Downloading update for {}…", plugin.manifest.name).into(),
                    ),
                    Err(error) => {
                        window.set_plugin_github_installing(false);
                        window.set_plugin_update_status(
                            format!("Could not start plugin update: {error}").into(),
                        );
                    }
                }
            }
            return;
        }
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

#[cfg(test)]
mod tests {
    use super::inventory::runtime_supports_permission;

    #[test]
    fn plugin_runtime_advertises_supported_network_and_file_permissions() {
        assert!(runtime_supports_permission("network-request"));
        assert!(runtime_supports_permission("file-system"));
        assert!(!runtime_supports_permission("unknown-permission"));
    }
}
