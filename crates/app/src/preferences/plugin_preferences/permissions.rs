//! Permission review and per-plugin grant persistence.
use crate::PreferencesWindow;
use rfd::{MessageButtons, MessageDialog, MessageDialogResult};
use riina_core::plugin::{Manifest, Permission};
use riina_ui::PluginReviewText;
use slint::{ComponentHandle, SharedString};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

pub(super) fn confirm_permissions(window: &PreferencesWindow, manifest: &Manifest) -> bool {
    if manifest.permissions.is_empty() {
        return true;
    }
    let text = window.global::<PluginReviewText>();
    let permissions = describe_permissions(window, manifest);
    MessageDialog::new()
        .set_title(
            text.invoke_enable_title(manifest.name.as_str().into())
                .as_str(),
        )
        .set_description(text.invoke_enable_details(permissions.into()).as_str())
        .set_buttons(MessageButtons::YesNo)
        .show()
        == MessageDialogResult::Yes
}

pub(super) fn describe_permissions(window: &PreferencesWindow, manifest: &Manifest) -> String {
    manifest
        .permissions
        .iter()
        .map(|permission| format!("• {}", permission_caption(window, permission)))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn permission_name(permission: Permission) -> &'static str {
    match permission {
        Permission::NetworkRequest => "network-request",
        Permission::NetworkListener => "network-listener",
        Permission::ShowOsd => "show-osd",
        Permission::ShowAlert => "show-alert",
        Permission::VideoOverlay => "video-overlay",
        Permission::FileSystem => "file-system",
        Permission::ProcessExecution => "process-execution",
        Permission::OpenExternal => "open-external",
    }
}

pub(super) fn permission_caption(window: &PreferencesWindow, raw: &str) -> SharedString {
    let text = window.global::<PluginReviewText>();
    match Permission::parse(raw) {
        Some(Permission::NetworkRequest) => text.get_network_request(),
        Some(Permission::NetworkListener) => text.get_network_listener(),
        Some(Permission::ShowOsd) => text.get_show_osd(),
        Some(Permission::ShowAlert) => text.get_show_alert(),
        Some(Permission::VideoOverlay) => text.get_video_overlay(),
        Some(Permission::FileSystem) => text.get_file_system(),
        Some(Permission::ProcessExecution) => text.get_process_execution(),
        Some(Permission::OpenExternal) => text.get_open_external(),
        None => text.invoke_unknown_permission(raw.into()),
    }
}

#[cfg(test)]
#[test]
fn review_translates_permissions_and_preserves_plugin_identifiers()
-> Result<(), Box<dyn std::error::Error>> {
    i_slint_backend_testing::init_no_event_loop();
    let window = PreferencesWindow::new()?;
    let text = window.global::<PluginReviewText>();
    for (locale, readable, unavailable) in [
        ("ru", "Показывать предупреждения", "не предоставлено"),
        ("ar", "عرض التنبيهات", "غير ممنوح"),
    ] {
        slint::select_bundled_translation(locale)?;
        assert_eq!(permission_caption(&window, "show-alert").as_str(), readable);
        let unknown = permission_caption(&window, "extension-permission");
        assert!(unknown.contains("extension-permission") && unknown.contains(unavailable));
        let title = text.invoke_install_title("Example plugin".into(), false);
        assert!(title.contains("Example plugin") && title != "Install Example plugin?");
    }
    slint::select_bundled_translation("en")?;
    Ok(())
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
