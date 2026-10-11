//! System and cache actions from the source Settings > Utilities page.
use crate::PreferencesWindow;
use rfd::{MessageButtons, MessageDialog, MessageDialogResult};
use slint::ComponentHandle;
use std::{fs, io, path::Path};

pub(crate) fn connect(
    window: &PreferencesWindow,
    preferences_path: &Path,
    clear_history: impl Fn() -> Result<usize, String> + 'static,
) {
    let watch_later = preferences_path.with_file_name("watch-later");
    window.set_utilities_progress_size(
        directory_size(&watch_later)
            .map(format_size)
            .unwrap_or_else(|_| "—".into())
            .into(),
    );

    let weak = window.as_weak();
    window.on_utilities_open_default_apps(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        match open_default_apps() {
            Ok(()) => window.set_utilities_status(
                "Choose Riina in Windows Default Apps. File associations appear after installation."
                    .into(),
            ),
            Err(error) => window.set_utilities_status(error.into()),
        }
    });

    let weak = window.as_weak();
    window.on_utilities_restore_alerts(move || {
        if let Some(window) = weak.upgrade() {
            // The pinned source currently has no suppressible alerts either; keep
            // the action present for compatibility without clearing unrelated data.
            window.set_utilities_status("There are no suppressed alerts to restore.".into());
        }
    });

    let weak = window.as_weak();
    window.on_utilities_clear_progress(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let response = MessageDialog::new()
            .set_title("Clear Saved Playback Progress")
            .set_description(format!(
                "Delete saved playback positions for all media? Current size: {}.",
                window.get_utilities_progress_size()
            ))
            .set_buttons(MessageButtons::YesNo)
            .show();
        if response != MessageDialogResult::Yes {
            return;
        }
        match clear_watch_later(&watch_later) {
            Ok(removed_bytes) => {
                window.set_utilities_progress_size(format_size(0).into());
                window.set_utilities_status(
                    format!(
                        "Saved playback progress cleared ({} removed).",
                        format_size(removed_bytes)
                    )
                    .into(),
                );
            }
            Err(error) => window.set_utilities_status(
                format!("Could not clear saved playback progress: {error}").into(),
            ),
        }
    });

    let weak = window.as_weak();
    window.on_utilities_clear_history(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let response = MessageDialog::new()
            .set_title("Clear Playback History")
            .set_description("Delete all recent playback history?")
            .set_buttons(MessageButtons::YesNo)
            .show();
        if response != MessageDialogResult::Yes {
            return;
        }
        match clear_history() {
            Ok(removed) => window.set_utilities_status(
                format!("Playback history cleared ({removed} recent items removed).").into(),
            ),
            Err(error) => window
                .set_utilities_status(format!("Could not clear playback history: {error}").into()),
        }
    });
}

pub(crate) fn clear_recent_history(preferences_path: &Path) -> Result<usize, String> {
    let mut preferences = match fs::read_to_string(preferences_path) {
        Ok(text) => riina_core::Preferences::parse(&text).map_err(|error| error.to_string())?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => riina_core::Preferences::default(),
        Err(error) => return Err(error.to_string()),
    };
    let removed = preferences.recent_media.len();
    preferences.recent_media.clear();
    let encoded = serde_json::to_string_pretty(&preferences).map_err(|error| error.to_string())?;
    crate::platform::save_preferences(preferences_path, &encoded)
        .map_err(|error| error.to_string())?;
    Ok(removed)
}

pub(crate) fn preserve_saved_history(preferences: &mut riina_core::Preferences, path: &Path) {
    if let Ok(text) = fs::read_to_string(path)
        && let Ok(saved) = riina_core::Preferences::parse(&text)
    {
        preferences.recent_media = saved.recent_media;
    }
}

fn open_default_apps() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer.exe")
            .arg("ms-settings:defaultapps")
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("Could not open Windows Default Apps: {error}"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("Default-app registration is not available on this platform yet.".into())
    }
}

fn clear_watch_later(directory: &Path) -> io::Result<u64> {
    if directory
        .file_name()
        .is_none_or(|name| name != "watch-later")
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the progress directory must be named watch-later",
        ));
    }
    let metadata = match fs::symlink_metadata(directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir_all(directory)?;
            return Ok(0);
        }
        Err(error) => return Err(error),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the progress path is not a regular directory",
        ));
    }

    let mut removed_bytes = 0;
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() {
            // Watch-later must never turn a cache clear into deletion outside
            // the app-owned progress directory.
            continue;
        } else if metadata.is_dir() {
            removed_bytes += directory_size(&entry.path())?;
            fs::remove_dir_all(entry.path())?;
        } else {
            removed_bytes += metadata.len();
            fs::remove_file(entry.path())?;
        }
    }
    Ok(removed_bytes)
}

fn directory_size(directory: &Path) -> io::Result<u64> {
    let mut size = 0u64;
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            size = size.saturating_add(directory_size(&entry.path())?);
        } else {
            size = size.saturating_add(metadata.len());
        }
    }
    Ok(size)
}

fn format_size(bytes: u64) -> String {
    if bytes < 1_024 {
        format!("{bytes} B")
    } else if bytes < 1_048_576 {
        format!("{:.1} KiB", bytes as f64 / 1_024.0)
    } else {
        format!("{:.1} MiB", bytes as f64 / 1_048_576.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_directory() -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("riina-utilities-{}-{nonce}", std::process::id()))
    }

    #[test]
    fn clear_progress_removes_entries_and_preserves_its_directory() {
        let root = temporary_directory();
        let watch_later = root.join("watch-later");
        fs::create_dir_all(watch_later.join("nested")).unwrap();
        fs::write(watch_later.join("one.conf"), b"resume").unwrap();
        fs::write(watch_later.join("nested").join("two.conf"), b"more").unwrap();

        assert_eq!(clear_watch_later(&watch_later).unwrap(), 10);
        assert!(watch_later.is_dir());
        assert_eq!(fs::read_dir(&watch_later).unwrap().count(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn clear_progress_refuses_an_unrelated_directory() {
        let path = temporary_directory().join("documents");
        assert_eq!(
            clear_watch_later(&path).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }

    #[test]
    fn sizes_use_binary_units_like_the_source_cache_label() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(1_024), "1.0 KiB");
        assert_eq!(format_size(1_048_576), "1.0 MiB");
    }

    #[test]
    fn clear_history_persists_empty_recents_without_resetting_preferences() {
        let root = temporary_directory();
        fs::create_dir_all(&root).unwrap();
        let path = root.join("preferences.json");
        let preferences = riina_core::Preferences {
            volume: 63.0,
            recent_media: vec![riina_core::RecentMedia {
                path: "C:/media/movie.mkv".into(),
                title: "movie".into(),
                position: 12.0,
            }],
            ..riina_core::Preferences::default()
        };
        fs::write(&path, serde_json::to_vec(&preferences).unwrap()).unwrap();

        assert_eq!(clear_recent_history(&path).unwrap(), 1);
        let saved = riina_core::Preferences::parse(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(saved.recent_media.is_empty());
        assert_eq!(saved.volume, 63.0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn clear_history_does_not_replace_unreadable_preferences() {
        let root = temporary_directory();
        fs::create_dir_all(&root).unwrap();
        let path = root.join("preferences.json");
        let original = b"{ invalid settings";
        fs::write(&path, original).unwrap();

        assert!(clear_recent_history(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stale_preferences_keep_the_latest_saved_history_state() {
        let root = temporary_directory();
        fs::create_dir_all(&root).unwrap();
        let path = root.join("preferences.json");
        let mut saved = riina_core::Preferences::default();
        saved.recent_media.clear();
        fs::write(&path, serde_json::to_vec(&saved).unwrap()).unwrap();
        let mut stale = riina_core::Preferences::default();
        stale.recent_media.push(riina_core::RecentMedia {
            path: "old.mkv".into(),
            title: "old".into(),
            position: 0.0,
        });

        preserve_saved_history(&mut stale, &path);
        assert!(stale.recent_media.is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
