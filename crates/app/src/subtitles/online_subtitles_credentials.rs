//! Store the OpenSubtitles key in the operating system's secure credential store.

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

pub(crate) fn restore_preferences(preferences: &mut riina_core::Preferences) -> Result<(), String> {
    preferences.online_subtitles.api_key = load()?.unwrap_or_default();
    preferences.online_subtitles.assrt_token = load_assrt()?.unwrap_or_default();
    Ok(())
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
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
