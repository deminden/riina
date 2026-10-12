//! Local-file selection rules from the pinned PlayerCore/DraggingDetect workflow.
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

pub fn prepare(
    paths: Vec<String>,
    open_media: bool,
    compare_names: impl Fn(&str, &str) -> Ordering,
) -> Result<Vec<String>, String> {
    if paths
        .iter()
        .any(|path| path.is_empty() || path.contains('\0'))
    {
        return Err("The media path is empty or contains a NUL byte".into());
    }
    if open_media && paths.len() == 1 && crate::media_reference::is_network_url_candidate(&paths[0])
    {
        return Ok(vec![
            crate::media_reference::validate_network_url(&paths[0])
                .map_err(str::to_owned)?
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
                .map_err(str::to_owned)?
                .to_owned();
            if seen.insert(url.clone()) {
                files.push(url);
            }
            continue;
        }
        if Path::new(&path).is_dir() {
            discover(Path::new(&path), &mut local_files)?;
        } else if !is_supported_subtitle(&path)
            && !MULTI_FILE_PLAYLIST_EXTENSIONS.contains(&extension(&path).as_str())
        {
            local_files.push(path);
        }
    }
    flush_local_files(&mut local_files, &mut files, &mut seen, &compare_names);
    Ok(files)
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

fn discover(root: &Path, files: &mut Vec<String>) -> Result<(), String> {
    // Iterative traversal bounds call-stack use for deep libraries. Do not follow
    // directory links into cycles or another tree; explicitly selected roots work.
    let mut pending = vec![(root.to_path_buf(), true)];
    while let Some((folder, is_root)) = pending.pop() {
        let entries =
            fs::read_dir(&folder).map_err(|error| format!("Cannot scan folder: {error}"))?;
        for entry in entries {
            let entry = entry.map_err(|error| error.to_string())?;
            if is_root && entry.file_name().to_string_lossy().starts_with('.') {
                // The compatibility rule checks the root-relative prefix, rather
                // than every component: visible/.nested/video.mkv remains eligible.
                continue;
            }
            let path = entry.path();
            let kind = entry.file_type().map_err(|error| error.to_string())?;
            if kind.is_dir() {
                pending.push((path, false));
            } else if !kind.is_symlink() {
                let path = path
                    .into_os_string()
                    .into_string()
                    .map_err(|_| "The path contains invalid Unicode")?;
                if is_supported_media(&path) {
                    files.push(path);
                }
            }
        }
    }
    Ok(())
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
    fn prepare(paths: Vec<String>, open_media: bool) -> Result<Vec<String>, String> {
        // Synthetic ordering verifies comparator injection without pretending that
        // a portable test reproduces a native locale's filename collation.
        super::prepare(paths, open_media, |left, right| {
            left.len().cmp(&right.len()).then_with(|| left.cmp(right))
        })
    }

    struct TestTree(std::path::PathBuf);

    impl TestTree {
        fn new() -> Self {
            let base = std::env::temp_dir().canonicalize().unwrap();
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            loop {
                let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let path = base.join(format!(
                    "riina-discovery-{}-{stamp}-{sequence}",
                    std::process::id()
                ));
                // Clock resolution cannot distinguish parallel tests on every OS.
                // Only successful creation establishes ownership for Drop cleanup;
                // an existing directory belongs to someone else and stays intact.
                match std::fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("Cannot create discovery fixture: {error}"),
                }
            }
        }

        fn path(&self, name: &str) -> String {
            self.0.join(name).into_os_string().into_string().unwrap()
        }

        fn file(&self, name: &str) {
            let path = self.0.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, []).unwrap();
        }
    }

    impl Drop for TestTree {
        fn drop(&mut self) {
            let base = std::env::temp_dir().canonicalize().unwrap();
            // Only this uniquely created child is owned by the test, never its parent.
            assert_eq!(self.0.parent(), Some(base.as_path()));
            std::fs::remove_dir_all(&self.0).unwrap();
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
