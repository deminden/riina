//! Local-file selection rules from the pinned PlayerCore/DraggingDetect workflow.
use crate::media_reference::NetworkUrlError;
use std::{cmp::Ordering, collections::HashSet, fs, path::Path};

// Utility.blacklistExt also excludes multi-file playlists from directory scans.
const MULTI_FILE_PLAYLIST_EXTENSIONS: &[&str] = &["m3u", "m3u8", "pls"];
pub const SUPPORTED_SUBTITLE_EXTENSIONS: &[&str] = &[
    "utf", "utf8", "utf-8", "idx", "sub", "srt", "smi", "rt", "ssa", "aqt", "jss", "js", "ass",
    "mks", "vtt", "sup", "scc", "lrc",
];

// IINA's supportedFileExt sets drive folder scans and same-folder playlist adds.
pub const SUPPORTED_VIDEO_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "avi", "m4v", "mov", "3gp", "ts", "mts", "m2ts", "wmv", "flv", "f4v", "asf",
    "webm", "rm", "rmvb", "qt", "dv", "mpg", "mpeg", "mxf", "vob", "gif", "ogv", "ogm",
];
pub const SUPPORTED_AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "aac", "mka", "dts", "flac", "ogg", "oga", "mogg", "m4a", "ac3", "opus", "wav", "wv",
    "aiff", "aif", "ape", "tta", "tak",
];

#[derive(Debug)]
pub enum SelectionError {
    InvalidPath,
    InvalidUnicode,
    Url(NetworkUrlError),
    ScanFolder(std::io::Error),
    ReadEntry(std::io::Error),
}

impl std::fmt::Display for SelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPath => f.write_str("The media path is empty or contains a NUL byte"),
            Self::InvalidUnicode => f.write_str("The path contains invalid Unicode"),
            Self::Url(error) => error.fmt(f),
            Self::ScanFolder(error) => write!(f, "Cannot scan folder: {error}"),
            Self::ReadEntry(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for SelectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Url(error) => Some(error),
            Self::ScanFolder(error) | Self::ReadEntry(error) => Some(error),
            Self::InvalidPath | Self::InvalidUnicode => None,
        }
    }
}

pub fn prepare(
    paths: Vec<String>,
    open_media: bool,
    compare_names: impl Fn(&str, &str) -> Ordering,
) -> Result<Vec<String>, SelectionError> {
    if paths
        .iter()
        .any(|path| path.is_empty() || path.contains('\0'))
    {
        return Err(SelectionError::InvalidPath);
    }
    if open_media && paths.len() == 1 && crate::media_reference::is_network_url_candidate(&paths[0])
    {
        return Ok(vec![
            crate::media_reference::validate_network_url(&paths[0])
                .map_err(SelectionError::Url)?
                .to_owned(),
        ]);
    }
    // PlayerCore.openURLs lets mpv expand a single playlist before applying the
    // regular blacklist. Playlist additions deliberately use the regular rules.
    if open_media
        && paths.len() == 1
        && (is_bluray_folder(Path::new(&paths[0]))
            || matches!(
                extension(&paths[0]).as_str(),
                "cue" | "m3u" | "m3u8" | "pls"
            ))
    {
        return Ok(paths);
    }
    let mut files = Vec::new();
    let mut local_files = Vec::new();
    let mut seen = HashSet::new();
    for path in paths {
        if open_media && crate::media_reference::is_network_url_candidate(&path) {
            // URLs are already explicit playlist entries; never treat their
            // slashes as directories or reorder them with local selections.
            flush_local_files(&mut local_files, &mut files, &mut seen, &compare_names);
            let url = crate::media_reference::validate_network_url(&path)
                .map_err(SelectionError::Url)?
                .to_owned();
            if seen.insert(url.clone()) {
                files.push(url);
            }
            continue;
        }
        if Path::new(&path).is_dir() {
            scan_folder(
                Path::new(&path),
                || false,
                |media| {
                    local_files.push(media);
                    false
                },
            )?;
        } else if accepts_playlist_file(&path) {
            local_files.push(path);
        }
    }
    flush_local_files(&mut local_files, &mut files, &mut seen, &compare_names);
    Ok(files)
}

/// Check a hovered path without collecting or sorting an entire directory.
pub fn can_add_to_playlist(path: &str, cancelled: impl Fn() -> bool) -> bool {
    if path.is_empty() || path.contains('\0') || cancelled() {
        return false;
    }
    if Path::new(path).is_dir() {
        scan_folder(Path::new(path), cancelled, |_| true).unwrap_or(false)
    } else {
        accepts_playlist_file(path)
    }
}

fn accepts_playlist_file(path: &str) -> bool {
    let extension = extension(path);
    !SUPPORTED_SUBTITLE_EXTENSIONS.contains(&extension.as_str())
        && !MULTI_FILE_PLAYLIST_EXTENSIONS.contains(&extension.as_str())
}

/// Recognize the subtitle formats accepted by IINA's file-drop fallback.
pub fn is_supported_subtitle(path: &str) -> bool {
    !Path::new(path).is_dir() && SUPPORTED_SUBTITLE_EXTENSIONS.contains(&extension(path).as_str())
}

pub fn is_supported_media(path: &str) -> bool {
    let extension = extension(path);
    SUPPORTED_VIDEO_EXTENSIONS.contains(&extension.as_str())
        || SUPPORTED_AUDIO_EXTENSIONS.contains(&extension.as_str())
}

pub fn is_supported_video(path: &str) -> bool {
    SUPPORTED_VIDEO_EXTENSIONS.contains(&extension(path).as_str())
}

fn flush_local_files(
    local_files: &mut Vec<String>,
    files: &mut Vec<String>,
    seen: &mut HashSet<String>,
    compare_names: &impl Fn(&str, &str) -> Ordering,
) {
    local_files.sort_by(|left, right| {
        let left_path = Path::new(left);
        let right_path = Path::new(right);
        match left_path.parent().cmp(&right_path.parent()) {
            Ordering::Equal => {
                let left_name = left_path.file_name().unwrap_or_default().to_string_lossy();
                let right_name = right_path.file_name().unwrap_or_default().to_string_lossy();
                compare_names(&left_name, &right_name).then_with(|| left.cmp(right))
            }
            order => order,
        }
    });
    files.extend(
        local_files
            .drain(..)
            .filter(|path| seen.insert(path.clone())),
    );
}

fn is_bluray_folder(path: &Path) -> bool {
    [path.to_path_buf(), path.join("BDMV")]
        .iter()
        .any(|folder| {
            folder.join("MovieObject.bdmv").is_file() && folder.join("index.bdmv").is_file()
        })
}

fn scan_folder(
    root: &Path,
    cancelled: impl Fn() -> bool,
    mut accept: impl FnMut(String) -> bool,
) -> Result<bool, SelectionError> {
    // Iterative traversal bounds call-stack use for deep libraries. Do not follow
    // directory links into cycles or another tree; explicitly selected roots work.
    let mut pending = vec![(root.to_path_buf(), true)];
    while let Some((folder, is_root)) = pending.pop() {
        if cancelled() {
            return Ok(false);
        }
        let entries = fs::read_dir(&folder).map_err(SelectionError::ScanFolder)?;
        for entry in entries {
            if cancelled() {
                return Ok(false);
            }
            let entry = entry.map_err(SelectionError::ReadEntry)?;
            if is_root && entry.file_name().to_string_lossy().starts_with('.') {
                // The compatibility rule checks the root-relative prefix, rather
                // than every component: visible/.nested/video.mkv remains eligible.
                continue;
            }
            let path = entry.path();
            let kind = entry.file_type().map_err(SelectionError::ReadEntry)?;
            if kind.is_dir() {
                pending.push((path, false));
            } else if !kind.is_symlink() {
                let path = path
                    .into_os_string()
                    .into_string()
                    .map_err(|_| SelectionError::InvalidUnicode)?;
                if is_supported_media(&path) && accept(path) {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

fn extension(path: &str) -> String {
    Path::new(path)
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::SelectionError;
    fn prepare(paths: Vec<String>, open_media: bool) -> Result<Vec<String>, SelectionError> {
        // Synthetic ordering verifies comparator injection without pretending that
        // a portable test reproduces a native locale's filename collation.
        super::prepare(paths, open_media, |left, right| {
            left.len().cmp(&right.len()).then_with(|| left.cmp(right))
        })
    }

    struct TestTree {
        _directory: riina_test_support::TempDir,
        root: std::path::PathBuf,
    }

    impl TestTree {
        fn new() -> Self {
            let directory = riina_test_support::TempDir::new("file-selection").unwrap();
            let root = directory.path().canonicalize().unwrap();
            Self {
                _directory: directory,
                root,
            }
        }

        fn path(&self, name: &str) -> String {
            self.root.join(name).into_os_string().into_string().unwrap()
        }

        fn file(&self, name: &str) {
            let path = self.root.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, []).unwrap();
        }
    }

    #[test]
    fn folder_discovery_filters_unknowns_and_root_hidden_entries_without_duplicates() {
        let tree = TestTree::new();
        for name in [
            "clip10.MKV",
            "clip2.mkv",
            "caption.srt",
            "custom.format",
            ".hidden/video.mkv",
            "visible/.nested/音楽.wav",
            "visible/deep/movie.mp4",
        ] {
            tree.file(name);
        }
        let files = prepare(vec![tree.path(""), tree.path("clip2.mkv")], true).unwrap();
        assert_eq!(
            files,
            [
                tree.path("clip2.mkv"),
                tree.path("clip10.MKV"),
                tree.path("visible/.nested/音楽.wav"),
                tree.path("visible/deep/movie.mp4")
            ]
        );
        // Unknown extensions are eligible only when explicitly selected.
        assert_eq!(
            prepare(vec![tree.path("custom.format")], true).unwrap(),
            [tree.path("custom.format")]
        );
    }

    #[test]
    fn playlist_hover_uses_the_same_file_and_folder_rules_as_insertion() {
        use super::can_add_to_playlist;

        let tree = TestTree::new();
        tree.file("empty/captions.srt");
        tree.file("empty/.hidden/clip.mkv");
        tree.file("mixed/visible/.nested/clip.mkv");
        for (name, accepted) in [
            ("empty", false),
            ("mixed", true),
            ("empty/captions.srt", false),
            ("folder.m3u8", false),
            ("recording.unusual", true),
        ] {
            assert_eq!(can_add_to_playlist(&tree.path(name), || false), accepted);
        }
        assert!(!can_add_to_playlist(&tree.path("mixed"), || true));
    }

    #[test]
    fn single_disc_structure_is_preserved_for_native_playlist_expansion() {
        let tree = TestTree::new();
        tree.file("BDMV/MovieObject.bdmv");
        tree.file("BDMV/index.bdmv");
        tree.file("BDMV/STREAM/title.m2ts");
        assert_eq!(prepare(vec![tree.path("")], true).unwrap(), [tree.path("")]);
        assert_eq!(
            prepare(vec![tree.path("")], false).unwrap(),
            [tree.path("BDMV/STREAM/title.m2ts")]
        );
    }

    #[test]
    fn selected_files_filter_and_deduplicate_with_injected_order() {
        let paths = [
            r"media/clip10.mkv",
            r"media/clip2.mkv",
            r"media/CAPTIONS.SRT",
            r"media/clip2.mkv",
            r"media/list.m3u8",
            r"media/custom.format",
        ];
        assert_eq!(
            prepare(paths.map(String::from).into(), true).unwrap(),
            [
                r"media/clip2.mkv",
                r"media/clip10.mkv",
                r"media/custom.format"
            ]
        );
    }

    #[test]
    fn playlist_file_bypass_applies_only_to_single_media_open() {
        assert_eq!(
            prepare(vec!["selection.M3U8".into()], true).unwrap(),
            ["selection.M3U8"]
        );
        assert!(
            prepare(vec!["selection.M3U8".into()], false)
                .unwrap()
                .is_empty()
        );
        assert!(prepare(vec!["valid.mkv".into(), "bad\0file.mkv".into()], true).is_err());
        assert!(prepare(Vec::new(), true).unwrap().is_empty());
    }
}
