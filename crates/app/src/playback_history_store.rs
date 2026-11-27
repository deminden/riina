//! Versioned, atomic persistence for playback history independent of preferences.

use md5::{Digest, Md5};
use riina_core::playback_history::{Entry, WatchLaterKey};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::io::{BufRead, BufReader, ErrorKind, Read};
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Archive<'a> {
    schema_version: u32,
    entries: Cow<'a, [Entry]>,
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
    let key = watch_later_key(media_reference, ignore_path)?;
    progress_for_key(&key, watch_later_directory)
}

pub(super) fn progress_for_key(key: &WatchLaterKey, watch_later_directory: &Path) -> Option<f64> {
    let file = std::fs::File::open(watch_later_directory.join(key.as_str())).ok()?;
    // mpv can prefix its options with a 1023-byte filename comment. The
    // truncated name need not be valid UTF-8, but the start value must be.
    const MAX_FILENAME_LINE: usize = 2 + 1023 + 2;
    const MAX_START_LINE: usize = 512;
    let mut reader = BufReader::new(file);
    let mut line = Vec::new();
    reader
        .by_ref()
        .take((MAX_FILENAME_LINE + 1) as u64)
        .read_until(b'\n', &mut line)
        .ok()?;
    if line.starts_with(b"# ") {
        if line.len() > MAX_FILENAME_LINE || !line.ends_with(b"\n") {
            return None;
        }
        line.clear();
        reader
            .take(MAX_START_LINE as u64)
            .read_until(b'\n', &mut line)
            .ok()?;
    }
    if line.len() >= MAX_START_LINE {
        return None;
    }
    let value = std::str::from_utf8(&line)
        .ok()?
        .trim_end_matches(['\r', '\n'])
        .strip_prefix("start=")?
        .parse::<f64>()
        .ok()?;
    (value.is_finite() && value >= 0.0).then_some(value)
}

pub(super) fn watch_later_key(media_reference: &str, ignore_path: bool) -> Option<WatchLaterKey> {
    let identifier = if media_reference
        .split_once(':')
        .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case("file"))
    {
        // History archives can store local entries as file URLs. Decode them
        // to the native path, including the Windows engine's backslashes.
        let path = url::Url::parse(media_reference)
            .ok()
            .and_then(|url| url.to_file_path().ok())
            .or_else(|| crate::file_actions::local_media_path(media_reference))?;
        Cow::Owned(local_path_identifier(path.to_str()?, ignore_path)?.to_owned())
    } else if mpv_url_scheme(media_reference).is_some() {
        Cow::Borrowed(media_reference)
    } else {
        Cow::Borrowed(local_path_identifier(media_reference, ignore_path)?)
    };
    Some(WatchLaterKey::from_digest(
        Md5::digest(identifier.as_bytes()).into(),
    ))
}

pub(super) fn mpv_url_scheme(reference: &str) -> Option<&str> {
    let (scheme, _) = reference.split_once("://")?;
    let mut bytes = scheme.bytes();
    bytes.next().filter(u8::is_ascii_alphabetic)?;
    bytes
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'))
        .then_some(scheme)
}

fn local_path_identifier(media_reference: &str, ignore_path: bool) -> Option<&str> {
    if ignore_path {
        Path::new(media_reference).file_name()?.to_str()
    } else {
        Some(media_reference)
    }
}

pub(crate) fn load(path: &Path) -> Result<Vec<Entry>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
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
    Ok(archive.entries.into_owned())
}

pub(super) fn clear(path: &Path) -> Result<usize, String> {
    let removed = load(path)?.len();
    if removed > 0 {
        save(path, &[])?;
    }
    Ok(removed)
}

pub(crate) fn save(path: &Path, entries: &[Entry]) -> Result<(), String> {
    if entries.iter().any(|entry| !entry.is_valid()) {
        return Err("Playback history contains invalid entries".into());
    }
    let archive = Archive {
        schema_version: SCHEMA_VERSION,
        entries: Cow::Borrowed(entries),
    };
    let text = serde_json::to_string_pretty(&archive).map_err(|error| error.to_string())?;
    crate::platform::save_preferences(path, &text).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_test_support::TempDir;

    fn watch_later_file_path(media: &str, directory: &Path, ignore_path: bool) -> Option<PathBuf> {
        watch_later_key(media, ignore_path).map(|key| directory.join(key.as_str()))
    }

    fn entry(path: &str, date_ms: u64) -> Entry {
        Entry {
            path: path.into(),
            title: None,
            watch_later_key: None,
            date_ms,
            progress: None,
            duration: 0.0,
        }
    }

    #[test]
    fn archive_is_atomic_json_with_versioned_load() {
        let directory = TempDir::new("riina-playback-history").unwrap();
        let path = directory.path().join("history.json");
        assert!(load(&path).unwrap().is_empty());
        assert!(!path.exists());

        let entries = [entry("loaded.mkv", 2)];
        save(&path, &entries).unwrap();
        assert_eq!(load(&path).unwrap(), entries);
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("\"schema_version\": 1")
        );
    }

    #[test]
    fn unsupported_or_invalid_archives_are_preserved_as_errors() {
        let directory = TempDir::new("riina-playback-history").unwrap();
        let path = directory.path().join("history.json");
        std::fs::write(&path, r#"{"schema_version":2,"entries":[]}"#).unwrap();
        assert!(load(&path).unwrap_err().contains("Unsupported"));
        std::fs::write(
            &path,
            r#"{"schema_version":1,"entries":[{"path":"","date_ms":0,"duration":0.0}]}"#,
        )
        .unwrap();
        assert!(load(&path).unwrap_err().contains("invalid"));
    }

    #[test]
    fn a_watch_later_key_cannot_name_an_arbitrary_file() {
        let directory = TempDir::new("riina-history-key-archive").unwrap();
        let path = directory.path().join("history.json");
        let mut document = serde_json::json!({
            "schema_version": 1,
            "entries": [{"path": "sample.mkv", "date_ms": 1, "duration": 90.0}]
        });
        std::fs::write(&path, document.to_string()).unwrap();
        let loaded = load(&path).unwrap();
        assert!(loaded[0].watch_later_key.is_none());
        assert!(loaded[0].progress.is_none());

        document["entries"][0]["watch_later_key"] = "5d41402abc4b2a76b9719d911017c592".into();
        std::fs::write(&path, document.to_string()).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(
            loaded[0].watch_later_key.as_ref().unwrap().as_str(),
            "5D41402ABC4B2A76B9719D911017C592"
        );

        document["entries"][0]["watch_later_key"] = "../history.json".into();
        std::fs::write(&path, document.to_string()).unwrap();
        assert!(load(&path).unwrap_err().contains("invalid watch-later key"));
    }

    #[test]
    fn watch_later_names_match_mpv_md5_inputs_for_urls_and_ignored_local_paths() {
        let directory = std::env::temp_dir();
        let hello = watch_later_file_path("hello", &directory, false).unwrap();
        assert_eq!(
            hello.file_name().unwrap(),
            "5D41402ABC4B2A76B9719D911017C592"
        );

        let address = "https://MEDIA.example:443/play/../video.mkv?key=a%2Fb";
        for ignore_path in [false, true] {
            let remote = watch_later_file_path(address, &directory, ignore_path).unwrap();
            assert_eq!(
                remote.file_name().unwrap(),
                "9EF3174EA54291768A2DE5FFFB732535"
            );
        }

        let first = watch_later_file_path("folder-a/video.mkv", &directory, true).unwrap();
        let second = watch_later_file_path("folder-b/video.mkv", &directory, true).unwrap();
        assert_eq!(first, second);

        #[cfg(unix)]
        assert_eq!(
            watch_later_file_path("Folder: Disc/video.mkv", &directory, true),
            Some(first)
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_history_urls_hash_the_same_native_path_as_the_playback_engine() {
        let directory = std::env::temp_dir();
        let native = r"C:\Media\sample file.wav";
        let from_path = watch_later_file_path(native, &directory, false).unwrap();
        let from_url =
            watch_later_file_path("file:///C:/Media/sample%20file.wav", &directory, false).unwrap();
        assert_eq!(from_path, from_url);
        assert_eq!(
            from_path.file_name().unwrap().to_string_lossy(),
            "55DBFAA262FAADDEB6E9FC56A2382DD5"
        );
        assert_ne!(
            from_path.file_name().unwrap().to_string_lossy(),
            "E72C0763AE1CAAED5E3554089B52054E"
        );
    }

    #[test]
    fn watch_later_progress_reads_the_leading_start_with_or_without_a_filename_comment() {
        let temp = TempDir::new("riina-watch-later").unwrap();
        let directory = temp.path();
        let media = "https://media.example/video.mkv";
        let file = watch_later_file_path(media, directory, false).unwrap();

        std::fs::write(&file, "start=42.25\nvolume=30\n").unwrap();
        assert_eq!(
            progress_from_watch_later(media, directory, false),
            Some(42.25)
        );

        let mut with_comment = b"# ".to_vec();
        with_comment.extend([0xc3; 1023]);
        with_comment.extend(b"\r\nstart=36.75\r\n");
        std::fs::write(&file, with_comment).unwrap();
        assert_eq!(
            progress_from_watch_later(media, directory, false),
            Some(36.75)
        );

        for invalid in [
            "volume=30\nstart=42.25\n",
            "# movie.mkv\nvolume=30\nstart=42.25\n",
            "# redirect entry\n# movie.mkv\n",
            "start=NaN\n",
            "start=-1\n",
        ] {
            std::fs::write(&file, invalid).unwrap();
            assert_eq!(progress_from_watch_later(media, directory, false), None);
        }
        std::fs::write(&file, format!("start={}\n", "0".repeat(600))).unwrap();
        assert_eq!(progress_from_watch_later(media, directory, false), None);
        std::fs::write(&file, format!("# {}\nstart=40\n", "x".repeat(1025))).unwrap();
        assert_eq!(progress_from_watch_later(media, directory, false), None);
    }
}
