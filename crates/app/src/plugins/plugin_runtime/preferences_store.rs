//! Package-scoped preferences shared by the global and player realms.
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
    path: Option<PathBuf>,
    state: Mutex<State>,
}

impl Store {
    fn new(path: Option<PathBuf>) -> Self {
        Self {
            path,
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
        let identifier = crate::plugin_files::safe_identifier(identifier);
        let path = plugins
            .join(".preferences")
            .join(format!("{identifier}.json"));
        // Settings belong to the installed package even when its last window
        // closes. Each new realm sees the pending edits until sync or app exit.
        let stores = STORES.get_or_init(Mutex::default);
        let mut stores = riina_core::sync::lock(stores);
        Ok(stores
            .entry(path.clone())
            .or_insert_with(|| Arc::new(Self::new(Some(path))))
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
                self.path
                    .as_deref()
                    .map(read)
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
        let Some(path) = self.path.as_deref() else {
            return Ok(());
        };
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
        let mut saved = read(path)?;
        saved.extend(state.pending.clone());
        let text = encode(&saved)?;
        crate::platform::save_preferences(path, &text).map_err(|error| error.to_string())?;
        state.saved = Some(saved);
        state.pending.clear();
        Ok(())
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

fn read(path: &Path) -> Result<Values, String> {
    if !verify_file(path)? {
        return Ok(Values::new());
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
    serde_json::from_slice(&contents)
        .map_err(|error| format!("Could not read plugin preferences: {error}"))
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
        let first = Store::new(Some(path.clone()));
        let second = Store::new(Some(path.clone()));
        assert_eq!(first.get("retained").unwrap(), Some(json!(1)));
        assert_eq!(second.get("retained").unwrap(), Some(json!(1)));
        first.set("first".into(), json!(true)).unwrap();
        second.set("second".into(), json!("next")).unwrap();

        let lock = File::create(path.with_extension("json.lock")).unwrap();
        lock.lock().unwrap();
        assert!(second.sync().is_err());
        assert_eq!(second.get("second").unwrap(), Some(json!("next")));
        assert_eq!(read(&path).unwrap().len(), 1);
        drop(lock);

        first.sync().unwrap();
        second.sync().unwrap();
        assert_eq!(
            json!(read(&path).unwrap()),
            json!({"first": true, "retained": 1, "second": "next"})
        );

        fs::write(&path, "invalid JSON").unwrap();
        first.set("retry".into(), json!([1, 2])).unwrap();
        assert!(first.sync().is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "invalid JSON");
        assert_eq!(first.get("retry").unwrap(), Some(json!([1, 2])));
    }
}
