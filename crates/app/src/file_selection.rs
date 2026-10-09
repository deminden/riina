//! Local-file selection rules from the pinned PlayerCore/DraggingDetect workflow.
use std::{cmp::Ordering, collections::HashSet, fs, path::Path};
use windows::{Win32::UI::Shell::StrCmpLogicalW, core::PCWSTR};

// Utility.blacklistExt: explicitly selected unknown formats remain eligible.
const BLACKLIST: &[&str] = &[
    "utf", "utf8", "utf-8", "idx", "sub", "srt", "smi", "rt", "ssa", "aqt", "jss", "js", "ass",
    "mks", "vtt", "sup", "scc", "lrc", "m3u", "m3u8", "pls",
];

// Compatibility data, not a decoder capability claim. Folder discovery uses a
// positive list; explicit file selection deliberately permits unknown formats.
const DISCOVERABLE: &[&str] = &[
    "mkv", "mp4", "avi", "m4v", "mov", "3gp", "ts", "mts", "m2ts", "wmv", "flv", "f4v", "asf",
    "webm", "rm", "rmvb", "qt", "dv", "mpg", "mpeg", "mxf", "vob", "gif", "ogv", "ogm", "mp3",
    "aac", "mka", "dts", "flac", "ogg", "oga", "mogg", "m4a", "ac3", "opus", "wav", "wv", "aiff",
    "aif", "ape", "tta", "tak",
];

pub fn prepare(paths: Vec<String>, open_media: bool) -> Result<Vec<String>, String> {
    if paths
        .iter()
        .any(|path| path.is_empty() || path.contains('\0'))
    {
        return Err("The media path is empty or contains a NUL byte".into());
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
    for path in paths {
        if Path::new(&path).is_dir() {
            discover(Path::new(&path), &mut files)?;
        } else if !BLACKLIST.contains(&extension(&path).as_str()) {
            files.push(path);
        }
    }
    let mut seen = HashSet::new();
    files.retain(|path| seen.insert(path.clone()));
    files.sort_by(|left, right| {
        let left_path = Path::new(left);
        let right_path = Path::new(right);
        match left_path.parent().cmp(&right_path.parent()) {
            Ordering::Equal => {
                let left_name = left_path.file_name().unwrap_or_default().to_string_lossy();
                let right_name = right_path.file_name().unwrap_or_default().to_string_lossy();
                natural_compare(&left_name, &right_name).then_with(|| left.cmp(right))
            }
            order => order,
        }
    });
    Ok(files)
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
                if DISCOVERABLE.contains(&extension(&path).as_str()) {
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

fn natural_compare(left: &str, right: &str) -> Ordering {
    let left: Vec<u16> = left.encode_utf16().chain(Some(0)).collect();
    let right: Vec<u16> = right.encode_utf16().chain(Some(0)).collect();
    // SAFETY: both arrays are NUL-terminated and live throughout this read-only call.
    // Explorer's comparator supplies the Windows equivalent of localizedStandardCompare.
    // Locale-specific ordering is an explicit platform adaptation, not pixel/byte parity.
    unsafe { StrCmpLogicalW(PCWSTR(left.as_ptr()), PCWSTR(right.as_ptr())).cmp(&0) }
}

#[cfg(test)]
mod tests {
    use super::prepare;

    struct TestTree(std::path::PathBuf);

    impl TestTree {
        fn new() -> Self {
            let base = std::env::temp_dir().canonicalize().unwrap();
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = base.join(format!("wiina-discovery-{}-{stamp}", std::process::id()));
            std::fs::create_dir(&path).unwrap();
            Self(path)
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
    fn selected_files_keep_unknown_formats_but_filter_subtitles_and_sort_numbers() {
        let paths = [
            r"C:\media\clip10.mkv",
            r"C:\media\clip2.mkv",
            r"C:\media\CAPTIONS.SRT",
            r"C:\media\clip2.mkv",
            r"C:\media\list.m3u8",
            r"C:\media\custom.format",
        ];
        assert_eq!(
            prepare(paths.map(String::from).into(), true).unwrap(),
            [
                r"C:\media\clip2.mkv",
                r"C:\media\clip10.mkv",
                r"C:\media\custom.format"
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
