//! Store the OpenSubtitles key in the operating system's secure credential store.

use std::path::Path;

const SERVICE: &str = "com.riina.player";
const ACCOUNT: &str = "opensubtitles-api-key";
const ASSRT_ACCOUNT: &str = "assrt-token";

pub(crate) fn load() -> Result<Option<String>, String> {
    let entry = keyring::Entry::new(SERVICE, ACCOUNT)
        .map_err(|error| format!("Could not access the system credential store: {error}"))?;
    load_entry(&entry)
}

fn load_entry(entry: &keyring::Entry) -> Result<Option<String>, String> {
    match entry.get_password() {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(format!("Could not read the OpenSubtitles key: {error}")),
    }
}

pub(crate) fn store(secret: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(SERVICE, ACCOUNT)
        .map_err(|error| format!("Could not access the system credential store: {error}"))?;
    store_entry(&entry, secret)
}

pub(crate) fn load_assrt() -> Result<Option<String>, String> {
    let entry = keyring::Entry::new(SERVICE, ASSRT_ACCOUNT)
        .map_err(|error| format!("Could not access the system credential store: {error}"))?;
    match entry.get_password() {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(format!("Could not read the Assrt token: {error}")),
    }
}

pub(crate) fn store_assrt(secret: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(SERVICE, ASSRT_ACCOUNT)
        .map_err(|error| format!("Could not access the system credential store: {error}"))?;
    if secret.trim().is_empty() {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(format!("Could not remove the Assrt token: {error}")),
        }
    } else {
        entry
            .set_password(secret.trim())
            .map_err(|error| format!("Could not store the Assrt token securely: {error}"))
    }
}

fn store_entry(entry: &keyring::Entry, secret: &str) -> Result<(), String> {
    if secret.trim().is_empty() {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(format!("Could not remove the OpenSubtitles key: {error}")),
        }
    } else {
        entry
            .set_password(secret.trim())
            .map_err(|error| format!("Could not store the OpenSubtitles key securely: {error}"))
    }
}

pub(crate) fn restore_preferences(
    preferences: &mut riina_core::Preferences,
    saved_json: Option<&str>,
    preferences_path: &Path,
) -> Result<(), String> {
    restore_preferences_with(
        preferences,
        saved_json,
        preferences_path,
        load,
        store,
        crate::platform::save_preferences,
    )?;
    preferences.online_subtitles.assrt_token = load_assrt()?.unwrap_or_default();
    Ok(())
}

fn restore_preferences_with(
    preferences: &mut riina_core::Preferences,
    saved_json: Option<&str>,
    preferences_path: &Path,
    load_secret: impl FnOnce() -> Result<Option<String>, String>,
    store_secret: impl FnOnce(&str) -> Result<(), String>,
    save_preferences: impl FnOnce(&Path, &str) -> std::io::Result<()>,
) -> Result<(), String> {
    let legacy_secret = saved_json
        .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
        .and_then(|value| {
            value
                .get("online_subtitles")?
                .get("api_key")?
                .as_str()
                .map(str::to_owned)
        });
    let had_legacy_secret = legacy_secret.is_some();
    match load_secret() {
        Ok(Some(secret)) => {
            preferences.online_subtitles.api_key = secret;
            if had_legacy_secret {
                let text = serde_json::to_string_pretty(preferences).map_err(|error| {
                    format!("Could not clean migrated subtitle settings: {error}")
                })?;
                save_preferences(preferences_path, &text).map_err(|error| {
                    format!("Could not remove the old plain-text subtitle key: {error}")
                })?;
            }
        }
        Ok(None) => {
            let Some(secret) = legacy_secret else {
                return Ok(());
            };
            // Preserve the key in memory if migration fails so it remains usable
            // in this session; all later JSON saves still omit it.
            preferences.online_subtitles.api_key = secret.clone();
            store_secret(&secret)?;
            let text = serde_json::to_string_pretty(preferences)
                .map_err(|error| format!("Could not clean migrated subtitle settings: {error}"))?;
            save_preferences(preferences_path, &text).map_err(|error| {
                format!("Could not remove the old plain-text subtitle key: {error}")
            })?;
        }
        Err(error) => {
            if let Some(secret) = legacy_secret {
                preferences.online_subtitles.api_key = secret;
            }
            return Err(error);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::restore_preferences_with;
    use riina_core::Preferences;
    use std::{cell::RefCell, path::Path};

    const LEGACY_SETTINGS: &str =
        r#"{"online_subtitles":{"api_key":"legacy-secret","auto_search":true}}"#;

    #[test]
    fn migration_moves_legacy_key_into_store_then_rewrites_settings() {
        let mut preferences = Preferences::parse(LEGACY_SETTINGS).unwrap();
        let secure_value = RefCell::new(None::<String>);
        let saved = RefCell::new(None::<String>);

        restore_preferences_with(
            &mut preferences,
            Some(LEGACY_SETTINGS),
            Path::new("preferences.json"),
            || Ok(secure_value.borrow().clone()),
            |secret| {
                *secure_value.borrow_mut() = Some(secret.to_owned());
                Ok(())
            },
            |_, text| {
                *saved.borrow_mut() = Some(text.to_owned());
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(preferences.online_subtitles.api_key, "legacy-secret");
        assert_eq!(secure_value.borrow().as_deref(), Some("legacy-secret"));
        let saved: serde_json::Value =
            serde_json::from_str(saved.borrow().as_deref().unwrap()).unwrap();
        assert!(saved["online_subtitles"].get("api_key").is_none());
        assert_eq!(saved["online_subtitles"]["auto_search"], true);
    }

    #[test]
    fn existing_secure_key_wins_and_legacy_value_is_removed() {
        let mut preferences = Preferences::parse(LEGACY_SETTINGS).unwrap();
        let saved = RefCell::new(None::<String>);
        restore_preferences_with(
            &mut preferences,
            Some(LEGACY_SETTINGS),
            Path::new("preferences.json"),
            || Ok(Some("secure-secret".into())),
            |_| panic!("an existing secure key must not be overwritten"),
            |_, text| {
                *saved.borrow_mut() = Some(text.to_owned());
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(preferences.online_subtitles.api_key, "secure-secret");
        let saved: serde_json::Value =
            serde_json::from_str(saved.borrow().as_deref().unwrap()).unwrap();
        assert!(saved["online_subtitles"].get("api_key").is_none());
    }

    #[test]
    fn failed_secure_migration_keeps_legacy_file_and_usable_session_key() {
        let mut preferences = Preferences::parse(LEGACY_SETTINGS).unwrap();
        let saved = RefCell::new(false);
        let result = restore_preferences_with(
            &mut preferences,
            Some(LEGACY_SETTINGS),
            Path::new("preferences.json"),
            || Ok(None),
            |_| Err("credential store unavailable".into()),
            |_, _| {
                *saved.borrow_mut() = true;
                Ok(())
            },
        );

        assert_eq!(result.unwrap_err(), "credential store unavailable");
        assert_eq!(preferences.online_subtitles.api_key, "legacy-secret");
        assert!(!saved.into_inner());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_credential_store_persists_and_deletes_a_unique_entry() {
        use super::{load_entry, store_entry};

        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let account = format!("test-{}-{stamp}", std::process::id());
        let entry = keyring::Entry::new(super::SERVICE, &account).unwrap();
        struct Cleanup(keyring::Entry);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = self.0.delete_credential();
            }
        }
        let cleanup = Cleanup(entry);
        store_entry(&cleanup.0, "credential-round-trip-test").unwrap();
        assert_eq!(
            load_entry(&cleanup.0).unwrap().as_deref(),
            Some("credential-round-trip-test")
        );
        store_entry(&cleanup.0, "").unwrap();
        assert_eq!(load_entry(&cleanup.0).unwrap(), None);
    }
}
