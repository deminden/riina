mod owner;
mod search;
mod selection;

pub(crate) use owner::{ApplicationHistory, Archive, HistorySync, PlayerDisplay, Registration};

use crate::{PlaybackHistoryRow, PlaybackHistoryWindow, media_open::HistoryDestination};
use chrono::{Local, TimeZone};
use rfd::{MessageButtons, MessageDialog, MessageDialogResult};
use riina_core::playback_history::Entry;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    path::PathBuf,
    rc::Weak,
    sync::{Arc, Mutex},
};

type HistoryState = Arc<Mutex<crate::playback_history::PlaybackHistoryState>>;
type CollapsedGroups = Arc<Mutex<HashSet<String>>>;

#[derive(Clone)]
struct WindowConnection {
    archive: Archive,
    collapsed_groups: CollapsedGroups,
    owner: Weak<ApplicationHistory>,
}

pub(crate) fn report(window: &PlaybackHistoryWindow, message: String) {
    crate::diagnostics::warn!("{message}");
    window.set_status_text(message.into());
}

fn connect_window(window: &PlaybackHistoryWindow, connection: WindowConnection) {
    let history = &connection.archive.history;
    let collapsed_groups = &connection.collapsed_groups;
    let selection_anchor = selection::Anchor::default();
    connect_search(window, history, collapsed_groups);
    search::install_recents(window, &connection.archive.preferences_path);
    selection::connect(window, &selection_anchor);
    connect_group_expansion(window, history, collapsed_groups);
    connect_context_actions(window, selection_anchor.clone(), &connection);
    connect_group_collapse(window, history, collapsed_groups);
    connect_open_entry(window, &connection);
    connect_clear_history(window, &connection, selection_anchor);
}

fn connect_search(
    window: &PlaybackHistoryWindow,
    history: &HistoryState,
    collapsed_groups: &CollapsedGroups,
) {
    let weak_window = window.as_weak();
    let observed_history = history.clone();
    let search_collapsed_groups = collapsed_groups.clone();
    window.on_search_changed(move || {
        let Some(window) = weak_window.upgrade() else {
            return;
        };
        riina_core::sync::lock(&search_collapsed_groups).clear();
        refresh(&window, &observed_history, &search_collapsed_groups);
    });
}

fn connect_group_expansion(
    window: &PlaybackHistoryWindow,
    history: &HistoryState,
    collapsed_groups: &CollapsedGroups,
) {
    let weak_window = window.as_weak();
    let groups_for_toggle = collapsed_groups.clone();
    let history_for_toggle = history.clone();
    window.on_toggle_group(move |label| {
        let Some(window) = weak_window.upgrade() else {
            return;
        };
        let label = label.to_string();
        {
            let mut collapsed = groups_for_toggle
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !collapsed.remove(&label) {
                collapsed.insert(label);
            }
        }
        refresh(&window, &history_for_toggle, &groups_for_toggle);
    });

    let weak_window = window.as_weak();
    let groups_for_expand = collapsed_groups.clone();
    let history_for_expand = history.clone();
    window.on_expand_all_groups(move || {
        let Some(window) = weak_window.upgrade() else {
            return;
        };
        groups_for_expand
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
        refresh(&window, &history_for_expand, &groups_for_expand);
    });
}

fn connect_group_collapse(
    window: &PlaybackHistoryWindow,
    history: &HistoryState,
    collapsed_groups: &CollapsedGroups,
) {
    let weak_window = window.as_weak();
    let groups_for_collapse = collapsed_groups.clone();
    let history_for_collapse = history.clone();
    window.on_collapse_all_groups(move || {
        let Some(window) = weak_window.upgrade() else {
            return;
        };
        let group_labels = {
            let history = riina_core::sync::lock(&history_for_collapse);
            build_rows(
                history.entries(),
                window.get_search_text().as_str(),
                window.get_search_mode(),
                window.get_group_mode(),
                &HashSet::new(),
                &HashSet::new(),
            )
            .into_iter()
            .filter(|row| row.is_group)
            .map(|row| row.label.to_string())
            .collect::<HashSet<_>>()
        };
        *groups_for_collapse
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = group_labels;
        refresh(&window, &history_for_collapse, &groups_for_collapse);
    });
}

fn connect_open_entry(window: &PlaybackHistoryWindow, connection: &WindowConnection) {
    let weak_window = window.as_weak();
    let context = connection.clone();
    window.on_open_entry(move |path| {
        let (Some(window), Some(owner)) = (weak_window.upgrade(), context.owner.upgrade()) else {
            return;
        };
        let path = path.to_string();
        let Some(position) = riina_core::sync::lock(&context.archive.history)
            .entries()
            .iter()
            .find(|entry| entry.path == path)
            .map(|entry| entry.progress.unwrap_or(0.0))
        else {
            return;
        };
        if describe(&path).available() {
            owner.open_single(&window, path, position);
        }
    });
}

fn connect_clear_history(
    window: &PlaybackHistoryWindow,
    connection: &WindowConnection,
    selection_anchor: selection::Anchor,
) {
    let weak_window = window.as_weak();
    let context = connection.clone();
    window.on_clear_history(move || {
        let Some(window) = weak_window.upgrade() else {
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
        let cleared = context
            .archive
            .history
            .lock()
            .map_err(|_| "playback history lock poisoned".to_owned())
            .and_then(|mut history| {
                let removed = crate::utilities_preferences::clear_playback_history(
                    &context.archive.preferences_path,
                )?;
                history.clear();
                context
                    .archive
                    .cleared
                    .store(true, std::sync::atomic::Ordering::Release);
                Ok((removed, history.entries().to_vec()))
            });
        match cleared {
            Ok((removed, entries)) => {
                let sync_error = context.sync_plugins(&entries).err();
                clear_history_selection(&window, &selection_anchor);
                context
                    .collapsed_groups
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clear();
                refresh(&window, &context.archive.history, &context.collapsed_groups);
                window.set_status_text(
                    match sync_error {
                        Some(error) => {
                            format!("History cleared, but plugins could not be updated: {error}")
                        }
                        None => format!("Playback history cleared ({removed} entries removed)"),
                    }
                    .into(),
                );
            }
            Err(error) => {
                report(
                    &window,
                    format!("Could not clear playback history: {error}"),
                );
            }
        }
    });
}

impl WindowConnection {
    fn sync_plugins(&self, entries: &[Entry]) -> Result<(), String> {
        self.owner
            .upgrade()
            .map_or(Ok(()), |owner| owner.sync_plugins(entries))
    }
}

fn read_selected_paths(window: &PlaybackHistoryWindow) -> HashSet<String> {
    window
        .get_selected_paths()
        .iter()
        .map(|path| path.to_string())
        .collect()
}

fn write_selected_paths(window: &PlaybackHistoryWindow, paths: &HashSet<String>) {
    let rows = window.get_rows();
    let mut playable = false;
    let mut local = false;
    for (index, mut row) in rows.iter().enumerate() {
        let selected = !row.is_group && paths.contains(row.path.as_str());
        playable |= selected && row.available;
        local |= selected && row.local_file;
        if row.selected != selected {
            row.selected = selected;
            rows.set_row_data(index, row);
        }
    }
    window.set_has_playable_selection(playable);
    window.set_has_local_selection(local);
    let mut paths = paths.iter().cloned().collect::<Vec<_>>();
    paths.sort();
    window.set_selected_paths(ModelRc::new(VecModel::from(
        paths
            .into_iter()
            .map(SharedString::from)
            .collect::<Vec<_>>(),
    )));
}

fn context_selected_paths(window: &PlaybackHistoryWindow, clicked_path: &str) -> Vec<String> {
    let rows = window.get_rows();
    let visible_paths = (0..rows.row_count())
        .filter_map(|index| rows.row_data(index))
        .filter(|row| !row.is_group)
        .map(|row| row.path.to_string())
        .collect::<Vec<_>>();
    context_selection_in_row_order(&visible_paths, &read_selected_paths(window), clicked_path)
}

fn context_selection_in_row_order(
    visible_paths: &[String],
    selected_paths: &HashSet<String>,
    clicked_path: &str,
) -> Vec<String> {
    let mut selected_paths = selected_paths.clone();
    if !clicked_path.is_empty() {
        if !visible_paths.iter().any(|path| path == clicked_path) {
            return Vec::new();
        }
        selected_paths.insert(clicked_path.to_owned());
    }
    visible_paths
        .iter()
        .filter(|path| selected_paths.contains(*path))
        .cloned()
        .collect()
}

fn connect_context_actions(
    window: &PlaybackHistoryWindow,
    selection_anchor: selection::Anchor,
    connection: &WindowConnection,
) {
    let weak_window = window.as_weak();
    let context = connection.clone();
    window.on_context_action(move |action, clicked_path| {
        let Some(window) = weak_window.upgrade() else {
            return;
        };
        let paths = context_selected_paths(&window, clicked_path.as_str());
        match action {
            0 | 1 => {
                let playable = paths
                    .into_iter()
                    .filter(|path| describe(path).available())
                    .collect::<Vec<_>>();
                let destination = if action == 0 {
                    HistoryDestination::CurrentPlayer
                } else {
                    HistoryDestination::NewPlayer
                };
                if let Some(owner) = context.owner.upgrade() {
                    owner.open_selection(&window, playable, destination);
                }
            }
            2 => reveal_history_files(&window, paths),
            3 => remove_history_entries(&window, paths, &selection_anchor, &context),
            4 => move_history_files_to_trash(&window, paths, &context),
            _ => {}
        }
    });
}

fn reveal_history_files(window: &PlaybackHistoryWindow, paths: Vec<String>) {
    if paths.is_empty() {
        return;
    }
    let weak = window.as_weak();
    if let Err(error) = crate::background::spawn_named("riina-history-reveal", move || {
        let files = paths
            .iter()
            .filter_map(|path| path_from_reference(path))
            .filter(|path| path.is_file())
            .collect::<Vec<_>>();
        if let Err(error) = crate::platform::reveal_files(&files) {
            let _ = crate::background::to_ui(weak, move |window| {
                report(&window, format!("Could not reveal history files: {error}"));
            });
        }
    }) {
        report(window, format!("Could not show history files: {error}"));
    }
}

fn remove_history_entries(
    window: &PlaybackHistoryWindow,
    paths: Vec<String>,
    selection_anchor: &selection::Anchor,
    connection: &WindowConnection,
) {
    if paths.is_empty() {
        return;
    }
    let response = MessageDialog::new()
        .set_title("Remove from Playback History")
        .set_description(if paths.len() == 1 {
            "Remove this item from the playback history?".to_owned()
        } else {
            format!("Remove {} items from the playback history?", paths.len())
        })
        .set_buttons(MessageButtons::YesNo)
        .show();
    if response != MessageDialogResult::Yes {
        return;
    }
    let result = connection
        .archive
        .history
        .lock()
        .map_err(|_| "playback history lock poisoned".to_owned())
        .and_then(|mut history| {
            let removed = history.remove_paths(paths.iter().map(String::as_str))?;
            let sync_error = if removed > 0 {
                connection
                    .archive
                    .cleared
                    .store(true, std::sync::atomic::Ordering::Release);
                let entries = history.entries().to_vec();
                drop(history);
                connection.sync_plugins(&entries).err()
            } else {
                None
            };
            Ok::<_, String>((removed, sync_error))
        });
    match result {
        Ok((removed, sync_error)) if removed > 0 => {
            clear_history_selection(window, selection_anchor);
            refresh(
                window,
                &connection.archive.history,
                &connection.collapsed_groups,
            );
            if let Some(error) = sync_error {
                report(
                    window,
                    format!("History was removed, but plugins could not be updated: {error}"),
                );
            }
        }
        Ok((_, _)) => window.set_status_text("The history items were already removed".into()),
        Err(error) => report(window, format!("Could not remove history items: {error}")),
    }
}

fn move_history_files_to_trash(
    window: &PlaybackHistoryWindow,
    paths: Vec<String>,
    connection: &WindowConnection,
) {
    let files = paths
        .iter()
        .filter_map(|reference| path_from_reference(reference))
        .filter(|path| path.is_file())
        .collect::<BTreeSet<_>>();
    if files.is_empty() {
        window.set_status_text("Only available local files can be moved to the system bin".into());
        return;
    }
    let trash_name = crate::platform::system_trash_name();
    let description = match files.first().filter(|_| files.len() == 1) {
        Some(file) => format!("Move {} to the {trash_name}?", file.display()),
        None => format!("Move {} files to the {trash_name}?", files.len()),
    };
    if MessageDialog::new()
        .set_title("Move Files to Trash")
        .set_description(description)
        .set_buttons(MessageButtons::YesNo)
        .show()
        != MessageDialogResult::Yes
    {
        return;
    }
    let weak_window = window.as_weak();
    let trash_history = connection.archive.history.clone();
    let trash_groups = connection.collapsed_groups.clone();
    let task = crate::background::spawn_named("riina-history-trash-selection", move || {
        let count = files.len();
        let moved = files
            .into_iter()
            .filter(|path| path.is_file() && trash::delete(path).is_ok())
            .count();
        if moved > 0 {
            riina_core::sync::lock(&trash_history).files_changed();
        }
        let _ = crate::background::to_ui(weak_window, move |window| {
            refresh(&window, &trash_history, &trash_groups);
            if moved == count {
                window.set_status_text(format!("Moved {moved} files to the {trash_name}").into());
            } else {
                report(
                    &window,
                    format!(
                        "Moved {moved} files to the {trash_name}. Could not move {} files.",
                        count - moved,
                    ),
                );
            }
        });
    });
    if let Err(error) = task {
        report(
            window,
            format!("Could not start the playback history trash worker: {error}"),
        );
    }
}

fn clear_history_selection(window: &PlaybackHistoryWindow, anchor: &selection::Anchor) {
    window.set_selected_path("".into());
    write_selected_paths(window, &HashSet::new());
    anchor.borrow_mut().take();
}

fn refresh(
    window: &PlaybackHistoryWindow,
    history: &HistoryState,
    collapsed_groups: &CollapsedGroups,
) {
    let entries = riina_core::sync::lock(history).entries().to_vec();
    let selected = read_selected_paths(window);
    let rows = build_rows(
        &entries,
        window.get_search_text().as_str(),
        window.get_search_mode(),
        window.get_group_mode(),
        &collapsed_groups
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        &selected,
    );
    let visible_paths = rows
        .iter()
        .filter(|row| !row.is_group)
        .map(|row| row.path.to_string())
        .collect::<HashSet<_>>();
    let selected = selected
        .into_iter()
        .filter(|path| visible_paths.contains(path))
        .collect::<HashSet<_>>();
    let selected_path = window.get_selected_path();
    if !rows
        .iter()
        .any(|row| !row.is_group && row.path == selected_path)
    {
        window.set_selected_path("".into());
    }
    window.set_has_collapsed_groups(rows.iter().any(|row| row.is_group && row.collapsed));
    window.set_has_expanded_groups(rows.iter().any(|row| row.is_group && !row.collapsed));
    window.set_rows(ModelRc::new(VecModel::from(rows)));
    write_selected_paths(window, &selected);
}

fn revision(history: &HistoryState) -> u64 {
    history.lock().map_or(0, |history| history.revision())
}

fn build_rows(
    entries: &[Entry],
    query: &str,
    search_mode: i32,
    group_mode: i32,
    collapsed_groups: &HashSet<String>,
    selected_paths: &HashSet<String>,
) -> Vec<PlaybackHistoryRow> {
    let query = search::Query::new(query);
    let mut groups = BTreeMap::<String, Vec<(&Entry, MediaDescription)>>::new();
    for entry in entries {
        let description = describe(&entry.path);
        let matches = match search_mode {
            0 => query.matches(&description.name),
            _ => query.matches_path(&entry.path, description.local.as_deref()),
        };
        if !matches {
            continue;
        }
        let key = if group_mode == 1 {
            description.location.clone()
        } else {
            date_string(entry.date_ms)
        };
        groups.entry(key).or_default().push((entry, description));
    }
    let mut buckets = groups.into_iter().collect::<Vec<_>>();
    if group_mode == 0 {
        buckets.sort_by(|(left_key, left), (right_key, right)| {
            right
                .iter()
                .map(|(entry, _)| entry.date_ms)
                .max()
                .cmp(&left.iter().map(|(entry, _)| entry.date_ms).max())
                .then_with(|| left_key.cmp(right_key))
        });
    } else {
        buckets.sort_by(|(left, _), (right, _)| {
            crate::platform::natural_compare(left, right).then_with(|| left.cmp(right))
        });
    }

    let mut rows = Vec::new();
    for (group, mut entries) in buckets {
        entries.sort_by_key(|(entry, _)| std::cmp::Reverse(entry.date_ms));
        let collapsed = collapsed_groups.contains(&group);
        rows.push(PlaybackHistoryRow {
            is_group: true,
            collapsed,
            label: group.clone().into(),
            detail: "".into(),
            progress: "".into(),
            played_at: "".into(),
            path: "".into(),
            progress_ratio: 0.0,
            available: false,
            local_file: false,
            selected: false,
        });
        if collapsed {
            continue;
        }
        for (entry, description) in entries {
            let available = description.available();
            let fallback = if description.local.is_some() {
                &description.name
            } else {
                &entry.path
            };
            let progress = entry.progress.filter(|_| available);
            rows.push(PlaybackHistoryRow {
                is_group: false,
                collapsed: false,
                label: entry.title.as_ref().unwrap_or(fallback).as_str().into(),
                detail: entry
                    .title
                    .as_ref()
                    .map_or("", |_| fallback.as_str())
                    .into(),
                progress: progress_label(progress, entry.duration).into(),
                played_at: played_at(entry.date_ms, group_mode == 1).into(),
                path: entry.path.clone().into(),
                progress_ratio: progress_ratio(progress, entry.duration),
                available,
                local_file: description.local.is_some() && available,
                selected: selected_paths.contains(entry.path.as_str()),
            });
        }
    }
    rows
}

struct MediaDescription {
    name: String,
    location: String,
    local: Option<PathBuf>,
}

impl MediaDescription {
    fn available(&self) -> bool {
        self.local.as_ref().is_none_or(|path| path.is_file())
    }
}

fn describe(reference: &str) -> MediaDescription {
    if let Some(path) = path_from_reference(reference) {
        let name = path.file_name().map_or_else(
            || reference.to_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        let location = path.parent().map_or_else(
            || "".to_owned(),
            |parent| parent.to_string_lossy().into_owned(),
        );
        return MediaDescription {
            name,
            location,
            local: Some(path),
        };
    }
    if let Ok(url) = url::Url::parse(reference) {
        let location = url.host_str().unwrap_or("-").to_owned();
        let name = url
            .path_segments()
            .and_then(|mut segments| segments.next_back())
            .filter(|name| !name.is_empty())
            .map_or_else(
                || reference.to_owned(),
                |name| {
                    percent_encoding::percent_decode_str(name)
                        .decode_utf8_lossy()
                        .into_owned()
                },
            );
        return MediaDescription {
            name,
            location,
            local: None,
        };
    }
    MediaDescription {
        name: reference.to_owned(),
        location: String::new(),
        local: Some(PathBuf::from(reference)),
    }
}

fn path_from_reference(reference: &str) -> Option<PathBuf> {
    let is_windows_drive = reference.as_bytes().get(1) == Some(&b':')
        && reference
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic);
    if is_windows_drive {
        return Some(PathBuf::from(reference));
    }
    match url::Url::parse(reference) {
        Ok(url) if url.scheme() == "file" => url.to_file_path().ok(),
        Ok(_) => None,
        Err(_) => Some(PathBuf::from(reference)),
    }
}

fn date_string(date_ms: u64) -> String {
    Local
        .timestamp_millis_opt(date_ms.min(i64::MAX as u64) as i64)
        .single()
        .map_or_else(
            || "Unknown Date".to_owned(),
            |date| date.format("%Y-%m-%d").to_string(),
        )
}

fn played_at(date_ms: u64, include_date: bool) -> String {
    Local
        .timestamp_millis_opt(date_ms.min(i64::MAX as u64) as i64)
        .single()
        .map_or_else(String::new, |date| {
            if include_date {
                date.format("%Y-%m-%d %H:%M").to_string()
            } else {
                date.format("%H:%M").to_string()
            }
        })
}

fn progress_ratio(progress: Option<f64>, duration: f64) -> f32 {
    progress
        .filter(|_| duration.is_finite() && duration > 0.0)
        .map_or(0.0, |progress| (progress / duration).clamp(0.0, 1.0) as f32)
}

fn progress_label(progress: Option<f64>, duration: f64) -> String {
    let Some(progress) = progress.filter(|_| duration.is_finite() && duration > 0.0) else {
        return String::new();
    };
    format!("{} / {}", format_time(progress), format_time(duration))
}

fn format_time(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return String::new();
    }
    let total = seconds.floor().min(u64::MAX as f64) as u64;
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{build_rows, context_selection_in_row_order};
    use riina_core::playback_history::Entry;

    #[test]
    fn context_actions_keep_selected_rows_in_outline_order_and_add_the_clicked_row() {
        let visible = vec![
            "first.mkv".to_owned(),
            "second.mkv".to_owned(),
            "third.mkv".to_owned(),
        ];
        let selected = HashSet::from(["third.mkv".to_owned(), "first.mkv".to_owned()]);

        assert_eq!(
            context_selection_in_row_order(&visible, &selected, "second.mkv"),
            ["first.mkv", "second.mkv", "third.mkv"]
        );
    }

    #[test]
    fn context_actions_ignore_selected_rows_hidden_from_the_current_outline() {
        let visible = vec!["shown.mkv".to_owned()];
        let selected = HashSet::from(["filtered-out.mkv".to_owned()]);

        assert_eq!(
            context_selection_in_row_order(&visible, &selected, "shown.mkv"),
            ["shown.mkv"]
        );
        assert_eq!(
            context_selection_in_row_order(&visible, &selected, ""),
            [] as [String; 0]
        );
        assert!(context_selection_in_row_order(&visible, &selected, "removed.mkv").is_empty());
    }

    #[test]
    fn history_shows_titles_and_decoded_paths_and_hides_missing_file_progress() {
        let tree = riina_test_support::TempDir::new("history-projection").unwrap();
        let available_path = tree.path().join("Épisode 1.mkv");
        std::fs::write(&available_path, []).unwrap();
        let make_entry = |path: &std::path::Path| Entry {
            path: url::Url::from_file_path(path).unwrap().into(),
            title: Some("Chroniques de Montréal".into()),
            progress: Some(12.0),
            duration: 60.0,
            date_ms: 0,
        };
        let entries = [
            make_entry(&available_path),
            make_entry(&tree.path().join("Épisode 2.mkv")),
        ];
        let rows = build_rows(&entries, "ÉPISODE ", 1, 1, &HashSet::new(), &HashSet::new());
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1].label, "Chroniques de Montréal");
        assert_eq!(rows[1].detail, "Épisode 1.mkv");
        assert_eq!(rows[1].progress, "0:12 / 1:00");
        assert!(rows[1].available);
        assert_eq!(rows[2].detail, "Épisode 2.mkv");
        assert!(rows[2].progress.is_empty());
        assert!(!rows[2].local_file);
        assert!(!rows[2].available);
    }
}
