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
    match thumbnail_cache_directory() {
        Ok(directory) => {
            let size = fs::create_dir_all(&directory)
                .and_then(|()| directory_size(&directory))
                .map(format_size)
                .unwrap_or_else(|_| "—".into());
            window.set_utilities_thumbnail_size(size.into());
        }
        Err(_) => window.set_utilities_thumbnail_size("—".into()),
    }

    let weak = window.as_weak();
    window.on_utilities_open_default_apps(move || {
        if let Some(window) = weak.upgrade() {
            window.set_utilities_default_app_sheet_open(true);
        }
    });

    let weak = window.as_weak();
    window.on_utilities_default_apps_requested(move |video, audio, playlists| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        if !video && !audio && !playlists {
            window.set_utilities_status("Select at least one media type.".into());
            return;
        }
        #[cfg(target_os = "linux")]
        {
            let weak_window = window.as_weak();
            window.set_utilities_status("Updating file associations…".into());
            let worker = std::thread::Builder::new()
                .name("riina-file-associations".into())
                .spawn(move || {
                    let message = match register_default_file_types(video, audio, playlists) {
                        Ok(count) => default_app_status(count),
                        Err(error) => error,
                    };
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(window) = weak_window.upgrade() {
                            window.set_utilities_status(message.into());
                        }
                    });
                });
            if let Err(error) = worker {
                window.set_utilities_status(
                    format!("Could not start file-association update: {error}").into(),
                );
            }
        }
        #[cfg(not(target_os = "linux"))]
        match register_default_file_types(video, audio, playlists) {
            Ok(count) => match open_default_apps() {
                Ok(()) => window.set_utilities_status(default_app_status(count).into()),
                Err(error) => window.set_utilities_status(
                    format!("Registered {count} file types, but could not open Windows Default Apps: {error}").into(),
                ),
            },
            Err(error) => window.set_utilities_status(error.into()),
        }
    });

    let weak = window.as_weak();
    window.on_utilities_restore_alerts(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let response = MessageDialog::new()
            .set_title("Restore Suppressed Alerts")
            .set_description("Are you sure you want to restore suppressed alerts?")
            .set_buttons(MessageButtons::YesNo)
            .show();
        if response != MessageDialogResult::Yes {
            return;
        }
        let _ = MessageDialog::new()
            .set_title("Restore Suppressed Alerts")
            .set_description("Restored.")
            .set_buttons(MessageButtons::Ok)
            .show();
        window.set_utilities_status("Restored.".into());
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
                format!("Playback history cleared ({removed} entries removed).").into(),
            ),
            Err(error) => window
                .set_utilities_status(format!("Could not clear playback history: {error}").into()),
        }
    });

    let weak = window.as_weak();
    window.on_utilities_clear_thumbnail_cache(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let directory = match thumbnail_cache_directory() {
            Ok(directory) => directory,
            Err(error) => {
                window.set_utilities_status(error.into());
                return;
            }
        };
        let size = directory_size(&directory)
            .map(format_size)
            .unwrap_or_else(|_| "—".into());
        let response = MessageDialog::new()
            .set_title("Clear Thumbnail Cache")
            .set_description(format!(
                "Delete all cached thumbnails? Current size: {size}."
            ))
            .set_buttons(MessageButtons::YesNo)
            .show();
        if response != MessageDialogResult::Yes {
            return;
        }
        match clear_thumbnail_cache(&directory) {
            Ok(removed_bytes) => {
                window.set_utilities_thumbnail_size(format_size(0).into());
                window.set_utilities_status(
                    format!(
                        "Thumbnail cache cleared ({} removed).",
                        format_size(removed_bytes)
                    )
                    .into(),
                );
            }
            Err(error) => window
                .set_utilities_status(format!("Could not clear thumbnail cache: {error}").into()),
        }
    });
}

pub(crate) fn thumbnail_cache_directory() -> Result<std::path::PathBuf, String> {
    let cache = dirs::cache_dir().ok_or("The user cache directory is unavailable")?;
    Ok(cache.join("riina").join("thumb_cache"))
}

pub(crate) fn clear_playback_history(preferences_path: &Path) -> Result<usize, String> {
    let mut preferences = match fs::read_to_string(preferences_path) {
        Ok(text) => riina_core::Preferences::parse(&text).map_err(|error| error.to_string())?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => riina_core::Preferences::default(),
        Err(error) => return Err(error.to_string()),
    };
    let history_path = crate::playback_history_store::path_for_preferences(preferences_path);
    let entries =
        crate::playback_history_store::load(&history_path, &preferences.playback_history)?;
    let removed = entries.len();
    preferences.playback_history.clear();
    crate::playback_history_store::save(&history_path, &[])?;
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
        preferences.playback_history = saved.playback_history;
    }
}

#[cfg(not(target_os = "linux"))]
fn open_default_apps() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer.exe")
            .arg("ms-settings:defaultapps")
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("Could not open Windows Default Apps: {error}"))
    }
    #[cfg(target_os = "macos")]
    {
        Ok(())
    }
}

fn register_default_file_types(video: bool, audio: bool, playlists: bool) -> Result<usize, String> {
    #[cfg(target_os = "windows")]
    {
        crate::platform::register_default_file_types(video, audio, playlists)
            .map_err(|error| format!("Could not register Riina file types: {error}"))
    }
    #[cfg(target_os = "macos")]
    {
        crate::platform::register_default_file_types(video, audio, playlists)
            .map_err(|error| format!("Could not register Riina file types: {error}"))
    }
    #[cfg(target_os = "linux")]
    {
        crate::platform::register_default_file_types(video, audio, playlists)
            .map_err(|error| format!("Could not register Riina file types: {error}"))
    }
}

fn default_app_status(count: usize) -> String {
    #[cfg(target_os = "windows")]
    {
        format!(
            "Registered {count} file types. Choose Riina for the selected types in Windows Default Apps."
        )
    }
    #[cfg(target_os = "macos")]
    {
        format!("Launch Services set Riina as the default for {count} media types.")
    }
    #[cfg(target_os = "linux")]
    {
        format!("Riina is now the default for {count} selected media types.")
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

fn clear_thumbnail_cache(directory: &Path) -> io::Result<u64> {
    if directory
        .file_name()
        .is_none_or(|name| name != "thumb_cache")
        || directory
            .parent()
            .and_then(Path::file_name)
            .is_none_or(|name| name != "riina")
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the thumbnail cache must be inside the Riina cache directory",
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
            "the thumbnail cache path is not a regular directory",
        ));
    }

    let mut removed_bytes = 0u64;
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() {
            continue;
        } else if metadata.is_dir() {
            removed_bytes = removed_bytes.saturating_add(directory_size(&entry.path())?);
            fs::remove_dir_all(entry.path())?;
        } else {
            removed_bytes = removed_bytes.saturating_add(metadata.len());
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
    fn clear_thumbnail_cache_removes_entries_and_preserves_its_directory() {
        let root = temporary_directory().join("riina");
        let cache = root.join("thumb_cache");
        fs::create_dir_all(cache.join("nested")).unwrap();
        fs::write(cache.join("one.jpg"), b"image").unwrap();
        fs::write(cache.join("nested").join("two.jpg"), b"more").unwrap();

        assert_eq!(clear_thumbnail_cache(&cache).unwrap(), 9);
        assert!(cache.is_dir());
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 0);
        fs::remove_dir_all(root.parent().unwrap()).unwrap();
    }

    #[test]
    fn clear_thumbnail_cache_refuses_a_directory_outside_its_owned_path() {
        let path = temporary_directory().join("documents").join("thumb_cache");
        assert_eq!(
            clear_thumbnail_cache(&path).unwrap_err().kind(),
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
    fn clear_history_preserves_recent_media_and_other_preferences() {
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
            playback_history: vec![riina_core::playback_history::Entry {
                path: "file:///C:/media/movie.mkv".into(),
                date_ms: 1_750_000_000_000,
                progress: Some(12.0),
                duration: 90.0,
            }],
            ..riina_core::Preferences::default()
        };
        let mut encoded = serde_json::to_value(&preferences).unwrap();
        encoded["playback_history"] = serde_json::to_value(&preferences.playback_history).unwrap();
        fs::write(&path, serde_json::to_vec(&encoded).unwrap()).unwrap();

        assert_eq!(clear_playback_history(&path).unwrap(), 1);
        let saved = riina_core::Preferences::parse(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(saved.playback_history.is_empty());
        assert_eq!(saved.recent_media.len(), 1);
        assert_eq!(saved.volume, 63.0);
        assert!(
            crate::playback_history_store::load(
                &crate::playback_history_store::path_for_preferences(&path),
                &[],
            )
            .unwrap()
            .is_empty()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn clear_history_does_not_replace_unreadable_preferences() {
        let root = temporary_directory();
        fs::create_dir_all(&root).unwrap();
        let path = root.join("preferences.json");
        let original = b"{ invalid settings";
        fs::write(&path, original).unwrap();

        assert!(clear_playback_history(&path).is_err());
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
        stale
            .playback_history
            .push(riina_core::playback_history::Entry {
                path: "stale.mkv".into(),
                date_ms: 1,
                progress: None,
                duration: 0.0,
            });

        preserve_saved_history(&mut stale, &path);
        assert!(stale.recent_media.is_empty());
        assert!(stale.playback_history.is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
