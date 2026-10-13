use crate::{PlaybackHistoryRow, PlaybackHistoryWindow, PlayerWindow};
use chrono::{Local, TimeZone};
use rfd::{MessageButtons, MessageDialog, MessageDialogResult};
use riina_core::{RecentMedia, playback_history::Entry};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashSet},
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicI32, Ordering},
    },
    time::Duration,
};

type HistoryState = Arc<Mutex<crate::plugin_host::PlaybackHistoryState>>;
type HistoryWindowSlot = Rc<RefCell<Option<PlaybackHistoryWindow>>>;
type CollapsedGroups = Arc<Mutex<HashSet<String>>>;
pub(crate) type HistorySync =
    Arc<dyn Fn(&mut crate::plugin_host::PlaybackHistoryState) -> Result<(), String> + Send + Sync>;

struct WindowConnection {
    history: HistoryState,
    player: riina_mpv::CommandSender,
    preferences_path: PathBuf,
    history_cleared: Arc<std::sync::atomic::AtomicBool>,
    history_sync: HistorySync,
    collapsed_groups: CollapsedGroups,
}

pub(crate) fn install(
    app: &PlayerWindow,
    playback_history: HistoryState,
    player: riina_mpv::CommandSender,
    preferences_path: PathBuf,
    history_cleared: Arc<std::sync::atomic::AtomicBool>,
    history_sync: HistorySync,
) -> (HistoryWindowSlot, Rc<slint::Timer>) {
    let slot = Rc::new(RefCell::new(None::<PlaybackHistoryWindow>));
    let last_revision = Rc::new(Cell::new(None::<u64>));
    let collapsed_groups = Arc::new(Mutex::new(HashSet::new()));
    let weak_app = app.as_weak();
    let show_slot = slot.clone();
    let show_history = playback_history.clone();
    let show_last_revision = last_revision.clone();
    let show_player = player.clone();
    let show_preferences_path = preferences_path.clone();
    let show_history_cleared = history_cleared.clone();
    let show_history_sync = history_sync.clone();
    let show_collapsed_groups = collapsed_groups.clone();

    app.on_show_playback_history(move || {
        let Some(app) = weak_app.upgrade() else {
            return;
        };
        let window = if let Some(window) = show_slot.borrow().as_ref() {
            window.clone_strong()
        } else {
            let window = match PlaybackHistoryWindow::new() {
                Ok(window) => window,
                Err(error) => {
                    app.set_status(format!("Could not open Playback History: {error}").into());
                    return;
                }
            };
            connect_window(
                &app,
                &window,
                WindowConnection {
                    history: show_history.clone(),
                    player: show_player.clone(),
                    preferences_path: show_preferences_path.clone(),
                    history_cleared: show_history_cleared.clone(),
                    history_sync: show_history_sync.clone(),
                    collapsed_groups: show_collapsed_groups.clone(),
                },
            );
            window
                .window()
                .on_close_requested(|| slint::CloseRequestResponse::HideWindow);
            crate::platform::center_auxiliary_window(&window, &app);
            *show_slot.borrow_mut() = Some(window.clone_strong());
            window
        };
        refresh(&window, &show_history, &show_collapsed_groups);
        show_last_revision.set(Some(revision(&show_history)));
        if let Err(error) = window.show() {
            app.set_status(format!("Could not show Playback History: {error}").into());
        }
    });

    let timer = Rc::new(slint::Timer::default());
    let timer_slot = slot.clone();
    let timer_history = playback_history;
    let timer_revision = last_revision;
    let timer_collapsed_groups = collapsed_groups;
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(500),
        move || {
            let Some(window) = timer_slot
                .borrow()
                .as_ref()
                .map(|window| window.clone_strong())
            else {
                return;
            };
            if !window.window().is_visible() {
                return;
            }
            let current_revision = revision(&timer_history);
            if timer_revision.get() == Some(current_revision) {
                return;
            }
            refresh(&window, &timer_history, &timer_collapsed_groups);
            timer_revision.set(Some(current_revision));
        },
    );
    (slot, timer)
}

fn connect_window(
    app: &PlayerWindow,
    window: &PlaybackHistoryWindow,
    connection: WindowConnection,
) {
    let WindowConnection {
        history,
        player,
        preferences_path,
        history_cleared,
        history_sync,
        collapsed_groups,
    } = connection;
    let weak_window = window.as_weak();
    let observed_history = history.clone();
    let search_collapsed_groups = collapsed_groups.clone();
    window.on_search_changed(move |_query, _group_mode| {
        let Some(window) = weak_window.upgrade() else {
            return;
        };
        refresh(&window, &observed_history, &search_collapsed_groups);
    });

    let weak_window = window.as_weak();
    let selection_anchor = Arc::new(AtomicI32::new(-1_i32));
    let anchor_for_selection = selection_anchor.clone();
    let history_for_selection = history.clone();
    let groups_for_selection = collapsed_groups.clone();
    window.on_select_entry(move |path, index, extend, range| {
        let Some(window) = weak_window.upgrade() else {
            return;
        };
        let path = path.to_string();
        let mut selected = read_selected_paths(&window);
        if range {
            let anchor = anchor_for_selection.load(Ordering::Acquire);
            if !extend {
                selected.clear();
            }
            if anchor >= 0 {
                let rows = window.get_rows();
                let first = anchor.min(index).max(0) as usize;
                let last = anchor.max(index).min(rows.row_count() as i32 - 1).max(0) as usize;
                for row_index in first..=last {
                    if let Some(row) = rows.row_data(row_index)
                        && !row.is_group
                    {
                        selected.insert(row.path.to_string());
                    }
                }
            } else {
                selected.insert(path.clone());
                anchor_for_selection.store(index, Ordering::Release);
            }
        } else if extend {
            if !selected.remove(&path) {
                selected.insert(path.clone());
            }
            anchor_for_selection.store(index, Ordering::Release);
        } else {
            selected.clear();
            selected.insert(path.clone());
            anchor_for_selection.store(index, Ordering::Release);
        }
        window.set_selected_path(path.into());
        write_selected_paths(&window, &selected);
        refresh(&window, &history_for_selection, &groups_for_selection);
    });

    let weak_window = window.as_weak();
    let history_for_keys = history.clone();
    let groups_for_keys = collapsed_groups.clone();
    let anchor_for_keys = selection_anchor.clone();
    window.on_history_key(move |action| {
        let Some(window) = weak_window.upgrade() else {
            return;
        };
        if action == 0 {
            let rows = window.get_rows();
            let selected = (0..rows.row_count())
                .filter_map(|index| rows.row_data(index).map(|row| (index, row)))
                .filter(|(_, row)| !row.is_group)
                .map(|(_, row)| row.path.to_string())
                .collect::<Vec<_>>();
            let first_index = (0..rows.row_count())
                .find(|index| rows.row_data(*index).is_some_and(|row| !row.is_group));
            window.set_selected_path(selected.first().cloned().unwrap_or_default().into());
            write_selected_paths(&window, &selected.iter().cloned().collect());
            anchor_for_keys.store(
                first_index.map_or(-1, |index| index as i32),
                Ordering::Release,
            );
            refresh(&window, &history_for_keys, &groups_for_keys);
            return;
        }
        let context_action = match action {
            1 => 0,
            2 => 1,
            3 => 2,
            4 => 3,
            _ => return,
        };
        window.invoke_context_action(context_action, SharedString::default());
    });

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

    connect_context_actions(
        app,
        window,
        selection_anchor.clone(),
        history.clone(),
        player.clone(),
        history_cleared.clone(),
        history_sync.clone(),
        collapsed_groups.clone(),
    );

    let weak_window = window.as_weak();
    let groups_for_collapse = collapsed_groups.clone();
    let history_for_collapse = history.clone();
    window.on_collapse_all_groups(move || {
        let Some(window) = weak_window.upgrade() else {
            return;
        };
        let group_labels = history_for_collapse
            .lock()
            .ok()
            .map(|history| {
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
            })
            .unwrap_or_default();
        *groups_for_collapse
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = group_labels;
        refresh(&window, &history_for_collapse, &groups_for_collapse);
    });

    let weak_app = app.as_weak();
    let history_for_open = history.clone();
    let player_for_open = player.clone();
    window.on_open_entry(move |path| {
        let Some(app) = weak_app.upgrade() else {
            return;
        };
        let path = path.to_string();
        let position = history_for_open
            .lock()
            .ok()
            .and_then(|history| {
                history
                    .entries()
                    .iter()
                    .find(|entry| entry.path == path)
                    .and_then(|entry| entry.progress)
            })
            .unwrap_or(0.0);
        media_open_history_entry(&app, path, position, player_for_open.clone());
    });

    let weak_app = app.as_weak();
    let weak_window = window.as_weak();
    let history_for_clear = history.clone();
    let groups_for_clear = collapsed_groups.clone();
    let history_cleared_for_clear = history_cleared.clone();
    let history_sync_for_clear = history_sync;
    window.on_clear_history(move || {
        let (Some(app), Some(window)) = (weak_app.upgrade(), weak_window.upgrade()) else {
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
        match crate::utilities_preferences::clear_playback_history(&preferences_path) {
            Ok(removed) => {
                if let Ok(mut history) = history_for_clear.lock() {
                    history.clear();
                    history_cleared_for_clear.store(true, std::sync::atomic::Ordering::Release);
                    if let Err(error) = history_sync_for_clear(&mut history) {
                        app.set_status(
                            format!("History cleared, but plugins could not be updated: {error}")
                                .into(),
                        );
                    }
                }
                window.set_selected_path("".into());
                groups_for_clear
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clear();
                refresh(&window, &history_for_clear, &groups_for_clear);
                app.set_status(
                    format!("Playback history cleared ({removed} entries removed)").into(),
                );
            }
            Err(error) => {
                app.set_status(format!("Could not clear playback history: {error}").into());
            }
        }
    });
}

fn read_selected_paths(window: &PlaybackHistoryWindow) -> HashSet<String> {
    window
        .get_selected_paths()
        .iter()
        .map(|path| path.to_string())
        .collect()
}

fn write_selected_paths(window: &PlaybackHistoryWindow, paths: &HashSet<String>) {
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
        selected_paths.insert(clicked_path.to_owned());
    }
    let paths = visible_paths
        .iter()
        .filter(|path| selected_paths.contains(*path))
        .cloned()
        .collect::<Vec<_>>();
    if paths.is_empty() && !clicked_path.is_empty() {
        vec![clicked_path.to_owned()]
    } else {
        paths
    }
}

#[allow(clippy::too_many_arguments)]
fn connect_context_actions(
    app: &PlayerWindow,
    window: &PlaybackHistoryWindow,
    selection_anchor: Arc<AtomicI32>,
    history: HistoryState,
    player: riina_mpv::CommandSender,
    history_cleared: Arc<std::sync::atomic::AtomicBool>,
    history_sync: HistorySync,
    collapsed_groups: CollapsedGroups,
) {
    let weak_app = app.as_weak();
    let weak_window = window.as_weak();
    let action_history = history.clone();
    let action_player = player.clone();
    let action_cleared = history_cleared.clone();
    let action_sync = history_sync.clone();
    let action_groups = collapsed_groups.clone();
    window.on_context_action(move |action, clicked_path| {
        let (Some(app), Some(window)) = (weak_app.upgrade(), weak_window.upgrade()) else {
            return;
        };
        let paths = context_selected_paths(&window, clicked_path.as_str());
        match action {
            0 => {
                let playable = paths
                    .into_iter()
                    .filter(|path| describe(path).4)
                    .collect::<Vec<_>>();
                crate::media_open::selection(&app, playable, false, action_player.clone());
            }
            1 => {
                let playable = paths
                    .into_iter()
                    .filter(|path| describe(path).4)
                    .collect::<Vec<_>>();
                crate::media_open::selection_in_new_player(&app, playable);
            }
            2 => {
                let files = paths
                    .iter()
                    .filter_map(|reference| path_from_reference(reference))
                    .filter(|path| path.is_file())
                    .collect::<Vec<_>>();
                for path in files {
                    if let Err(error) = crate::platform::reveal_file(&path) {
                        app.set_status(format!("Could not reveal a history file: {error}").into());
                        return;
                    }
                }
            }
            3 => {
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
                let result = action_history
                    .lock()
                    .map_err(|_| "playback history lock poisoned".to_owned())
                    .and_then(|mut history| {
                        let mut removed = 0;
                        for path in &paths {
                            if history.remove_path(path)? {
                                removed += 1;
                            }
                        }
                        let sync_error = if removed > 0 {
                            action_cleared.store(true, std::sync::atomic::Ordering::Release);
                            action_sync(&mut history).err()
                        } else {
                            None
                        };
                        Ok::<_, String>((removed, sync_error))
                    });
                match result {
                    Ok((removed, sync_error)) if removed > 0 => {
                        clear_history_selection(&window, &selection_anchor);
                        refresh(&window, &action_history, &action_groups);
                        if let Some(error) = sync_error {
                            app.set_status(format!("History was removed, but plugins could not be updated: {error}").into());
                        }
                    }
                    Ok((_, _)) => app.set_status("The history items were already removed".into()),
                    Err(error) => app.set_status(format!("Could not remove history items: {error}").into()),
                }
            }
            4 => {
                let files = paths
                    .iter()
                    .filter_map(|reference| {
                        path_from_reference(reference)
                            .filter(|path| path.is_file())
                            .map(|path| (reference.clone(), path))
                    })
                    .collect::<Vec<_>>();
                if files.is_empty() {
                    app.set_status("Only available local files can be moved to the system bin".into());
                    return;
                }
                let trash_name = crate::platform::system_trash_name();
                let description = if files.len() == 1 {
                    format!("Move {} to the system bin?", files[0].1.display())
                } else {
                    format!("Move {} files to the {trash_name}?", files.len())
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
                let weak_app = weak_app.clone();
                let weak_window = weak_window.clone();
                let trash_history = action_history.clone();
                let trash_cleared = action_cleared.clone();
                let trash_sync = action_sync.clone();
                let trash_groups = action_groups.clone();
                let trash_anchor = selection_anchor.clone();
                let _ = std::thread::Builder::new()
                    .name("riina-history-trash-selection".into())
                    .spawn(move || {
                        let mut removed_references = Vec::new();
                        let mut failures = Vec::new();
                        for (reference, path) in files {
                            match trash::delete(&path) {
                                Ok(()) => removed_references.push(reference),
                                Err(error) => failures.push(error.to_string()),
                            }
                        }
                        let history_result = trash_history
                            .lock()
                            .map_err(|_| "playback history lock poisoned".to_owned())
                            .and_then(|mut history| {
                                let mut removed = 0;
                                for reference in &removed_references {
                                    if history.remove_path(reference)? {
                                        removed += 1;
                                    }
                                }
                                if removed > 0 {
                                    trash_cleared.store(true, std::sync::atomic::Ordering::Release);
                                    trash_sync(&mut history)?;
                                }
                                Ok(removed)
                            });
                        let _ = slint::invoke_from_event_loop(move || {
                            let (Some(app), Some(window)) = (weak_app.upgrade(), weak_window.upgrade()) else {
                                return;
                            };
                            match history_result {
                                Ok(removed) => {
                                    if removed > 0 {
                                        clear_history_selection(&window, &trash_anchor);
                                        refresh(&window, &trash_history, &trash_groups);
                                    }
                                    if failures.is_empty() {
                                        app.set_status(format!("Moved {removed} files to the {trash_name}").into());
                                    } else {
                                        app.set_status(format!("Moved {removed} files to the {trash_name}; {} could not be moved", failures.len()).into());
                                    }
                                }
                                Err(error) => app.set_status(format!("Files were moved to the {trash_name}, but history could not be updated: {error}").into()),
                            }
                        });
                    });
            }
            _ => {}
        }
    });
}

fn clear_history_selection(window: &PlaybackHistoryWindow, anchor: &AtomicI32) {
    window.set_selected_path("".into());
    write_selected_paths(window, &HashSet::new());
    anchor.store(-1, Ordering::Release);
}

fn media_open_history_entry(
    app: &PlayerWindow,
    path: String,
    position: f64,
    player: riina_mpv::CommandSender,
) {
    let recent = RecentMedia {
        path,
        title: String::new(),
        position,
    };
    crate::media_open::open_recent(app, recent, app.get_resume_last_position(), player);
}

fn refresh(
    window: &PlaybackHistoryWindow,
    history: &HistoryState,
    collapsed_groups: &CollapsedGroups,
) {
    let Ok(history) = history.lock() else {
        window.set_rows(ModelRc::new(VecModel::from(
            Vec::<PlaybackHistoryRow>::new(),
        )));
        return;
    };
    let selected = read_selected_paths(window);
    let mut rows = build_rows(
        history.entries(),
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
    for row in &mut rows {
        row.selected = !row.is_group && selected.contains(row.path.as_str());
    }
    write_selected_paths(window, &selected);
    let selected_path = window.get_selected_path();
    if !rows
        .iter()
        .any(|row| !row.is_group && row.path == selected_path)
    {
        window.set_selected_path("".into());
    }
    window.set_rows(ModelRc::new(VecModel::from(rows)));
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
    let query = query.trim().to_lowercase();
    let mut groups = BTreeMap::<String, Vec<&Entry>>::new();
    for entry in entries {
        let (name, location, path, is_local, available) = describe(&entry.path);
        let searchable = if search_mode == 0 { &name } else { &entry.path };
        if !query.is_empty() && !searchable.to_lowercase().contains(&query) {
            continue;
        }
        let key = if group_mode == 1 {
            location
        } else {
            date_string(entry.date_ms)
        };
        let _ = (path, is_local, available);
        groups.entry(key).or_default().push(entry);
    }
    let mut buckets = groups.into_iter().collect::<Vec<_>>();
    if group_mode == 0 {
        buckets.sort_by(|(left_key, left), (right_key, right)| {
            right
                .iter()
                .map(|entry| entry.date_ms)
                .max()
                .cmp(&left.iter().map(|entry| entry.date_ms).max())
                .then_with(|| left_key.cmp(right_key))
        });
    } else {
        buckets.sort_by_key(|(group, _)| group.to_lowercase());
    }

    let mut rows = Vec::new();
    for (group, mut entries) in buckets {
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.date_ms));
        let collapsed = collapsed_groups.contains(&group);
        rows.push(PlaybackHistoryRow {
            is_group: true,
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
        for entry in entries {
            let (name, location, path, local_file, available) = describe(&entry.path);
            rows.push(PlaybackHistoryRow {
                is_group: false,
                label: name.into(),
                detail: location.into(),
                progress: progress_label(entry.progress, entry.duration).into(),
                played_at: played_at(entry.date_ms, group_mode == 1).into(),
                path: entry.path.clone().into(),
                progress_ratio: progress_ratio(entry.progress, entry.duration),
                available,
                local_file: local_file && path.is_file(),
                selected: selected_paths.contains(entry.path.as_str()),
            });
        }
    }
    rows
}

fn describe(reference: &str) -> (String, String, PathBuf, bool, bool) {
    if let Some(path) = path_from_reference(reference) {
        let name = path.file_name().map_or_else(
            || reference.to_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        let location = path.parent().map_or_else(
            || "".to_owned(),
            |parent| parent.to_string_lossy().into_owned(),
        );
        let available = path.is_file();
        return (name, location, path, true, available);
    }
    if let Ok(url) = url::Url::parse(reference) {
        let location = url.host_str().unwrap_or(reference).to_owned();
        let name = url
            .path_segments()
            .and_then(|mut segments| segments.next_back())
            .filter(|name| !name.is_empty())
            .map_or_else(|| reference.to_owned(), str::to_owned);
        return (name, location, PathBuf::new(), false, true);
    }
    (
        reference.to_owned(),
        String::new(),
        PathBuf::from(reference),
        true,
        false,
    )
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

    use super::context_selection_in_row_order;

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
    }
}
