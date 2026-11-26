//! System and cache actions from the source Settings > Utilities page.
use crate::PreferencesWindow;
use riina_ui::{UtilityAction, UtilitySettingsState};
use slint::ComponentHandle;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub(crate) fn connect(
    window: &PreferencesWindow,
    preferences_path: &Path,
    clear_history: impl Fn() -> Result<usize, String> + 'static,
) {
    let watch_later = preferences_path.with_file_name("watch-later");
    let weak = window.as_weak();
    let progress_directory = watch_later.clone();
    window.on_utilities_refresh_sizes(move || {
        if let Some(window) = weak.upgrade() {
            refresh_sizes(&window, &progress_directory);
        }
    });
    install_default_app_actions(window);
    install_browser_extension_action(window);
    install_cache_and_history_actions(window, preferences_path.to_owned(), clear_history);
}

fn install_browser_extension_action(window: &PreferencesWindow) {
    use riina_ui::BrowserExtension;

    window
        .global::<riina_ui::UtilitySettingsState>()
        .set_utilities_safari_available(cfg!(target_os = "macos"));
    let weak = window.as_weak();
    window.on_utilities_browser_extension(move |browser| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let filename = match browser {
            BrowserExtension::Chromium => "setup-chromium.html",
            BrowserExtension::Firefox => "setup-firefox.html",
            BrowserExtension::Safari => "setup-safari.html",
        };
        let result = browser_extension_guide(filename)
            .and_then(|path| crate::platform::open_associated_path(&path));
        let status = result.err().map_or_else(String::new, |error| {
            format!("Could not open setup: {error}")
        });
        window
            .global::<riina_ui::UtilitySettingsState>()
            .set_utilities_status(status.into());
    });
}

fn browser_extension_guide(filename: &str) -> io::Result<PathBuf> {
    let executable = std::env::current_exe()?;
    let directory = executable
        .parent()
        .ok_or_else(|| io::Error::other("The application folder is unavailable."))?;
    let root = if cfg!(target_os = "macos") {
        directory.join("../Resources")
    } else {
        directory.to_path_buf()
    };
    let packaged = root.join("browser-extension").join(filename);
    #[cfg(debug_assertions)]
    if !packaged.is_file() {
        let checkout = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../browser/open-in-riina");
        let guide = checkout.join(filename);
        if guide.is_file() {
            return Ok(guide);
        }
    }
    if packaged.is_file() {
        Ok(packaged)
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "The browser extension folder is missing.",
        ))
    }
}

fn refresh_sizes(window: &PreferencesWindow, watch_later: &Path) {
    let settings = window.global::<UtilitySettingsState>();
    settings.set_utilities_progress_size(display_size(watch_later).into());
    let thumbnails = thumbnail_cache_directory()
        .map_or_else(|_| "—".into(), |directory| display_size(&directory));
    settings.set_utilities_thumbnail_size(thumbnails.into());
}

fn install_default_app_actions(window: &PreferencesWindow) {
    let weak = window.as_weak();
    window.on_utilities_open_default_apps(move || {
        if let Some(window) = weak.upgrade() {
            window
                .global::<riina_ui::UtilitySettingsState>()
                .set_utilities_default_app_sheet_open(true);
        }
    });

    let weak = window.as_weak();
    window.on_utilities_default_apps_requested(move |video, audio, playlists| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let settings = window.global::<riina_ui::UtilitySettingsState>();
        if !video && !audio && !playlists {
            settings.set_utilities_status("Select at least one media type.".into());
            return;
        }
        crate::platform::file_associations::set_for_media(&window, video, audio, playlists);
    });
}

fn install_cache_and_history_actions(
    window: &PreferencesWindow,
    preferences_path: PathBuf,
    clear_history: impl Fn() -> Result<usize, String> + 'static,
) {
    let weak = window.as_weak();
    let watch_later = preferences_path.with_file_name("watch-later");
    window.on_utility_confirmed(move |action| {
        let Some(window) = weak.upgrade() else {
            return slint::SharedString::default();
        };
        let settings = window.global::<UtilitySettingsState>();
        settings.set_utilities_status("".into());
        match action {
            UtilityAction::None => slint::SharedString::default(),
            // The pinned release retains this action without any suppressible alerts.
            UtilityAction::RestoreAlerts => settings.get_alerts_restored(),
            UtilityAction::ClearProgress => {
                let cleared = clear_watch_later(&watch_later);
                crate::playback_history::progress::files_cleared(
                    &preferences_path,
                    cleared.is_ok(),
                );
                crate::player_ui::refresh_playlist_resume_positions();
                settings.set_utilities_progress_size(display_size(&watch_later).into());
                match cleared {
                    Ok(bytes) => settings.invoke_progress_cleared(format_size(bytes).into()),
                    Err(error) => {
                        format!("Could not clear saved playback progress: {error}").into()
                    }
                }
            }
            UtilityAction::ClearHistory => match clear_history() {
                Ok(removed) => settings.invoke_history_cleared(removed.to_string().into()),
                Err(error) => error.into(),
            },
            UtilityAction::ClearThumbnails => {
                let directory = match thumbnail_cache_directory() {
                    Ok(directory) => directory,
                    Err(error) => return error.into(),
                };
                let cleared = clear_thumbnail_cache(&directory);
                settings.set_utilities_thumbnail_size(display_size(&directory).into());
                match cleared {
                    Ok(bytes) => {
                        settings.set_utilities_status(
                            settings.invoke_thumbnails_cleared(format_size(bytes).into()),
                        );
                        slint::SharedString::default()
                    }
                    Err(error) => format!("Could not clear thumbnail cache: {error}").into(),
                }
            }
        }
    });
}

fn display_size(directory: &Path) -> String {
    directory_size(directory)
        .map(format_size)
        .unwrap_or_else(|_| "—".into())
}

pub(crate) fn thumbnail_cache_directory() -> Result<std::path::PathBuf, String> {
    let cache = dirs::cache_dir().ok_or("The user cache directory is unavailable")?;
    Ok(cache.join("riina").join("thumb_cache"))
}

pub(crate) fn clear_playback_history(preferences_path: &Path) -> Result<usize, String> {
    let mut removed = 0;
    crate::preference_store::update(
        preferences_path,
        &riina_core::Preferences::default(),
        |preferences| {
            let history_path =
                crate::playback_history_store::path_for_preferences(preferences_path);
            removed =
                crate::playback_history_store::load(&history_path, &preferences.playback_history)?
                    .len();
            crate::playback_history_store::save(&history_path, &[])?;
            preferences.playback_history.clear();
            Ok(())
        },
    )?;
    Ok(removed)
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
    use riina_test_support::TempDir;

    #[test]
    fn clear_progress_removes_entries_and_preserves_its_directory() {
        let directory = TempDir::new("utilities").unwrap();
        let root = directory.path();
        let watch_later = root.join("watch-later");
        fs::create_dir_all(watch_later.join("nested")).unwrap();
        fs::write(watch_later.join("one.conf"), b"resume").unwrap();
        fs::write(watch_later.join("nested").join("two.conf"), b"more").unwrap();

        assert_eq!(clear_watch_later(&watch_later).unwrap(), 10);
        assert!(watch_later.is_dir());
        assert_eq!(fs::read_dir(&watch_later).unwrap().count(), 0);
    }

    #[test]
    fn clear_progress_refuses_an_unrelated_directory() {
        let directory = TempDir::new("utilities").unwrap();
        let path = directory.path().join("documents");
        assert_eq!(
            clear_watch_later(&path).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }

    #[test]
    fn clear_thumbnail_cache_removes_entries_and_preserves_its_directory() {
        let directory = TempDir::new("utilities").unwrap();
        let root = directory.path().join("riina");
        let cache = root.join("thumb_cache");
        fs::create_dir_all(cache.join("nested")).unwrap();
        fs::write(cache.join("one.jpg"), b"image").unwrap();
        fs::write(cache.join("nested").join("two.jpg"), b"more").unwrap();

        assert_eq!(clear_thumbnail_cache(&cache).unwrap(), 9);
        assert!(cache.is_dir());
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 0);
    }

    #[test]
    fn clear_thumbnail_cache_refuses_a_directory_outside_its_owned_path() {
        let directory = TempDir::new("utilities").unwrap();
        let path = directory.path().join("documents").join("thumb_cache");
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
        let directory = TempDir::new("utilities").unwrap();
        let root = directory.path();
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
                title: None,
                watch_later_key: None,
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
    }

    #[test]
    fn clear_history_does_not_replace_unreadable_preferences() {
        let directory = TempDir::new("utilities").unwrap();
        let root = directory.path();
        let path = root.join("preferences.json");
        let original = b"{ invalid settings";
        fs::write(&path, original).unwrap();

        assert!(clear_playback_history(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
    }
}
