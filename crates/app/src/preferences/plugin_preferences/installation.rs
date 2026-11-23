//! Register local package imports and the GitHub download lifecycle.
use super::{
    PluginSettings,
    inventory::refresh,
    permissions::{confirm_permissions, save_granted_permissions},
};
use crate::{PlayerWindow, PreferencesWindow};
use rfd::FileDialog;
use slint::ComponentHandle;
use std::{
    collections::BTreeMap,
    fs,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

pub(super) fn local(window: &PreferencesWindow, app: &PlayerWindow, settings: &PluginSettings) {
    let runtime_available = settings.runtime_available;
    let weak = window.as_weak();
    let parent = app.as_weak();
    let plugins = settings.plugins.clone();
    let entries = settings.inventory.clone();
    let enabled_plugins = settings.enabled.clone();
    let preferences_path = settings.preferences_path.clone();
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
                    if let Err(cleanup_error) = fs::remove_dir_all(&path) {
                        crate::diagnostics::warn!(
                            "Could not remove a plugin without saved grants: {cleanup_error}"
                        );
                    }
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
                    crate::diagnostics::report(
                        &app,
                        crate::diagnostics::UserMessage::info(format!(
                            "Installed plugin {}",
                            manifest.name
                        )),
                    );
                }
            }
            Err(error) => window.set_plugin_update_status(error.into()),
        }
    });
}

pub(super) fn github(window: &PreferencesWindow, app: &PlayerWindow, settings: &PluginSettings) {
    let runtime_available = settings.runtime_available;
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
    let plugins = settings.plugins.clone();
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
        let started = crate::background::spawn_named("plugin-github-install", move || {
            let result = crate::plugin_github::stage_repository(&source, &plugins);
            riina_core::sync::lock(&results).insert(id, result);
            // The result has no recipient once its window or event loop closes.
            let _ = crate::background::to_ui(weak_result, move |window| {
                window.invoke_plugin_github_install_completed(id as i32);
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
    let plugins = settings.plugins.clone();
    let entries = settings.inventory.clone();
    let enabled_plugins = settings.enabled.clone();
    let preferences_path_for_github = settings.preferences_path.clone();
    let github_parent = app.as_weak();
    let results = github_results;
    window.on_plugin_github_install_completed(move |id| {
        let Some(window) = weak.upgrade() else { return };
        let Ok(id) = u64::try_from(id) else { return };
        let staged = riina_core::sync::lock(&results).remove(&id);
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
                    if let Err(cleanup_error) = fs::remove_dir_all(&path) {
                        crate::diagnostics::warn!(
                            "Could not remove a plugin without saved grants: {cleanup_error}"
                        );
                    }
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
                    crate::diagnostics::report(
                        &app,
                        crate::diagnostics::UserMessage::info(format!(
                            "Installed plugin {}",
                            manifest.name
                        )),
                    );
                }
            }
            Err(error) => window.set_plugin_update_status(error.into()),
        }
    });
}
