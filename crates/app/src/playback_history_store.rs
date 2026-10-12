//! Versioned, atomic persistence for playback history independent of preferences.

use md5::{Digest, Md5};
use riina_core::playback_history::Entry;
use serde::{Deserialize, Serialize};
use std::io::BufRead;
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Archive {
    schema_version: u32,
    entries: Vec<Entry>,
}

pub(crate) fn path_for_preferences(preferences_path: &Path) -> PathBuf {
    preferences_path.with_file_name("playback-history.json")
}

/// Read the saved playback position using mpv's watch-later filename convention.
/// The MD5 here is solely for compatibility with mpv's persisted filenames.
pub(crate) fn progress_from_watch_later(
    media_reference: &str,
    watch_later_directory: &Path,
    ignore_path: bool,
) -> Option<f64> {
    let watch_later_path =
        watch_later_file_path(media_reference, watch_later_directory, ignore_path)?;
    let file = std::fs::File::open(watch_later_path).ok()?;
    let first_line = std::io::BufReader::new(file).lines().next()?.ok()?;
    let value = first_line.strip_prefix("start=")?.parse::<f64>().ok()?;
    (value.is_finite() && value >= 0.0).then_some(value)
}

fn watch_later_file_path(
    media_reference: &str,
    watch_later_directory: &Path,
    ignore_path: bool,
) -> Option<PathBuf> {
    let is_windows_drive_path = media_reference.as_bytes().get(1) == Some(&b':')
        && media_reference
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic)
        && matches!(media_reference.as_bytes().get(2), Some(b'/') | Some(b'\\'));

    let identifier = if !is_windows_drive_path {
        match url::Url::parse(media_reference) {
            Ok(url) if url.scheme() == "file" => {
                let path = url.to_file_path().ok()?;
                if ignore_path {
                    path.file_name()?.to_string_lossy().into_owned()
                } else {
                    // mpv hashes the native media path. Forward slashes keep the
                    // file URL's path stable across Rust's Windows path spelling.
                    path.to_string_lossy().replace('\\', "/")
                }
            }
            Ok(url) => url.as_str().to_owned(),
            Err(_) => local_path_identifier(media_reference, ignore_path)?,
        }
    } else {
        local_path_identifier(media_reference, ignore_path)?
    };
    Some(watch_later_directory.join(md5_hex(identifier.as_bytes())))
}

pub(crate) fn md5_hex(input: &[u8]) -> String {
    Md5::digest(input)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn local_path_identifier(media_reference: &str, ignore_path: bool) -> Option<String> {
    let path = Path::new(media_reference);
    if ignore_path {
        Some(path.file_name()?.to_string_lossy().into_owned())
    } else {
        Some(media_reference.replace('\\', "/"))
    }
}

pub(crate) fn load(path: &Path, legacy_entries: &[Entry]) -> Result<Vec<Entry>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(legacy_entries
                .iter()
                .filter(|entry| entry.is_valid())
                .cloned()
                .collect());
        }
        Err(error) => return Err(error.to_string()),
    };
    let archive: Archive = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    if archive.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "Unsupported playback history schema version {}",
            archive.schema_version
        ));
    }
    if archive.entries.iter().any(|entry| !entry.is_valid()) {
        return Err("Playback history contains invalid entries".into());
    }
    Ok(archive.entries)
}

pub(crate) fn save(path: &Path, entries: &[Entry]) -> Result<(), String> {
    if entries.iter().any(|entry| !entry.is_valid()) {
        return Err("Playback history contains invalid entries".into());
    }
    let archive = Archive {
        schema_version: SCHEMA_VERSION,
        entries: entries.to_vec(),
    };
    let text = serde_json::to_string_pretty(&archive).map_err(|error| error.to_string())?;
    crate::platform::save_preferences(path, &text).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "riina-playback-history-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn entry(path: &str, date_ms: u64) -> Entry {
        Entry {
            path: path.into(),
            date_ms,
            progress: None,
            duration: 0.0,
        }
    }

    #[test]
    fn archive_is_atomic_json_with_versioned_load_and_legacy_migration() {
        let path = temporary_path();
        let legacy = [entry("legacy.mkv", 1)];
        assert_eq!(load(&path, &legacy).unwrap(), legacy);

        let entries = [entry("loaded.mkv", 2)];
        save(&path, &entries).unwrap();
        assert_eq!(load(&path, &legacy).unwrap(), entries);
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("\"schema_version\": 1")
        );
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn unsupported_or_invalid_archives_are_preserved_as_errors() {
        let path = temporary_path();
        std::fs::write(&path, r#"{"schema_version":2,"entries":[]}"#).unwrap();
        assert!(load(&path, &[]).unwrap_err().contains("Unsupported"));
        std::fs::write(
            &path,
            r#"{"schema_version":1,"entries":[{"path":"","date_ms":0,"progress":null,"duration":0.0}]}"#,
        )
        .unwrap();
        assert!(load(&path, &[]).unwrap_err().contains("invalid"));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn watch_later_names_match_mpv_md5_inputs_for_urls_and_ignored_local_paths() {
        let directory = std::env::temp_dir();
        let hello = watch_later_file_path("hello", &directory, false).unwrap();
        assert_eq!(
            hello.file_name().unwrap(),
            "5d41402abc4b2a76b9719d911017c592"
        );

        let remote =
            watch_later_file_path("https://media.example/video.mkv", &directory, true).unwrap();
        let expected = md5_hex(b"https://media.example/video.mkv");
        assert_eq!(remote.file_name().unwrap().to_string_lossy(), expected);

        let first = watch_later_file_path("folder-a/video.mkv", &directory, true).unwrap();
        let second = watch_later_file_path("folder-b/video.mkv", &directory, true).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn watch_later_progress_reads_only_a_valid_start_value_from_the_first_line() {
        let directory = std::env::temp_dir().join(format!(
            "riina-watch-later-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let media = "https://media.example/video.mkv";
        let file = watch_later_file_path(media, &directory, false).unwrap();

        std::fs::write(&file, "start=42.25\nvolume=30\n").unwrap();
        assert_eq!(
            progress_from_watch_later(media, &directory, false),
            Some(42.25)
        );

        for invalid in ["volume=30\nstart=42.25\n", "start=NaN\n", "start=-1\n"] {
            std::fs::write(&file, invalid).unwrap();
            assert_eq!(progress_from_watch_later(media, &directory, false), None);
        }
        std::fs::remove_dir_all(directory).unwrap();
    }
}
