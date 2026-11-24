//! Serialize field changes against the latest preferences, including other players.
use riina_core::Preferences;
use serde_json::Value;
use std::{
    cell::RefCell,
    fs::{self, OpenOptions},
    io,
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
        self.save_with_policy(candidate, true)
    }

    pub(crate) fn save_session(&self, candidate: &Preferences) -> Result<(), String> {
        self.save_with_policy(candidate, false)
    }

    fn save_with_policy(&self, candidate: &Preferences, user_edit: bool) -> Result<(), String> {
        let previous = serde_json::to_value(&*self.observed.borrow()).map_err(|e| e.to_string())?;
        let next = serde_json::to_value(candidate).map_err(|e| e.to_string())?;
        let allow_repair = user_edit && previous != next;
        let written = update_inner(
            &self.path,
            &self.observed.borrow(),
            allow_repair,
            |latest| merge_profile_edit(latest, &previous, &next),
        )?;
        // Advance only after a successful commit, so failed writes remain retryable.
        if written {
            *self.observed.borrow_mut() = candidate.clone();
        }
        Ok(())
    }
}

#[cfg(any(target_os = "macos", test))]
pub(crate) fn save_form_edit(
    path: &Path,
    saved: &Preferences,
    previous_form: &Preferences,
    edited_form: &Preferences,
) -> Result<(), String> {
    let previous = serde_json::to_value(previous_form).map_err(|error| error.to_string())?;
    let next = serde_json::to_value(edited_form).map_err(|error| error.to_string())?;
    if previous == next {
        return Ok(());
    }
    update(path, saved, |latest| {
        merge_profile_edit(latest, &previous, &next)
    })
}

fn merge_profile_edit(
    latest: &mut Preferences,
    previous: &Value,
    next: &Value,
) -> Result<(), String> {
    let mut value = serde_json::to_value(&*latest).map_err(|error| error.to_string())?;
    merge_changes(&mut value, previous, next);
    *latest = serde_json::from_value(value).map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn update(
    path: &Path,
    fallback: &Preferences,
    change: impl FnOnce(&mut Preferences) -> Result<(), String>,
) -> Result<(), String> {
    update_inner(path, fallback, true, change).map(|_| ())
}

pub(crate) fn update_automatically(
    path: &Path,
    fallback: &Preferences,
    change: impl FnOnce(&mut Preferences) -> Result<(), String>,
) -> Result<(), String> {
    update_inner(path, fallback, false, change).map(|_| ())
}

fn update_inner(
    path: &Path,
    fallback: &Preferences,
    allow_repair: bool,
    change: impl FnOnce(&mut Preferences) -> Result<(), String>,
) -> Result<bool, String> {
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
    let Some((mut preferences, repairing)) = read_for_update(path, fallback, allow_repair)? else {
        return Ok(false);
    };
    let recovered = if repairing {
        Some(serde_json::to_value(&preferences).map_err(|error| error.to_string())?)
    } else {
        None
    };
    change(&mut preferences)?;
    if let Some(recovered) = recovered
        && serde_json::to_value(&preferences).map_err(|error| error.to_string())? == recovered
    {
        return Ok(false);
    }
    let text = serde_json::to_string_pretty(&preferences).map_err(|e| e.to_string())?;
    Preferences::parse(&text)?;
    crate::platform::save_preferences(path, &text).map_err(|e| e.to_string())?;
    Ok(true)
}

fn read_for_update(
    path: &Path,
    fallback: &Preferences,
    allow_repair: bool,
) -> Result<Option<(Preferences, bool)>, String> {
    let original = match fs::read(path) {
        Ok(original) => original,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(Some((fallback.clone(), false)));
        }
        Err(error) => return Err(error.to_string()),
    };
    let text = std::str::from_utf8(&original);
    let parsed = text
        .map_err(|error| error.to_string())
        .and_then(crate::preference_load::parse_object);
    match parsed {
        Ok(preferences) => Ok(Some((preferences, false))),
        Err(error) if !crate::preference_load::has_matching_backup(path, &original) => Err(error),
        Err(_) if !allow_repair => Ok(None),
        Err(_) => Ok(Some((crate::preference_load::recover(text.ok()).0, true))),
    }
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
