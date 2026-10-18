//! Serialize field changes against the latest preferences, including other players.
use riina_core::Preferences;
use serde_json::Value;
use std::{
    cell::RefCell,
    fs::OpenOptions,
    path::{Path, PathBuf},
};

pub(crate) struct Writer {
    path: PathBuf,
    observed: RefCell<Preferences>,
}

impl Writer {
    pub(crate) fn new(path: PathBuf, initial: Preferences) -> Self {
        Self {
            path,
            observed: RefCell::new(initial),
        }
    }

    pub(crate) fn save(&self, candidate: &Preferences) -> Result<(), String> {
        let previous = serde_json::to_value(&*self.observed.borrow()).map_err(|e| e.to_string())?;
        let next = serde_json::to_value(candidate).map_err(|e| e.to_string())?;
        update(&self.path, &self.observed.borrow(), |latest| {
            let mut value = serde_json::to_value(&*latest).map_err(|e| e.to_string())?;
            merge_changes(&mut value, &previous, &next);
            *latest = serde_json::from_value(value).map_err(|e| e.to_string())?;
            Ok(())
        })?;
        // Advance only after a successful commit, so failed writes remain retryable.
        *self.observed.borrow_mut() = candidate.clone();
        Ok(())
    }
}

pub(crate) fn update(
    path: &Path,
    fallback: &Preferences,
    change: impl FnOnce(&mut Preferences) -> Result<(), String>,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    // Atomic replacement alone does not serialize read/modify/write. The stable
    // sidecar lock also coordinates independently launched Windows players.
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path.with_extension("json.lock"))
        .map_err(|e| e.to_string())?;
    lock.lock().map_err(|e| e.to_string())?;
    let mut preferences = match std::fs::read_to_string(path) {
        Ok(text) => Preferences::parse(&text).map_err(|e| e.to_string())?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fallback.clone(),
        Err(error) => return Err(error.to_string()),
    };
    change(&mut preferences)?;
    let text = serde_json::to_string_pretty(&preferences).map_err(|e| e.to_string())?;
    Preferences::parse(&text)?;
    crate::platform::save_preferences(path, &text).map_err(|e| e.to_string())
}

fn merge_changes(latest: &mut Value, previous: &Value, next: &Value) {
    if previous == next {
        return;
    }
    if let (Value::Object(current), Value::Object(old), Value::Object(new)) =
        (&mut *latest, previous, next)
    {
        for (key, value) in new {
            match old.get(key) {
                Some(previous) if previous == value => {}
                Some(previous) => merge_changes(
                    current.entry(key.clone()).or_insert(Value::Null),
                    previous,
                    value,
                ),
                None => {
                    current.insert(key.clone(), value.clone());
                }
            }
        }
        for key in old.keys().filter(|key| !new.contains_key(*key)) {
            current.remove(key);
        }
    } else {
        *latest = next.clone();
    }
}

#[cfg(test)]
#[path = "preference_store_tests.rs"]
mod tests;
