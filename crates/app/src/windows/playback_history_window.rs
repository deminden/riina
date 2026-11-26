mod availability;
mod confirmation;
mod layout;
mod open_request;
mod outline;
mod owner;
mod search;
mod selection;

pub(crate) use owner::{ApplicationHistory, Archive, HistorySync, Registration};

use crate::{PlaybackHistoryRow, PlaybackHistoryWindow};
use outline::{Projection, build_rows, path_from_reference};
use riina_core::playback_history::Entry;
use riina_ui::PlaybackHistoryAction;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::{
    collections::HashSet,
    rc::{Rc, Weak},
    sync::{Arc, Mutex},
};

type HistoryState = Arc<Mutex<crate::playback_history::PlaybackHistoryState>>;
type CollapsedGroups = Arc<Mutex<HashSet<String>>>;

#[derive(Clone)]
struct WindowConnection {
    archive: Archive,
    collapsed_groups: CollapsedGroups,
    owner: Weak<ApplicationHistory>,
    confirmation: Rc<confirmation::Session>,
    availability: availability::Availability,
}

pub(crate) fn report(window: &PlaybackHistoryWindow, message: String) {
    crate::diagnostics::warn!("{message}");
    window.set_status_text(message.into());
}

fn connect_window(window: &PlaybackHistoryWindow, connection: WindowConnection) {
    let selection_anchor = selection::Anchor::default();
    connect_search(window, &connection);
    search::install_recents(window, &connection.archive.preferences_path);
    selection::connect(window, &selection_anchor);
    connect_group_expansion(window, &connection);
    connect_context_actions(window, &connection);
    connect_group_collapse(window, &connection);
    connect_open_entry(window, &connection);
    confirmation::connect(window, &connection, selection_anchor);
}

fn connect_search(window: &PlaybackHistoryWindow, connection: &WindowConnection) {
    let weak_window = window.as_weak();
    let context = connection.clone();
    window.on_search_changed(move || {
        let Some(window) = weak_window.upgrade() else {
            return;
        };
        riina_core::sync::lock(&context.collapsed_groups).clear();
        context.refresh(&window);
    });
}

fn connect_group_expansion(window: &PlaybackHistoryWindow, connection: &WindowConnection) {
    let weak_window = window.as_weak();
    let context = connection.clone();
    window.on_toggle_group(move |group_key| {
        let Some(window) = weak_window.upgrade() else {
            return;
        };
        let group_key = group_key.to_string();
        {
            let mut collapsed = riina_core::sync::lock(&context.collapsed_groups);
            if !collapsed.remove(&group_key) {
                collapsed.insert(group_key);
            }
        }
        context.refresh(&window);
    });

    let weak_window = window.as_weak();
    let context = connection.clone();
    window.on_expand_all_groups(move || {
        let Some(window) = weak_window.upgrade() else {
            return;
        };
        riina_core::sync::lock(&context.collapsed_groups).clear();
        context.refresh(&window);
    });
}

fn connect_group_collapse(window: &PlaybackHistoryWindow, connection: &WindowConnection) {
    let weak_window = window.as_weak();
    let context = connection.clone();
    window.on_collapse_all_groups(move || {
        let Some(window) = weak_window.upgrade() else {
            return;
        };
        *riina_core::sync::lock(&context.collapsed_groups) = window
            .get_rows()
            .iter()
            .filter(|row| row.is_group)
            .map(|row| row.group_key.to_string())
            .collect();
        context.refresh(&window);
    });
}

fn connect_open_entry(window: &PlaybackHistoryWindow, connection: &WindowConnection) {
    let weak_window = window.as_weak();
    let context = connection.clone();
    window.on_open_entry(move |path| {
        let (Some(window), Some(owner)) = (weak_window.upgrade(), context.owner.upgrade()) else {
            return;
        };
        if window.get_confirmation_visible() {
            return;
        }
        let path = path.to_string();
        if !riina_core::sync::lock(&context.archive.history)
            .entries()
            .iter()
            .any(|entry| entry.path == path)
        {
            return;
        };
        if window
            .get_rows()
            .iter()
            .any(|row| row.path.as_str() == path && row.available)
        {
            owner.request_open(&window, vec![path], open_request::Intent::Single);
        }
    });
}

impl WindowConnection {
    fn refresh(&self, window: &PlaybackHistoryWindow) {
        refresh(
            window,
            &self.archive.history,
            &self.collapsed_groups,
            &self.availability,
        );
    }

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
    let active = window.get_selected_path();
    let mut first_selected = None;
    let mut active_selected = None;
    let mut playable = false;
    let mut local = false;
    for (index, mut row) in rows.iter().enumerate() {
        let selected = !row.is_group && paths.contains(row.path.as_str());
        if selected {
            first_selected.get_or_insert(index);
            if row.path == active {
                active_selected = Some(index);
            }
        }
        playable |= selected && row.available;
        local |= selected && row.local_file;
        if row.selected != selected {
            row.selected = selected;
            rows.set_row_data(index, row);
        }
    }
    window.set_has_playable_selection(playable);
    window.set_has_local_selection(local);
    window.set_selected_menu_row(
        active_selected
            .or(first_selected)
            .and_then(|index| i32::try_from(index).ok())
            .unwrap_or(-1),
    );
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

fn selected_available(
    window: &PlaybackHistoryWindow,
    paths: Vec<String>,
    predicate: impl Fn(&PlaybackHistoryRow) -> bool,
) -> Vec<String> {
    let available = window
        .get_rows()
        .iter()
        .filter(|row| !row.is_group && predicate(row))
        .map(|row| row.path.to_string())
        .collect::<HashSet<_>>();
    paths
        .into_iter()
        .filter(|path| available.contains(path.as_str()))
        .collect()
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

fn connect_context_actions(window: &PlaybackHistoryWindow, connection: &WindowConnection) {
    let weak_window = window.as_weak();
    let context = connection.clone();
    window.on_context_action(move |action, clicked_path| {
        let Some(window) = weak_window.upgrade() else {
            return;
        };
        if window.get_confirmation_visible() {
            return;
        }
        let paths = context_selected_paths(&window, clicked_path.as_str());
        match action {
            PlaybackHistoryAction::Play | PlaybackHistoryAction::PlayInNewWindow => {
                let playable = selected_available(&window, paths, |row| row.available);
                let destination = if action == PlaybackHistoryAction::Play {
                    open_request::Intent::Play
                } else {
                    open_request::Intent::PlayInNewWindow
                };
                if let Some(owner) = context.owner.upgrade() {
                    owner.request_open(&window, playable, destination);
                }
            }
            PlaybackHistoryAction::Reveal => reveal_history_files(
                &window,
                selected_available(&window, paths, |row| row.local_file),
            ),
            PlaybackHistoryAction::Remove => context.confirmation.remove(&window, paths),
            PlaybackHistoryAction::Trash => context.confirmation.trash(
                &window,
                selected_available(&window, paths, |row| row.local_file),
            ),
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

fn clear_history_selection(window: &PlaybackHistoryWindow, anchor: &selection::Anchor) {
    window.set_selected_path("".into());
    write_selected_paths(window, &HashSet::new());
    anchor.borrow_mut().take();
}

fn refresh(
    window: &PlaybackHistoryWindow,
    history: &HistoryState,
    collapsed_groups: &CollapsedGroups,
    availability: &availability::Availability,
) {
    let entries = riina_core::sync::lock(history).entries().to_vec();
    let selected = read_selected_paths(window);
    let Projection { rows, checks } = build_rows(
        &entries,
        window.get_search_text().as_str(),
        window.get_search_mode(),
        window.get_group_mode(),
        &collapsed_groups
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        &selected,
        &availability.snapshot(),
    );
    if window.window().is_visible() {
        availability.request(checks);
    }
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{build_rows, context_selection_in_row_order};
    use chrono::{Local, TimeZone};
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
        let missing_path = tree.path().join("Épisode 2.mkv");
        let groups = HashSet::new();
        let selected = HashSet::new();
        let pending = build_rows(
            &entries,
            "ÉPISODE ",
            1,
            1,
            &groups,
            &selected,
            &Default::default(),
        );
        assert_eq!(
            pending.checks,
            [available_path.clone(), missing_path.clone()]
        );
        assert!(pending.rows[1].available);
        assert!(!pending.rows[1].local_file);
        let files =
            super::availability::Files::from([(available_path, true), (missing_path, false)]);
        let rows = build_rows(&entries, "ÉPISODE ", 1, 1, &groups, &selected, &files).rows;
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

    #[test]
    fn day_groups_use_local_days_and_collapse_by_independent_keys() {
        let entry = |name: &str, day, hour| Entry {
            path: format!("https://example.test/{name}.mkv"),
            title: Some(name.into()),
            date_ms: Local
                .with_ymd_and_hms(2025, 1, day, hour, 10, 15)
                .single()
                .unwrap()
                .timestamp_millis() as u64,
            progress: None,
            duration: 0.0,
        };
        let entries = [
            entry("Morning", 9, 8),
            entry("Evening", 9, 20),
            entry("Next day", 10, 1),
        ];
        let selected = HashSet::from([entries[2].path.clone()]);
        let collapsed = HashSet::from(["day:2025-01-09".to_owned()]);
        let files = Default::default();
        let dates = build_rows(&entries, "", 0, 0, &collapsed, &selected, &files).rows;
        assert_eq!(dates.len(), 3);
        assert_eq!(dates[0].group_key, "day:2025-01-10");
        assert_eq!(dates[1].label, "Next day");
        assert!(dates[1].selected);
        assert_eq!(dates[2].group_key, "day:2025-01-09");
        assert!(dates[2].collapsed);

        let location = build_rows(&entries, "", 0, 1, &collapsed, &selected, &files).rows;
        assert_eq!(location[0].group_key, "location:example.test");
        assert_eq!(
            location[1..]
                .iter()
                .map(|row| row.label.as_str())
                .collect::<Vec<_>>(),
            ["Next day", "Evening", "Morning"],
        );
        assert_ne!(dates[1].played_at, location[1].played_at);
    }
}
