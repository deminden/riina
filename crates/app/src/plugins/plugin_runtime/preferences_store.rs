//! Package-scoped preferences shared by the global and player realms.
mod legacy;

use serde_json::{Map, Value};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, Read},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};

pub(super) const MAX_PREFERENCE_BYTES: usize = 1024 * 1024;
type Values = Map<String, Value>;

#[derive(Default)]
struct State {
    saved: Option<Values>,
    pending: Values,
}

pub(super) struct Store {
    files: Option<Files>,
    state: Mutex<State>,
}

struct Files {
    json: PathBuf,
    legacy_name: String,
}

impl Store {
    fn new(files: Option<Files>) -> Self {
        Self {
            files,
            state: Mutex::new(State::default()),
        }
    }

    pub(super) fn for_package(root: &Path, identifier: &str) -> Result<Arc<Self>, String> {
        static STORES: OnceLock<Mutex<BTreeMap<PathBuf, Arc<Store>>>> = OnceLock::new();
        let plugins = root
            .parent()
            .unwrap_or(root)
            .canonicalize()
            .map_err(|error| error.to_string())?;
        let stem = crate::plugin_files::safe_identifier(identifier);
        let path = plugins.join(".preferences").join(format!("{stem}.json"));
        // Settings belong to the installed package even when its last window
        // closes. Each new realm sees the pending edits until sync or app exit.
        let stores = STORES.get_or_init(Mutex::default);
        let mut stores = riina_core::sync::lock(stores);
        Ok(stores
            .entry(path.clone())
            .or_insert_with(|| {
                Arc::new(Self::new(Some(Files {
                    json: path,
                    legacy_name: format!("{identifier}.plist"),
                })))
            })
            .clone())
    }

    #[cfg(test)]
    pub(super) fn temporary() -> Arc<Self> {
        Arc::new(Self::new(None))
    }

    pub(super) fn get(&self, key: &str) -> Result<Option<Value>, String> {
        let mut state = self.state.lock().map_err(|error| error.to_string())?;
        if let Some(value) = state.pending.get(key) {
            return Ok(Some(value.clone()));
        }
        if state.saved.is_none() {
            state.saved = Some(
                self.files
                    .as_ref()
                    .map(Files::read)
                    .transpose()?
                    .unwrap_or_default(),
            );
        }
        Ok(state
            .saved
            .as_ref()
            .and_then(|saved| saved.get(key))
            .cloned())
    }

    pub(super) fn set(&self, key: String, value: Value) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|error| error.to_string())?;
        let mut next = state.pending.clone();
        next.insert(key, value);
        encode(&next)?;
        state.pending = next;
        Ok(())
    }

    pub(super) fn sync(&self) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|error| error.to_string())?;
        if state.pending.is_empty() {
            return Ok(());
        }
        let Some(files) = self.files.as_ref() else {
            return Ok(());
        };
        let path = &files.json;
        let directory = directory(path)?;
        fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        verify_directory(directory)?;

        let lock_path = path.with_extension("json.lock");
        verify_file(&lock_path)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)
            .map_err(|error| error.to_string())?;
        lock.try_lock()
            .map_err(|error| format!("Could not lock plugin preferences: {error}"))?;

        // Merge only this process's edits with the last durable copy. A write
        // failure leaves those edits available for the next sync attempt.
        let mut saved = files.read()?;
        saved.extend(state.pending.clone());
        let text = encode(&saved)?;
        crate::platform::save_preferences(path, &text).map_err(|error| error.to_string())?;
        state.saved = Some(saved);
        state.pending.clear();
        Ok(())
    }
}

impl Files {
    fn read(&self) -> Result<Values, String> {
        if let Some(contents) = read_file(&self.json)? {
            return serde_json::from_slice(&contents)
                .map_err(|error| format!("Could not read plugin preferences: {error}"));
        }
        let Some(path) = self.legacy_path()? else {
            return Ok(Values::new());
        };
        read_file(&path)?
            .map(|contents| legacy::decode(&contents))
            .transpose()
            .map(Option::unwrap_or_default)
    }

    fn legacy_path(&self) -> Result<Option<PathBuf>, String> {
        let directory = directory(&self.json)?;
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.to_string()),
        };
        verify_directory(directory)?;
        let portable = self.json.with_extension("plist");
        let mut fallback = None;
        // Match the on-disk spelling so different IINA identifiers remain
        // separate even on case-insensitive volumes.
        for entry in entries {
            let entry = entry.map_err(|error| error.to_string())?;
            let name = entry.file_name();
            if name == self.legacy_name.as_str() {
                return Ok(Some(entry.path()));
            }
            if Some(name.as_os_str()) == portable.file_name() {
                fallback = Some(entry.path());
            }
        }
        Ok(fallback)
    }
}

fn directory(path: &Path) -> Result<&Path, String> {
    path.parent()
        .ok_or_else(|| "Plugin preferences have no parent directory".into())
}

fn verify_directory(path: &Path) -> Result<(), String> {
    let canonical = path.canonicalize().map_err(|error| error.to_string())?;
    if canonical != path || !canonical.is_dir() {
        return Err("Plugin preferences must use their own directory".into());
    }
    Ok(())
}

fn verify_file(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(true),
        Ok(_) => Err("Plugin preferences must use a regular file".into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.to_string()),
    }
}

fn read_file(path: &Path) -> Result<Option<Vec<u8>>, String> {
    if !verify_file(path)? {
        return Ok(None);
    }
    verify_directory(directory(path)?)?;
    let mut contents = Vec::new();
    File::open(path)
        .and_then(|file| {
            file.take(MAX_PREFERENCE_BYTES as u64 + 1)
                .read_to_end(&mut contents)
        })
        .map_err(|error| error.to_string())?;
    if contents.len() > MAX_PREFERENCE_BYTES {
        return Err("Plugin preferences exceed 1 MiB".into());
    }
    Ok(Some(contents))
}

fn encode(values: &Values) -> Result<String, String> {
    let contents = serde_json::to_string(values).map_err(|error| error.to_string())?;
    if contents.len() > MAX_PREFERENCE_BYTES {
        return Err("Plugin preferences exceed 1 MiB".into());
    }
    Ok(contents)
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_test_support::TempDir;
    use serde_json::json;

    #[test]
    fn sync_merges_independent_writers_and_retries_a_failed_commit() {
        let scratch = TempDir::new("plugin-preference-writers").unwrap();
        let path = scratch.path().canonicalize().unwrap().join("state.json");
        fs::write(&path, r#"{"retained":1}"#).unwrap();
        let files = || Files {
            json: path.clone(),
            legacy_name: "state.plist".into(),
        };
        let first = Store::new(Some(files()));
        let second = Store::new(Some(files()));
        assert_eq!(first.get("retained").unwrap(), Some(json!(1)));
        assert_eq!(second.get("retained").unwrap(), Some(json!(1)));
        first.set("first".into(), json!(true)).unwrap();
        second.set("second".into(), json!("next")).unwrap();

        let lock = File::create(path.with_extension("json.lock")).unwrap();
        lock.lock().unwrap();
        assert!(second.sync().is_err());
        assert_eq!(second.get("second").unwrap(), Some(json!("next")));
        assert_eq!(files().read().unwrap().len(), 1);
        drop(lock);

        first.sync().unwrap();
        second.sync().unwrap();
        assert_eq!(
            json!(files().read().unwrap()),
            json!({"first": true, "retained": 1, "second": "next"})
        );

        fs::write(&path, "invalid JSON").unwrap();
        first.set("retry".into(), json!([1, 2])).unwrap();
        assert!(first.sync().is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "invalid JSON");
        assert_eq!(first.get("retry").unwrap(), Some(json!([1, 2])));
    }

    #[test]
    fn binary_preferences_keep_case_sensitive_identity_and_merge_on_sync() {
        let scratch = TempDir::new("plugin-preference-plist").unwrap();
        let package = scratch.path().join("plugin.iinaplugin");
        let directory = scratch.path().join(".preferences");
        fs::create_dir(&package).unwrap();
        fs::create_dir(&directory).unwrap();
        let original = directory.join("org.example.Panel.plist");
        let old = plist::Value::Dictionary(
            [
                ("pinned", plist::Value::Boolean(false)),
                ("flags", plist::Value::Data(vec![0, 127, 255])),
                (
                    "date",
                    plist::Value::Date(
                        plist::Date::from_xml_format("2024-05-01T10:12:45Z").unwrap(),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
        );
        old.to_file_binary(&original).unwrap();
        let before = fs::read(&original).unwrap();

        let other = Store::for_package(&package, "org.example.panel").unwrap();
        assert_eq!(other.get("pinned").unwrap(), None);
        let store = Store::for_package(&package, "org.example.Panel").unwrap();
        assert_eq!(store.get("pinned").unwrap(), Some(json!(false)));
        assert_eq!(store.get("flags").unwrap(), Some(json!([0, 127, 255])));
        assert_eq!(
            store.get("date").unwrap(),
            Some(json!("2024-05-01T10:12:45Z"))
        );
        let files = store.files.as_ref().unwrap();
        assert!(!files.json.exists());
        store.set("pinned".into(), json!(true)).unwrap();
        store.sync().unwrap();
        assert_eq!(fs::read(&original).unwrap(), before);
        assert_eq!(
            json!(files.read().unwrap()),
            json!({"pinned":true, "flags":[0,127,255], "date":"2024-05-01T10:12:45Z"})
        );
    }
}
