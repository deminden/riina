//! Local-file selection rules from the pinned PlayerCore/DraggingDetect workflow.
use std::{cmp::Ordering, collections::HashSet, path::Path};
use windows::{Win32::UI::Shell::StrCmpLogicalW, core::PCWSTR};

// Utility.blacklistExt: explicitly selected unknown formats remain eligible.
const BLACKLIST: &[&str] = &[
    "utf", "utf8", "utf-8", "idx", "sub", "srt", "smi", "rt", "ssa", "aqt", "jss", "js", "ass",
    "mks", "vtt", "sup", "scc", "lrc", "m3u", "m3u8", "pls",
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
        && matches!(
            extension(&paths[0]).as_str(),
            "cue" | "m3u" | "m3u8" | "pls"
        )
    {
        return Ok(paths);
    }
    let mut seen = HashSet::new();
    let mut files: Vec<_> = paths
        .into_iter()
        .filter(|path| !BLACKLIST.contains(&extension(path).as_str()))
        .filter(|path| seen.insert(path.clone()))
        .collect();
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
