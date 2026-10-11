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

/// Convert a drop insertion slot into the resulting row index after removing its source.
pub fn drop_destination(source: usize, insertion_slot: usize, row_count: usize) -> Option<usize> {
    if row_count == 0 || source >= row_count || insertion_slot > row_count {
        return None;
    }
    Some(insertion_slot.saturating_sub(usize::from(insertion_slot > source)))
}

/// mpv's playlist-move destination counts the source row until the move completes.
pub fn move_command_target(source: usize, destination: usize) -> usize {
    destination + usize::from(destination > source)
}

/// Plan stable, ordered moves for a selection dropped at a playlist insertion slot.
pub fn reorder_selection(
    entries: &[i64],
    selected: &[i64],
    insertion_slot: usize,
) -> Option<Vec<(usize, usize)>> {
    if insertion_slot > entries.len() {
        return None;
    }

    let mut selected_entries = Vec::with_capacity(selected.len());
    for id in entries {
        if selected.contains(id) && !selected_entries.contains(id) {
            selected_entries.push(*id);
        }
    }
    if selected_entries.is_empty() || selected_entries.len() != selected.len() {
        return None;
    }

    let removed_before_slot = entries[..insertion_slot]
        .iter()
        .filter(|id| selected_entries.contains(id))
        .count();
    let destination = insertion_slot - removed_before_slot;
    let mut desired = entries
        .iter()
        .copied()
        .filter(|id| !selected_entries.contains(id))
        .collect::<Vec<_>>();
    desired.splice(destination..destination, selected_entries);

    let mut current = entries.to_vec();
    let mut moves = Vec::new();
    for (destination, id) in desired.into_iter().enumerate() {
        let source = current.iter().position(|entry| *entry == id)?;
        if source == destination {
            continue;
        }
        moves.push((source, move_command_target(source, destination)));
        let entry = current.remove(source);
        current.insert(destination, entry);
    }
    Some(moves)
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
    use super::{
        PlaylistSort, drop_destination, move_command_target, reorder_selection, sorted_ids,
    };
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

    #[test]
    fn drop_slots_resolve_to_final_indices_after_source_removal() {
        assert_eq!(drop_destination(0, 3, 4), Some(2));
        assert_eq!(drop_destination(3, 0, 4), Some(0));
        assert_eq!(drop_destination(1, 4, 4), Some(3));
        assert_eq!(drop_destination(2, 2, 4), Some(2));
        assert_eq!(drop_destination(2, 3, 4), Some(2));
    }

    #[test]
    fn drop_slots_reject_out_of_range_rows_and_slots() {
        assert_eq!(drop_destination(0, 0, 0), None);
        assert_eq!(drop_destination(4, 0, 4), None);
        assert_eq!(drop_destination(0, 5, 4), None);
    }

    #[test]
    fn move_command_target_accounts_for_the_source_row() {
        assert_eq!(move_command_target(0, 2), 3);
        assert_eq!(move_command_target(3, 0), 0);
        assert_eq!(move_command_target(2, 2), 2);
    }

    #[test]
    fn multi_selection_reorder_preserves_order_and_drop_slot() {
        let entries = [10, 20, 30, 40, 50, 60];
        let moves = reorder_selection(&entries, &[20, 40], 5).unwrap();
        let mut reordered = entries.to_vec();
        for (from, command_target) in moves {
            let destination = command_target - usize::from(command_target > from);
            let entry = reordered.remove(from);
            reordered.insert(destination, entry);
        }
        assert_eq!(reordered, [10, 30, 50, 20, 40, 60]);
    }

    #[test]
    fn multi_selection_reorder_matches_stable_drop_order_for_small_playlists() {
        for length in 1..=6 {
            let entries = (0..length as i64).collect::<Vec<_>>();
            for mask in 1..(1 << length) {
                let selected = entries
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| mask & (1 << index) != 0)
                    .map(|(_, id)| *id)
                    .collect::<Vec<_>>();
                for insertion_slot in 0..=length {
                    let moves = reorder_selection(&entries, &selected, insertion_slot).unwrap();
                    let mut actual = entries.clone();
                    for (from, command_target) in moves {
                        let destination = command_target - usize::from(command_target > from);
                        let entry = actual.remove(from);
                        actual.insert(destination, entry);
                    }

                    let mut expected = entries
                        .iter()
                        .copied()
                        .filter(|id| !selected.contains(id))
                        .collect::<Vec<_>>();
                    let destination = insertion_slot
                        - entries[..insertion_slot]
                            .iter()
                            .filter(|id| selected.contains(id))
                            .count();
                    expected.splice(destination..destination, selected.iter().copied());
                    assert_eq!(
                        actual, expected,
                        "length={length}, mask={mask}, slot={insertion_slot}"
                    );
                }
            }
        }
    }

    #[test]
    fn multi_selection_drop_inside_selection_is_a_noop() {
        assert_eq!(reorder_selection(&[1, 2, 3, 4], &[2, 3], 2), Some(vec![]));
    }

    #[test]
    fn multi_selection_rejects_missing_or_duplicate_identity() {
        assert_eq!(reorder_selection(&[1, 2], &[3], 0), None);
        assert_eq!(reorder_selection(&[1, 2], &[1, 1, 2], 0), None);
        assert_eq!(reorder_selection(&[1, 2], &[1], 3), None);
    }
}
