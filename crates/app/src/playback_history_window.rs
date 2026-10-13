use crate::{PlaybackHistoryRow, PlaybackHistoryWindow, PlayerWindow};
use chrono::{Local, TimeZone};
use rfd::{MessageButtons, MessageDialog, MessageDialogResult};
use riina_core::{RecentMedia, playback_history::Entry};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

type HistoryState = Arc<Mutex<crate::plugin_host::PlaybackHistoryState>>;
type HistoryWindowSlot = Rc<RefCell<Option<PlaybackHistoryWindow>>>;
pub(crate) type HistorySync =
    Arc<dyn Fn(&mut crate::plugin_host::PlaybackHistoryState) -> Result<(), String> + Send + Sync>;

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
    let weak_app = app.as_weak();
    let show_slot = slot.clone();
    let show_history = playback_history.clone();
    let show_last_revision = last_revision.clone();
    let show_player = player.clone();
    let show_preferences_path = preferences_path.clone();
    let show_history_cleared = history_cleared.clone();
    let show_history_sync = history_sync.clone();

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
                show_history.clone(),
                show_player.clone(),
                show_preferences_path.clone(),
                show_history_cleared.clone(),
                show_history_sync.clone(),
            );
            window
                .window()
                .on_close_requested(|| slint::CloseRequestResponse::HideWindow);
            crate::platform::center_auxiliary_window(&window, &app);
            *show_slot.borrow_mut() = Some(window.clone_strong());
            window
        };
        refresh(&window, &show_history);
        show_last_revision.set(Some(revision(&show_history)));
        if let Err(error) = window.show() {
            app.set_status(format!("Could not show Playback History: {error}").into());
        }
    });

    let timer = Rc::new(slint::Timer::default());
    let timer_slot = slot.clone();
    let timer_history = playback_history;
    let timer_revision = last_revision;
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
            refresh(&window, &timer_history);
            timer_revision.set(Some(current_revision));
        },
    );
    (slot, timer)
}

fn connect_window(
    app: &PlayerWindow,
    window: &PlaybackHistoryWindow,
    history: HistoryState,
    player: riina_mpv::CommandSender,
    preferences_path: PathBuf,
    history_cleared: Arc<std::sync::atomic::AtomicBool>,
    history_sync: HistorySync,
) {
    let weak_window = window.as_weak();
    let observed_history = history.clone();
    window.on_search_changed(move |_query, _group_mode| {
        let Some(window) = weak_window.upgrade() else {
            return;
        };
        refresh(&window, &observed_history);
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
    window.on_open_entry_in_new_window(move |path| {
        let Some(app) = weak_app.upgrade() else {
            return;
        };
        app.invoke_open_player_requested(
            ModelRc::new(VecModel::from(vec![SharedString::from(path.as_str())])),
            -1.0,
        );
    });

    let weak_app = app.as_weak();
    window.on_reveal_entry(move |path| {
        let Some(app) = weak_app.upgrade() else {
            return;
        };
        let path = path_from_reference(path.as_str());
        let Some(path) = path else {
            app.set_status("Only local playback history items can be revealed".into());
            return;
        };
        if !path.is_file() {
            app.set_status("The selected history file is unavailable".into());
        } else if let Err(error) = crate::platform::reveal_file(&path) {
            app.set_status(format!("Could not reveal the history file: {error}").into());
        }
    });

    let weak_app = app.as_weak();
    let weak_window = window.as_weak();
    let history_for_remove = history.clone();
    let history_cleared_for_remove = history_cleared.clone();
    let history_sync_for_remove = history_sync.clone();
    window.on_remove_entry(move |path| {
        let (Some(app), Some(window)) = (weak_app.upgrade(), weak_window.upgrade()) else {
            return;
        };
        let response = MessageDialog::new()
            .set_title("Remove from Playback History")
            .set_description("Remove this item from the playback history?")
            .set_buttons(MessageButtons::YesNo)
            .show();
        if response != MessageDialogResult::Yes {
            return;
        }
        match history_for_remove
            .lock()
            .map_err(|_| "playback history lock poisoned".to_owned())
            .and_then(|mut history| history.remove_path(path.as_str()))
        {
            Ok(true) => {
                if let Ok(mut history) = history_for_remove.lock() {
                    history_cleared_for_remove.store(true, std::sync::atomic::Ordering::Release);
                    if let Err(error) = history_sync_for_remove(&mut history) {
                        app.set_status(
                            format!("History removed, but plugins could not be updated: {error}")
                                .into(),
                        );
                    }
                }
                window.set_selected_index(-1);
                window.set_selected_path("".into());
                refresh(&window, &history_for_remove);
            }
            Ok(false) => app.set_status("The history item was already removed".into()),
            Err(error) => app.set_status(format!("Could not remove history item: {error}").into()),
        }
    });

    let weak_app = app.as_weak();
    let weak_window = window.as_weak();
    let history_for_delete = history.clone();
    let history_cleared_for_delete = history_cleared.clone();
    let history_sync_for_delete = history_sync.clone();
    window.on_delete_file(move |reference| {
        let (Some(app), Some(window)) = (weak_app.upgrade(), weak_window.upgrade()) else {
            return;
        };
        let Some(path) = path_from_reference(reference.as_str()) else {
            app.set_status("Only local history files can be moved to the system bin".into());
            return;
        };
        if !path.is_file() {
            app.set_status("The selected history file is unavailable".into());
            return;
        }
        let response = MessageDialog::new()
            .set_title("Move File to Trash")
            .set_description(format!("Move {} to the system bin?", path.display()))
            .set_buttons(MessageButtons::YesNo)
            .show();
        if response != MessageDialogResult::Yes {
            return;
        }
        let weak_app = app.as_weak();
        let weak_window = window.as_weak();
        let history_after_delete = history_for_delete.clone();
        let history_sync_after_delete = history_sync_for_delete.clone();
        let history_cleared_after_delete = history_cleared_for_delete.clone();
        let trash_name = crate::platform::system_trash_name();
        let _ = std::thread::Builder::new()
            .name("riina-history-trash".into())
            .spawn(move || {
                let result = trash::delete(&path).map_err(|error| error.to_string());
                let _ = slint::invoke_from_event_loop(move || {
                    let (Some(app), Some(window)) =
                        (weak_app.upgrade(), weak_window.upgrade())
                    else {
                        return;
                    };
                    match result {
                        Ok(()) => {
                            let removed = history_after_delete.lock().map_err(|_| {
                                "playback history lock poisoned".to_owned()
                            }).and_then(|mut history| {
                                let removed = history.remove_path(reference.as_str())?;
                                if removed {
                                    history_cleared_after_delete
                                        .store(true, std::sync::atomic::Ordering::Release);
                                    history_sync_after_delete(&mut history)?;
                                }
                                Ok(removed)
                            });
                            match removed {
                                Ok(_) => {
                                    window.set_selected_index(-1);
                                    window.set_selected_path("".into());
                                    refresh(&window, &history_after_delete);
                                    app.set_status(format!("Moved file to the {trash_name}").into());
                                }
                                Err(error) => app.set_status(format!(
                                    "File moved to the {trash_name}, but history could not be updated: {error}"
                                ).into()),
                            }
                        }
                        Err(error) => app.set_status(
                            format!("Could not move history file to the {trash_name}: {error}")
                                .into(),
                        ),
                    }
                });
            });
    });

    let weak_app = app.as_weak();
    let weak_window = window.as_weak();
    let history_for_clear = history.clone();
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
                window.set_selected_index(-1);
                window.set_selected_path("".into());
                refresh(&window, &history_for_clear);
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

fn refresh(window: &PlaybackHistoryWindow, history: &HistoryState) {
    let Ok(history) = history.lock() else {
        window.set_rows(ModelRc::new(VecModel::from(
            Vec::<PlaybackHistoryRow>::new(),
        )));
        return;
    };
    let rows = build_rows(
        history.entries(),
        window.get_search_text().as_str(),
        window.get_search_mode(),
        window.get_group_mode(),
    );
    let selected_path = window.get_selected_path();
    let selected_index = rows
        .iter()
        .position(|row| !row.is_group && row.path == selected_path);
    window.set_selected_index(selected_index.map_or(-1, |index| index as i32));
    if selected_index.is_none() {
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
        rows.push(PlaybackHistoryRow {
            is_group: true,
            label: group.into(),
            detail: "".into(),
            progress: "".into(),
            played_at: "".into(),
            path: "".into(),
            progress_ratio: 0.0,
            available: false,
            local_file: false,
        });
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
