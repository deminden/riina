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
            merge_changes(
                current.entry(key.clone()).or_insert(Value::Null),
                old.get(key).unwrap_or(&Value::Null),
                value,
            );
        }
        for key in old.keys().filter(|key| !new.contains_key(*key)) {
            current.remove(key);
        }
    } else {
        *latest = next.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn path(label: &str) -> PathBuf {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        std::env::temp_dir().join(format!(
            "riina-store-{}-{}-{label}/preferences.json",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ))
    }
    #[test]
    fn independent_writers_preserve_settings_and_can_return_to_initial_values() {
        let path = path("interleave");
        let initial = Preferences::default();
        let settings = Writer::new(path.clone(), initial.clone());
        let layout = Writer::new(path.clone(), initial.clone());
        let session = Writer::new(path.clone(), initial.clone());
        let mut changed = initial.clone();
        changed.video_threads = 7;
        changed.load_icc_profile = !initial.load_icc_profile;
        settings.save(&changed).unwrap();
        let mut resized = initial.clone();
        resized.leading_sidebar_width += 30;
        layout.save(&resized).unwrap();
        session.save(&initial).unwrap();
        let saved = Preferences::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved.video_threads, 7);
        assert_eq!(saved.load_icc_profile, changed.load_icc_profile);
        assert_eq!(saved.leading_sidebar_width, resized.leading_sidebar_width);
        settings.save(&initial).unwrap();
        let saved = Preferences::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved.video_threads, initial.video_threads);
        assert_eq!(saved.leading_sidebar_width, resized.leading_sidebar_width);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn concurrent_players_merge_disjoint_changes() {
        let path = path("concurrent");
        let workers = (0..2)
            .map(|index| {
                let path = path.clone();
                std::thread::spawn(move || {
                    let initial = Preferences::default();
                    let writer = Writer::new(path, initial.clone());
                    let mut next = initial;
                    if index == 0 {
                        next.video_threads = 9;
                    } else {
                        next.volume = 42.0;
                    }
                    writer.save(&next).unwrap();
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            worker.join().unwrap();
        }
        let saved = Preferences::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved.video_threads, 9);
        assert_eq!(saved.volume, 42.0);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn invalid_existing_file_is_preserved_and_failed_write_can_be_retried() {
        let path = path("failure");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "not json").unwrap();
        let initial = Preferences::default();
        let writer = Writer::new(path.clone(), initial.clone());
        let mut next = initial;
        next.video_threads = 3;
        assert!(writer.save(&next).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "not json");
        std::fs::remove_file(&path).unwrap();
        writer.save(&next).unwrap();
        assert_eq!(
            Preferences::parse(&std::fs::read_to_string(&path).unwrap())
                .unwrap()
                .video_threads,
            3
        );
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
