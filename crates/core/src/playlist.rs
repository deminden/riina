//! Deterministic playlist ordering for the sidebar sorting actions.
use std::path::Path;

use crate::PlaylistEntry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaylistSort {
    FilenameAscending,
    FilenameDescending,
    PathAscending,
    PathDescending,
}

/// Decode newline-separated clipboard references without splitting spaces inside paths.
pub fn clipboard_references(text: &str) -> Vec<String> {
    text.lines()
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            if let Some(path) = line.strip_prefix("file://") {
                let path = decode_uri_path(path)?;
                #[cfg(windows)]
                let path = path
                    .strip_prefix('/')
                    .filter(|value| value.as_bytes().get(1) == Some(&b':'))
                    .unwrap_or(&path)
                    .to_owned();
                Some(path)
            } else {
                Some(line.to_owned())
            }
        })
        .collect()
}

fn decode_uri_path(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = std::str::from_utf8(bytes.get(index + 1..index + 3)?).ok()?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

impl PlaylistSort {
    fn descending(self) -> bool {
        matches!(self, Self::FilenameDescending | Self::PathDescending)
    }

    fn by_path(self) -> bool {
        matches!(self, Self::PathAscending | Self::PathDescending)
    }
}

pub fn sorted_ids(entries: &[PlaylistEntry], sort: PlaylistSort) -> Vec<i64> {
    let mut ordered = entries.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        let ordering = if sort.by_path() {
            left.filename.cmp(&right.filename)
        } else {
            display_name(left).cmp(&display_name(right))
        };
        if sort.descending() {
            ordering.reverse()
        } else {
            ordering
        }
    });
    ordered.into_iter().map(|entry| entry.id).collect()
}

fn display_name(entry: &PlaylistEntry) -> String {
    if let Some(title) = entry.title.as_deref() {
        return title.to_owned();
    }
    if entry.filename.contains("://") {
        return entry.filename.clone();
    }
    Path::new(&entry.filename).file_name().map_or_else(
        || entry.filename.clone(),
        |name| name.to_string_lossy().into_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::{PlaylistSort, sorted_ids};
    use crate::PlaylistEntry;

    fn entry(id: i64, filename: &str, title: Option<&str>) -> PlaylistEntry {
        PlaylistEntry {
            id,
            filename: filename.into(),
            title: title.map(str::to_owned),
            current: id == 2,
            playing: id == 2,
        }
    }

    fn entries() -> Vec<PlaylistEntry> {
        vec![
            entry(1, "D:/media/zeta.mkv", None),
            entry(2, "D:/media/alpha.mkv", Some("Zed title")),
            entry(3, "https://example.test/stream", None),
            entry(4, "C:/media/beta.mkv", Some("Alpha title")),
        ]
    }

    #[test]
    fn filename_sort_uses_display_titles_and_keeps_entry_identity() {
        assert_eq!(
            sorted_ids(&entries(), PlaylistSort::FilenameAscending),
            [4, 2, 3, 1]
        );
        assert_eq!(
            sorted_ids(&entries(), PlaylistSort::FilenameDescending),
            [1, 3, 2, 4]
        );
    }

    #[test]
    fn path_sort_uses_full_filenames_and_keeps_entry_identity() {
        assert_eq!(
            sorted_ids(&entries(), PlaylistSort::PathAscending),
            [4, 2, 1, 3]
        );
        assert_eq!(
            sorted_ids(&entries(), PlaylistSort::PathDescending),
            [3, 1, 2, 4]
        );
    }
}
