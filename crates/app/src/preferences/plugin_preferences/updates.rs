//! Downloaded plugin replacement retains rollback and permission recovery.
use super::{InstalledPlugin, permissions::*};
use rfd::{MessageButtons, MessageDialog, MessageDialogResult};
use riina_core::plugin::{Manifest, Permission};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

pub(super) fn check_update_status(
    plugins: &[InstalledPlugin],
) -> (String, BTreeMap<String, String>) {
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

pub(super) fn confirm_update_permissions(updated: &Manifest, previous: &Manifest) -> bool {
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

pub(super) fn apply_plugin_update(
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

pub(super) fn append_recovery(message: String, recovery: Vec<String>) -> String {
    if recovery.is_empty() {
        message
    } else {
        format!("{message}; recovery problems: {}", recovery.join("; "))
    }
}
