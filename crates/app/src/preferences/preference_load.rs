//! Read the startup profile and preserve malformed files before using recovered settings.
use riina_core::{Preferences, Rejection};
use slint::ComponentHandle;
use std::{
    fs, io,
    path::{Path, PathBuf},
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

#[derive(Clone)]
pub(crate) struct LoadedPreferences {
    pub preferences: Preferences,
    pub recovery: Option<Recovery>,
}

#[derive(Clone)]
pub(crate) struct Recovery {
    pub quarantined_as: Option<PathBuf>,
    pub reasons: Vec<String>,
    pub reset_sections: Vec<String>,
}

impl Recovery {
    pub(crate) fn notice(&self) -> String {
        match self.quarantined_as.as_deref().and_then(Path::file_name) {
            Some(name) => format!(
                "Some settings could not be read and were reset. The original file was kept as {}.",
                name.to_string_lossy()
            ),
            None => "Some settings could not be read and were reset. The original file remains in place.".into(),
        }
    }

    pub(crate) fn announce(self, app: &crate::PlayerWindow) -> Option<Rc<slint::Timer>> {
        static ANNOUNCED: AtomicBool = AtomicBool::new(false);
        if ANNOUNCED.swap(true, Ordering::AcqRel) {
            return None;
        }
        for (index, reason) in self.reasons.iter().enumerate() {
            if let Some(section) = self.reset_sections.get(index) {
                crate::diagnostics::warn!("Settings recovery [{section}]: {reason}");
            } else {
                crate::diagnostics::warn!("Settings recovery: {reason}");
            }
        }
        app.set_startup_status(self.notice().into());
        // A Welcome window may keep the player hidden until a file is selected.
        // Count the display time only once the player's status line is visible.
        let timer = Rc::new(slint::Timer::default());
        let timer_handle = Rc::downgrade(&timer);
        let weak = app.as_weak();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(250),
            move || {
                let Some(app) = weak.upgrade() else { return };
                if !app.window().is_visible() {
                    return;
                }
                if let Some(timer) = timer_handle.upgrade() {
                    timer.stop();
                }
                let weak = weak.clone();
                slint::Timer::single_shot(Duration::from_secs(8), move || {
                    if let Some(app) = weak.upgrade() {
                        app.set_startup_status("".into());
                    }
                });
            },
        );
        Some(timer)
    }
}

/// Reads valid saved values for field merges that already have a fallback.
pub(crate) fn read_valid(path: &Path) -> io::Result<Option<Preferences>> {
    match fs::read_to_string(path) {
        Ok(text) => Preferences::parse(&text)
            .map(Some)
            .map_err(io::Error::other),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub(crate) fn load(path: &Path) -> io::Result<LoadedPreferences> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(LoadedPreferences {
                preferences: Preferences::default(),
                recovery: None,
            });
        }
        Err(error) => return Err(error),
    };
    let source_text = std::str::from_utf8(&bytes).ok();
    if let Some(text) = source_text
        && let Ok(preferences) = Preferences::parse(text)
    {
        return Ok(LoadedPreferences {
            preferences,
            recovery: None,
        });
    }

    let (preferences, rejected) = recover(source_text);
    let (reset_sections, mut reasons): (Vec<_>, Vec<_>) = rejected
        .into_iter()
        .map(|entry| (entry.key, entry.reason))
        .unzip();
    if reasons.is_empty() {
        // Strict deserialization rejects duplicate known keys. The raw map
        // keeps the last of each repeated key for recovery.
        reasons.push("Expected one value per known preference".into());
    }
    let quarantined_as = match quarantine(path, &bytes, chrono::Utc::now()) {
        Ok(backup) => Some(backup),
        Err(error) => {
            reasons.push(format!("Could not preserve a backup ({:?})", error.kind()));
            None
        }
    };
    Ok(LoadedPreferences {
        preferences,
        recovery: Some(Recovery {
            quarantined_as,
            reasons,
            reset_sections,
        }),
    })
}

pub(crate) fn recover(text: Option<&str>) -> (Preferences, Vec<Rejection>) {
    match text {
        Some(text) => Preferences::parse_lenient(text),
        None => (
            Preferences::default(),
            vec![Rejection {
                key: "$".into(),
                reason: "The file is not UTF-8".into(),
            }],
        ),
    }
}

pub(crate) fn has_matching_backup(path: &Path, original: &[u8]) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    let Ok(entries) = fs::read_dir(parent) else {
        return false;
    };
    // An older backup cannot authorize overwriting damage from a later write.
    entries.filter_map(Result::ok).any(|entry| {
        entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with("preferences.invalid-") && name.ends_with(".json"))
            && entry.file_type().is_ok_and(|kind| kind.is_file())
            && entry
                .metadata()
                .is_ok_and(|metadata| metadata.len() == original.len() as u64)
            && fs::read(entry.path()).is_ok_and(|backup| backup == original)
    })
}

fn quarantine(
    path: &Path,
    contents: &[u8],
    now: chrono::DateTime<chrono::Utc>,
) -> io::Result<PathBuf> {
    let timestamp = now.format("%Y%m%dT%H%M%SZ");
    for sequence in 0..=u32::MAX {
        let name = if sequence == 0 {
            format!("preferences.invalid-{timestamp}.json")
        } else {
            format!("preferences.invalid-{timestamp}-{sequence}.json")
        };
        let backup = path.with_file_name(name);
        match crate::platform::settings_file::save_new(&backup, contents) {
            Ok(()) => return Ok(backup),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::other("No quarantine filename is available"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_test_support::TempDir;

    struct Profile(TempDir);

    impl Profile {
        fn new() -> Self {
            Self(TempDir::new("preference-recovery").unwrap())
        }

        fn preferences(&self) -> PathBuf {
            self.0.path().join("preferences.json")
        }
    }

    #[test]
    fn load_preserves_corrupt_bytes_and_keeps_valid_preferences() {
        for original in [
            br#"{"volume":"bad","theme_material":"light"}"#.as_slice(),
            b"[]",
            b"\xff\xfe",
        ] {
            let profile = Profile::new();
            let path = profile.preferences();
            fs::write(&path, original).unwrap();
            let loaded = load(&path).unwrap();
            assert!(read_valid(&path).is_err());
            let recovery = loaded.recovery.unwrap();
            let backup = recovery.quarantined_as.as_ref().unwrap();
            assert_eq!(fs::read(backup).unwrap(), original);
            assert_eq!(fs::read(&path).unwrap(), original);
            assert!(
                recovery
                    .notice()
                    .contains(backup.file_name().unwrap().to_str().unwrap())
            );
            let theme = if original.starts_with(b"{") {
                riina_core::ThemeMaterial::Light
            } else {
                riina_core::ThemeMaterial::Dark
            };
            assert_eq!(loaded.preferences.theme_material, theme);
            assert_eq!(loaded.preferences.volume, Preferences::default().volume);
        }
    }

    #[test]
    fn each_quarantine_keeps_existing_copies_even_in_the_same_second() {
        let profile = Profile::new();
        let path = profile.preferences();
        let time = chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let first = quarantine(&path, b"first", time).unwrap();
        let second = quarantine(&path, b"second", time).unwrap();
        assert_eq!(
            first.file_name().unwrap(),
            "preferences.invalid-20231114T221320Z.json"
        );
        assert_ne!(first, second);
        assert_eq!(fs::read(first).unwrap(), b"first");
        assert_eq!(fs::read(second).unwrap(), b"second");
        assert!(!path.exists());
    }
}
