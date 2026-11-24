//! Keep confirmation and its operation attached to the History window.
use super::{
    PlaybackHistoryWindow, WindowConnection, clear_history_selection, path_from_reference, refresh,
    report, selection,
};
use slint::ComponentHandle;
use std::{cell::RefCell, collections::BTreeSet, path::PathBuf};

enum Action {
    Clear,
    Remove(Vec<String>),
    Trash(BTreeSet<PathBuf>, &'static str),
}

#[derive(Default)]
pub(super) struct Session(RefCell<Option<Action>>);

impl Session {
    fn request(
        &self,
        window: &PlaybackHistoryWindow,
        action: Action,
        title: &str,
        description: String,
        button: &str,
    ) {
        let mut pending = self.0.borrow_mut();
        if pending.is_some() {
            return;
        }
        *pending = Some(action);
        drop(pending);
        window.set_confirmation_title(title.into());
        window.set_confirmation_description(description.into());
        window.set_confirmation_button(button.into());
        window.set_confirmation_visible(true);
    }

    pub(super) fn remove(&self, window: &PlaybackHistoryWindow, paths: Vec<String>) {
        let description = match paths.len() {
            0 => return,
            1 => "Remove this item from the playback history?".to_owned(),
            count => format!("Remove {count} items from the playback history?"),
        };
        self.request(
            window,
            Action::Remove(paths),
            "Remove from Playback History",
            description,
            "Remove",
        );
    }

    pub(super) fn trash(&self, window: &PlaybackHistoryWindow, paths: Vec<String>) {
        let files = paths
            .iter()
            .filter_map(|reference| path_from_reference(reference))
            .filter(|path| path.is_file())
            .collect::<BTreeSet<_>>();
        let Some(first) = files.first() else {
            window.set_status_text(
                "Only available local files can be moved to the system bin".into(),
            );
            return;
        };
        let trash_name = crate::platform::system_trash_name();
        let description = if files.len() == 1 {
            let name = first
                .file_name()
                .unwrap_or(first.as_os_str())
                .to_string_lossy();
            format!("Move {name} to the {trash_name}?")
        } else {
            format!("Move {} files to the {trash_name}?", files.len())
        };
        self.request(
            window,
            Action::Trash(files, trash_name),
            "Move Files to Trash",
            description,
            "Move to Trash",
        );
    }
}

pub(super) fn connect(
    window: &PlaybackHistoryWindow,
    context: &WindowConnection,
    selection_anchor: selection::Anchor,
) {
    let pending = context.confirmation.clone();
    let weak = window.as_weak();
    window.on_clear_history(move || {
        if let Some(window) = weak.upgrade() {
            pending.request(
                &window,
                Action::Clear,
                "Clear Playback History",
                "Delete all recent playback history?".to_owned(),
                "Clear",
            );
        }
    });

    let context = context.clone();
    let weak = window.as_weak();
    window.on_answer_confirmation(move |accepted| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let action = context.confirmation.0.borrow_mut().take();
        window.set_confirmation_visible(false);
        if !accepted || !window.window().is_visible() {
            return;
        }
        match action {
            Some(Action::Clear) => clear_history(&window, &selection_anchor, &context),
            Some(Action::Remove(paths)) => {
                remove_history_entries(&window, paths, &selection_anchor, &context);
            }
            Some(Action::Trash(files, trash_name)) => {
                move_history_files_to_trash(&window, files, trash_name, &context);
            }
            None => {}
        }
    });
}

fn clear_history(
    window: &PlaybackHistoryWindow,
    anchor: &selection::Anchor,
    context: &WindowConnection,
) {
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
            clear_history_selection(window, anchor);
            context
                .collapsed_groups
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clear();
            refresh(window, &context.archive.history, &context.collapsed_groups);
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
            report(window, format!("Could not clear playback history: {error}"));
        }
    }
}

fn remove_history_entries(
    window: &PlaybackHistoryWindow,
    paths: Vec<String>,
    selection_anchor: &selection::Anchor,
    connection: &WindowConnection,
) {
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
    files: BTreeSet<PathBuf>,
    trash_name: &'static str,
    connection: &WindowConnection,
) {
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
            let items = if moved == 1 {
                "1 file".to_owned()
            } else {
                format!("{moved} files")
            };
            if moved == count {
                window.set_status_text(format!("Moved {items} to the {trash_name}").into());
            } else {
                report(
                    &window,
                    format!(
                        "Moved {items} to the {trash_name}. Could not move {} files.",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playback_history::PlaybackHistoryState;
    use crate::playback_history_window::Archive;
    use riina_core::playback_history::Entry;
    use slint::platform::{Key, WindowEvent};
    use std::{
        rc::Weak,
        sync::{Arc, Mutex},
    };

    #[test]
    fn history_confirmation_defaults_to_cancel_and_consumes_only_one_action() {
        i_slint_backend_testing::init_no_event_loop();
        let window = PlaybackHistoryWindow::new().unwrap();
        let paths = [
            "https://example.test/first.mp4",
            "https://example.test/second.mp4",
        ];
        let entries = paths.map(|path| Entry {
            path: path.into(),
            title: None,
            date_ms: 0,
            duration: 30.0,
            progress: None,
        });
        let archive = Archive {
            history: Arc::new(Mutex::new(PlaybackHistoryState::with_persistence(
                entries.to_vec(),
                None,
                None,
            ))),
            preferences_path: PathBuf::new(),
            cleared: Arc::default(),
        };
        let connection = WindowConnection {
            archive,
            collapsed_groups: Arc::default(),
            owner: Weak::new(),
            confirmation: Default::default(),
        };
        connect(&window, &connection, selection::Anchor::default());
        window.show().unwrap();
        let press = |text: slint::SharedString| {
            slint::platform::update_timers_and_animations();
            window
                .window()
                .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
            window
                .window()
                .dispatch_event(WindowEvent::KeyReleased { text });
        };
        let remaining = || {
            riina_core::sync::lock(&connection.archive.history)
                .entries()
                .len()
        };

        connection
            .confirmation
            .remove(&window, vec![paths[0].into()]);
        window.invoke_clear_history();
        assert_eq!(window.get_confirmation_button().as_str(), "Remove");
        press(Key::Return.into());
        assert!(!window.get_confirmation_visible());
        assert_eq!(remaining(), 2);

        connection
            .confirmation
            .remove(&window, vec![paths[0].into()]);
        press(Key::Tab.into());
        press(Key::Return.into());
        assert!(!window.get_confirmation_visible());
        assert_eq!(remaining(), 1);
        window.invoke_answer_confirmation(true);
        assert_eq!(remaining(), 1);
        assert_eq!(
            riina_core::sync::lock(&connection.archive.history).entries()[0].path,
            paths[1]
        );

        window.invoke_clear_history();
        press(Key::Escape.into());
        assert!(!window.get_confirmation_visible());
        assert_eq!(remaining(), 1);
        window.hide().unwrap();
    }
}
