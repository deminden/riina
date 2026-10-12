//! Installed plugin inventory, local package import and permission review.

use crate::{PlayerWindow, PluginPreferencesRow, PreferencesWindow};
use rfd::{FileDialog, MessageButtons, MessageDialog, MessageDialogResult};
use riina_core::plugin::{Manifest, Permission};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
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

fn check_update_status(plugins: &[InstalledPlugin]) -> (String, BTreeMap<String, String>) {
    let mut updates = Vec::new();
    let mut available = BTreeMap::new();
    let mut failures = 0;
    let mut checked = 0;
    for plugin in plugins {
        let Some(repository) = plugin.manifest.github_repo.as_deref() else {
            continue;
        };
        let Some(version) = plugin.manifest.github_version else {
            continue;
        };
        checked += 1;
        match crate::plugin_github::check_new_version(repository, version) {
            Ok(Some(current)) => {
                updates.push(format!("{} {current}", plugin.manifest.name));
                available.insert(plugin.manifest.identifier.clone(), current);
            }
            Ok(None) => {}
            Err(_) => failures += 1,
        }
    }
    if checked == 0 {
        return ("No GitHub-managed plugins to check.".to_owned(), available);
    }
    let status = match (updates.len(), failures) {
        (0, 0) => "All GitHub plugins are up to date.".to_owned(),
        (0, count) => format!("Could not check updates for {count} GitHub plugin(s)."),
        (1, 0) => format!("Update available: {}.", updates[0]),
        (count, 0) => format!(
            "Updates available for {count} plugins: {}.",
            updates.join(", ")
        ),
        (count, failures) => format!(
            "Updates available for {count} plugins: {}; {failures} update check(s) failed.",
            updates.join(", ")
        ),
    };
    (status, available)
}

fn confirm_update_permissions(updated: &Manifest, previous: &Manifest) -> bool {
    let previous_permissions = previous
        .known_permissions()
        .map(|permission| permission_name(permission).to_owned())
        .collect::<BTreeSet<_>>();
    let requested = updated
        .permissions
        .iter()
        .filter(|permission| !previous_permissions.contains(*permission))
        .map(|permission| match Permission::parse(permission) {
            Some(permission) => format!("• {}", permission_description(permission)),
            None => format!("• Unknown permission: {permission} (not granted)"),
        })
        .collect::<Vec<_>>();
    if requested.is_empty() {
        return true;
    }
    MessageDialog::new()
        .set_title(format!("Additional permissions requested by {}", updated.name))
        .set_description(format!(
            "Review these new permissions before updating:\n\n{}\n\nPreviously granted permissions remain unchanged.",
            requested.join("\n")
        ))
        .set_buttons(MessageButtons::YesNo)
        .show()
        == MessageDialogResult::Yes
}

fn apply_plugin_update(
    staged: crate::plugin_packages::StagedPackage,
    previous: InstalledPlugin,
    plugins_directory: &Path,
    preferences_path: &Path,
    host: Option<&crate::plugin_host::PluginHostHandle>,
    should_activate: bool,
) -> Result<(), String> {
    let identifier = previous.manifest.identifier.clone();
    if staged.manifest.identifier != identifier {
        return Err(
            "The downloaded package identifier does not match the installed plugin.".into(),
        );
    }
    let old_grants = read_plugin_grants(preferences_path, &identifier)?;
    if should_activate {
        let Some(host) = host else {
            return Err("The plugin host is unavailable for an active plugin update.".into());
        };
        if let Err(error) = host.suspend_for_update(identifier.clone()) {
            let _ =
                host.activate_after_update(previous.manifest.clone(), previous.directory.clone());
            return Err(error);
        }
    }
    if let Err(error) = save_updated_permissions(
        preferences_path,
        &staged.manifest,
        &previous.manifest,
        old_grants.as_deref().unwrap_or_default(),
    ) {
        let mut recovery = Vec::new();
        if let Err(restore_error) =
            restore_plugin_grants(preferences_path, &identifier, old_grants.as_deref())
        {
            recovery.push(format!("permission restore failed: {restore_error}"));
        }
        if recovery.is_empty()
            && should_activate
            && let Some(host) = host
            && let Err(restart_error) =
                host.activate_after_update(previous.manifest.clone(), previous.directory.clone())
        {
            recovery.push(format!("previous plugin restart failed: {restart_error}"));
        }
        return Err(append_recovery(
            format!("Could not save updated plugin permissions: {error}"),
            recovery,
        ));
    }
    let updated_manifest = staged.manifest.clone();
    let replacement = match staged.replace(plugins_directory) {
        Ok(replacement) => replacement,
        Err(error) => {
            let mut recovery = Vec::new();
            if let Err(restore_error) =
                restore_plugin_grants(preferences_path, &identifier, old_grants.as_deref())
            {
                recovery.push(format!("permission restore failed: {restore_error}"));
            }
            if recovery.is_empty()
                && should_activate
                && let Some(host) = host
                && let Err(restart_error) = host
                    .activate_after_update(previous.manifest.clone(), previous.directory.clone())
            {
                recovery.push(format!("previous plugin restart failed: {restart_error}"));
            }
            return Err(append_recovery(error, recovery));
        }
    };
    let updated_directory = plugins_directory.join(format!("{identifier}.iinaplugin"));
    if should_activate
        && let Some(host) = host
        && let Err(error) = host.activate_after_update(updated_manifest.clone(), updated_directory)
    {
        let rollback = replacement.rollback();
        let mut recovery = Vec::new();
        if let Err(rollback_error) = rollback {
            recovery.push(format!("package rollback failed: {rollback_error}"));
        }
        if let Err(restore_error) =
            restore_plugin_grants(preferences_path, &identifier, old_grants.as_deref())
        {
            recovery.push(format!("permission restore failed: {restore_error}"));
        }
        if recovery.is_empty()
            && let Err(restart_error) =
                host.activate_after_update(previous.manifest, previous.directory)
        {
            recovery.push(format!("previous plugin restart failed: {restart_error}"));
        }
        return Err(append_recovery(
            format!("Could not start updated plugin: {error}"),
            recovery,
        ));
    }
    let _ = replacement.commit();
    Ok(())
}

fn append_recovery(message: String, recovery: Vec<String>) -> String {
    if recovery.is_empty() {
        message
    } else {
        format!("{message}; recovery problems: {}", recovery.join("; "))
    }
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
                Some(permission) => permission_description(permission).to_owned(),
                None => format!("Unknown permission: {permission} (not granted)"),
            };
            format!("• {description}")
        })
        .collect::<Vec<_>>()
        .join("\n");
    MessageDialog::new()
        .set_title(format!("Permissions requested by {}", manifest.name))
        .set_description(format!(
            "Review these permissions before installing:\n\n{permissions}\n\nNetwork requests are limited to the domains declared by the plugin. The file-system permission can read or change paths outside the plugin's private storage."
        ))
        .set_buttons(MessageButtons::YesNo)
        .show()
        == MessageDialogResult::Yes
}

fn permission_name(permission: Permission) -> &'static str {
    match permission {
        Permission::NetworkRequest => "network-request",
        Permission::ShowOsd => "show-osd",
        Permission::ShowAlert => "show-alert",
        Permission::VideoOverlay => "video-overlay",
        Permission::FileSystem => "file-system",
    }
}

fn permission_description(permission: Permission) -> &'static str {
    match permission {
        Permission::NetworkRequest => "Network requests to allowed domains (sensitive)",
        Permission::ShowOsd => "Show on-screen messages",
        Permission::ShowAlert => "Show alerts",
        Permission::VideoOverlay => "Display content over video",
        Permission::FileSystem => "Read and modify files on this device (sensitive)",
    }
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

fn read_plugin_grants(
    preferences_path: &Path,
    identifier: &str,
) -> Result<Option<Vec<String>>, String> {
    let path = preferences_path.with_file_name("plugin-permissions.json");
    let grants: BTreeMap<String, Vec<String>> = match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map_err(|error| error.to_string())?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    Ok(grants.get(identifier).cloned())
}

fn save_updated_permissions(
    preferences_path: &Path,
    updated: &Manifest,
    previous: &Manifest,
    previously_granted: &[String],
) -> Result<(), String> {
    let previous_declared = previous
        .known_permissions()
        .map(permission_name)
        .collect::<BTreeSet<_>>();
    let updated_declared = updated
        .known_permissions()
        .map(permission_name)
        .collect::<BTreeSet<_>>();
    let previous_grants = previously_granted
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut updated_grants = previous_grants
        .intersection(&updated_declared)
        .map(|permission| (*permission).to_owned())
        .collect::<BTreeSet<_>>();
    updated_grants.extend(
        updated_declared
            .difference(&previous_declared)
            .map(|permission| (*permission).to_owned()),
    );
    write_plugin_grants(
        preferences_path,
        &updated.identifier,
        Some(updated_grants.into_iter().collect()),
    )
}

fn restore_plugin_grants(
    preferences_path: &Path,
    identifier: &str,
    previous: Option<&[String]>,
) -> Result<(), String> {
    write_plugin_grants(
        preferences_path,
        identifier,
        previous.map(<[String]>::to_vec),
    )
}

fn write_plugin_grants(
    preferences_path: &Path,
    identifier: &str,
    updated: Option<Vec<String>>,
) -> Result<(), String> {
    let path = preferences_path.with_file_name("plugin-permissions.json");
    let mut grants: BTreeMap<String, Vec<String>> = match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map_err(|error| error.to_string())?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
        Err(error) => return Err(error.to_string()),
    };
    if let Some(updated) = updated {
        grants.insert(identifier.to_owned(), updated);
    } else {
        grants.remove(identifier);
    }
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

pub(crate) fn current_runtime_supports(manifest: &Manifest) -> bool {
    manifest
        .permissions
        .iter()
        .all(|permission| runtime_supports_permission(permission.as_str()))
        && manifest.unknown_permissions().next().is_none()
}

fn runtime_supports_permission(permission: &str) -> bool {
    matches!(
        permission,
        "show-osd" | "show-alert" | "network-request" | "file-system"
    ) || (cfg!(target_os = "windows") && permission == "video-overlay")
}

#[cfg(test)]
mod tests {
    use super::runtime_supports_permission;

    #[test]
    fn plugin_runtime_advertises_supported_network_and_file_permissions() {
        assert!(runtime_supports_permission("network-request"));
        assert!(runtime_supports_permission("file-system"));
        assert!(!runtime_supports_permission("unknown-permission"));
    }
}
