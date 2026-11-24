//! Stop plugin realms before retiring their installed package.
use super::{
    InstalledPlugin, Playback, PluginSettings,
    inventory::{load_enabled, read_enabled, refresh, set_enabled},
    permissions::{read_plugin_grants, restore_plugin_grants},
    updates::append_recovery,
};
use crate::{PreferencesWindow, plugin_host::GlobalHandle, plugin_packages::PackageRemoval};
use rfd::{MessageButtons, MessageDialog, MessageDialogResult};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

struct Completion {
    name: String,
    identifier: String,
    result: Result<Option<String>, String>,
}

pub(super) fn register(
    window: &PreferencesWindow,
    settings: &PluginSettings,
    host: Option<&GlobalHandle>,
    playback: Option<Playback>,
) {
    let completed = Arc::new(Mutex::new(None::<Completion>));
    let weak = window.as_weak();
    let entries = settings.inventory.clone();
    let plugins = settings.plugins.clone();
    let preferences = settings.preferences_path.clone();
    let host = host.cloned();
    let reply = completed.clone();
    window.on_plugin_row_uninstall(move |index| {
        let (Some(window), Ok(index)) = (weak.upgrade(), usize::try_from(index)) else {
            return;
        };
        let Some(plugin) = entries.borrow().get(index).cloned() else {
            return;
        };
        if plugin.is_development() || window.get_plugin_github_installing() {
            return;
        }
        let name = &plugin.manifest.name;
        let confirm = window.window().with_winit_window(|owner| {
            MessageDialog::new()
                .set_title(format!("Uninstall {name}?"))
                .set_description("The plugin's settings and saved data will be kept.")
                .set_parent(owner)
                .set_buttons(MessageButtons::YesNo)
                .show()
        });
        if confirm != Some(MessageDialogResult::Yes) || window.get_plugin_github_installing() {
            return;
        }
        window.invoke_close_plugin_settings();
        window.set_plugin_github_installing(true);
        window.set_plugin_update_status(format!("Uninstalling {name}…").into());
        let owner = weak.clone();
        let plugins = plugins.clone();
        let preferences = preferences.clone();
        let host = host.clone();
        let reply = reply.clone();
        if let Err(error) = crate::background::spawn_named("plugin-uninstall", move || {
            let result = uninstall(&plugins, &preferences, &plugin, host.as_ref());
            *riina_core::sync::lock(&reply) = Some(Completion {
                name: plugin.manifest.name,
                identifier: plugin.manifest.identifier,
                result,
            });
            let _ = crate::background::to_ui(owner, |window| {
                window.invoke_plugin_uninstall_completed();
            });
        }) {
            window.set_plugin_github_installing(false);
            window.set_plugin_update_status(format!("Could not start uninstall: {error}").into());
        }
    });

    let owner = window.as_weak();
    let entries = settings.inventory.clone();
    let enabled = settings.enabled.clone();
    let plugins = settings.plugins.clone();
    let enabled_path = settings.enabled_path.clone();
    let runtime_available = settings.runtime_available;
    window.on_plugin_uninstall_completed(move || {
        let Some(window) = owner.upgrade() else {
            return;
        };
        let Some(completion) = riina_core::sync::lock(&completed).take() else {
            return;
        };
        window.set_plugin_github_installing(false);
        *enabled.borrow_mut() = load_enabled(&enabled_path);
        refresh(
            &window,
            &plugins,
            &entries,
            &enabled.borrow(),
            runtime_available,
        );
        let status = match completion.result {
            Ok(cleanup) => {
                if completion.identifier == "io.iina.ytdl"
                    && let Some(playback) = &playback
                {
                    playback.ytdl_changed(false);
                }
                let name = completion.name;
                cleanup.map_or_else(
                    || format!("Uninstalled {name}."),
                    |error| format!("Uninstalled {name}. {error}"),
                )
            }
            Err(error) => format!("Could not uninstall {}: {error}", completion.name),
        };
        window.set_plugin_update_status(status.into());
    });
}

fn uninstall(
    plugins: &Path,
    preferences: &Path,
    plugin: &InstalledPlugin,
    host: Option<&GlobalHandle>,
) -> Result<Option<String>, String> {
    if plugin.is_development() {
        return Err("Development plugins are managed in their project folders.".into());
    }
    let identifier = &plugin.manifest.identifier;
    let enabled_path = preferences.with_file_name("plugin-enabled.json");
    let was_enabled = read_enabled(&enabled_path)?.contains(identifier);
    let result = (|| {
        if let Some(host) = host {
            host.suspend_for_update(identifier.clone())?;
        }
        let grants = read_plugin_grants(preferences, identifier)?;
        let package = PackageRemoval::stage(plugins, plugin)?;
        let saved = (|| {
            if grants.is_some() {
                restore_plugin_grants(preferences, identifier, None)?;
            }
            set_enabled(&enabled_path, identifier, false)?;
            Ok::<(), String>(())
        })();
        if let Err(error) = saved {
            let mut recovery = Vec::new();
            if let Err(error) = restore_plugin_grants(preferences, identifier, grants.as_deref()) {
                recovery.push(format!("permission restore failed: {error}"));
            }
            if let Err(error) = package.restore() {
                recovery.push(error);
            }
            return Err(append_recovery(error, recovery));
        }
        Ok(package.finish().err())
    })();
    if let Some(host) = host {
        let restart = result.is_err() && was_enabled && plugin.directory.is_dir();
        let retired = if restart {
            host.activate_after_update(plugin.manifest.clone(), plugin.directory.clone())
        } else {
            host.disable(identifier.clone())
        };
        if let Err(error) = retired {
            crate::diagnostics::warn!(
                "Could not finish processing the plugin host change: {error}"
            );
            if let Err(result) = result {
                return Err(append_recovery(result, vec![error]));
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_host::{PluginHost, PluginHostMessage};
    use riina_test_support::TempDir;
    use std::{
        fs,
        time::{Duration, Instant},
    };

    #[test]
    fn uninstall_stops_enabled_package_and_preserves_data_and_development_source() {
        let scratch = TempDir::new("plugin-uninstall").unwrap();
        let profile = scratch.path().join("preferences.json");
        let plugins = scratch.path().join("Plugins");
        let managed = plugins.join("org.example.managed.iinaplugin");
        let development = plugins.join("plugin.iinaplugin-dev");
        let clone = plugins.join("clone.iinaplugin-dev");
        for (path, id, name) in [
            (&managed, "org.example.managed", "Managed"),
            (&development, "org.example.development", "Development"),
            (&clone, "org.example.managed", "Clone"),
        ] {
            fs::create_dir_all(path).unwrap();
            fs::write(
                path.join("Info.json"),
                serde_json::json!({
                    "name": name, "identifier": id, "version": "1",
                    "author": {"name": "Test"}, "entry": "entry.js", "permissions": []
                })
                .to_string(),
            )
            .unwrap();
            fs::write(
                path.join("entry.js"),
                format!("iina.log('{name} startup');"),
            )
            .unwrap();
        }
        let grants = profile.with_file_name("plugin-permissions.json");
        fs::write(
            &grants,
            r#"{"org.example.managed":[],"org.example.development":[]}"#,
        )
        .unwrap();
        let enabled = profile.with_file_name("plugin-enabled.json");
        set_enabled(&enabled, "org.example.managed", true).unwrap();
        set_enabled(&enabled, "org.example.development", true).unwrap();
        let data = plugins.join(".data/org.example.managed").join("notes.txt");
        fs::create_dir_all(data.parent().unwrap()).unwrap();
        fs::write(&data, "retained").unwrap();

        let installed = crate::plugin_packages::discover(&plugins, None).plugins;
        let managed_plugin = installed
            .iter()
            .find(|entry| entry.directory == managed)
            .unwrap();
        let development_plugin = installed
            .iter()
            .find(|entry| entry.directory == development)
            .unwrap();
        let duplicate = installed
            .iter()
            .find(|entry| entry.directory == clone)
            .unwrap();
        assert_ne!(
            duplicate.manifest.identifier,
            managed_plugin.manifest.identifier
        );
        assert!(!development_plugin.can_update());

        let host = PluginHost::start(&profile, &[], &[]).unwrap();
        let handle = host.handle();
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut started = std::collections::BTreeSet::new();
        while started.len() < 2 {
            for message in handle.drain_messages().unwrap() {
                if let PluginHostMessage::Log(text) = message
                    && text.ends_with(" startup")
                {
                    assert!(!text.contains("Clone"), "{text}");
                    started.insert(text);
                }
            }
            assert!(
                Instant::now() < deadline,
                "Plugin startup events: {started:?}"
            );
            std::thread::sleep(Duration::from_millis(5));
        }

        assert!(uninstall(&plugins, &profile, development_plugin, Some(&handle.global)).is_err());
        assert!(
            uninstall(&plugins, &profile, managed_plugin, Some(&handle.global))
                .unwrap()
                .is_none()
        );
        assert!(handle.drain_messages().unwrap().iter().any(|message| {
            matches!(message, PluginHostMessage::RemoveSidebar(id) if id == "org.example.managed")
        }));
        assert!(!managed.exists());
        assert!(development.join("entry.js").is_file());
        assert_eq!(fs::read_to_string(data).unwrap(), "retained");
        assert_eq!(
            load_enabled(&enabled),
            ["org.example.development".into()].into()
        );
        assert_eq!(
            fs::read_to_string(grants).unwrap().trim(),
            "{\n  \"org.example.development\": []\n}"
        );
    }
}
