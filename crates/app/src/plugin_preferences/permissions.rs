//! Permission review and per-plugin grant persistence.
use rfd::{MessageButtons, MessageDialog, MessageDialogResult};
use riina_core::plugin::{Manifest, Permission};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

pub(super) fn confirm_permissions(manifest: &Manifest) -> bool {
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

pub(super) fn permission_name(permission: Permission) -> &'static str {
    match permission {
        Permission::NetworkRequest => "network-request",
        Permission::ShowOsd => "show-osd",
        Permission::ShowAlert => "show-alert",
        Permission::VideoOverlay => "video-overlay",
        Permission::FileSystem => "file-system",
    }
}

pub(super) fn permission_description(permission: Permission) -> &'static str {
    match permission {
        Permission::NetworkRequest => "Network requests to allowed domains (sensitive)",
        Permission::ShowOsd => "Show on-screen messages",
        Permission::ShowAlert => "Show alerts",
        Permission::VideoOverlay => "Display content over video",
        Permission::FileSystem => "Read and modify files on this device (sensitive)",
    }
}

pub(super) fn save_granted_permissions(
    preferences_path: &Path,
    manifest: &Manifest,
) -> Result<(), String> {
    write_plugin_grants(
        preferences_path,
        &manifest.identifier,
        Some(
            manifest
                .known_permissions()
                .map(permission_name)
                .map(str::to_owned)
                .collect(),
        ),
    )
}

pub(super) fn read_plugin_grants(
    preferences_path: &Path,
    identifier: &str,
) -> Result<Option<Vec<String>>, String> {
    let path = preferences_path.with_file_name("plugin-permissions.json");
    Ok(load_grants(&path)?.get(identifier).cloned())
}

pub(super) fn save_updated_permissions(
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

pub(super) fn restore_plugin_grants(
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

pub(super) fn write_plugin_grants(
    preferences_path: &Path,
    identifier: &str,
    updated: Option<Vec<String>>,
) -> Result<(), String> {
    let path = preferences_path.with_file_name("plugin-permissions.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    // Atomic replacement protects readers from partial JSON, but does not merge
    // concurrent writers. A stable sidecar coordinates all player processes.
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path.with_extension("json.lock"))
        .map_err(|error| error.to_string())?;
    lock.lock().map_err(|error| error.to_string())?;
    let mut grants = load_grants(&path)?;
    if let Some(updated) = updated {
        grants.insert(identifier.to_owned(), updated);
    } else {
        grants.remove(identifier);
    }
    let text = serde_json::to_string_pretty(&grants).map_err(|error| error.to_string())?;
    crate::platform::save_preferences(&path, &text).map_err(|error| error.to_string())
}

fn load_grants(path: &Path) -> Result<BTreeMap<String, Vec<String>>, String> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map_err(|error| error.to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(test)]
mod tests;
