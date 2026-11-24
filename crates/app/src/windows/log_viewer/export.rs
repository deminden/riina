//! Save a fixed snapshot through a dialog owned by the Log Viewer.
use super::{LogViewerWindow, View};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{cell::RefCell, path::Path, rc::Rc};

pub(super) fn bind(window: &LogViewerWindow, view: &Rc<RefCell<View>>, preferences: &Path) {
    let weak = window.as_weak();
    let state = view.clone();
    let sources = [
        preferences.with_file_name("native-player.log"),
        preferences.with_file_name("riina.log"),
    ];
    window.on_save_logs(move |filtered| {
        let Some(window) = weak
            .upgrade()
            .filter(|window| !window.get_save_in_progress())
        else {
            return;
        };
        let snapshot = state.borrow().export(&window, filtered);
        if snapshot.is_empty() {
            window.set_feedback_text("There are no log entries to save".into());
            return;
        }

        let (title, filename, saved) = if filtered {
            (
                "Save Filtered Logs",
                "riina-filtered.log",
                "Filtered logs saved",
            )
        } else {
            ("Save Logs", "riina-export.log", "Logs saved")
        };
        let weak = window.as_weak();
        let sources = sources.clone();
        window.set_save_in_progress(true);
        window.set_feedback_text("".into());
        if let Err(error) = slint::spawn_local(async move {
            let Some(window) = weak.upgrade() else { return };
            let choice = async {
                let owner = window.window().winit_window().await?;
                Ok::<_, slint::PlatformError>(
                    rfd::AsyncFileDialog::new()
                        .set_title(title)
                        .set_file_name(filename)
                        .add_filter("Log file", &["log", "txt"])
                        .set_parent(owner.as_ref())
                        .save_file()
                        .await,
                )
            }
            .await;

            match choice {
                Ok(Some(file)) => {
                    let path = file.path().to_path_buf();
                    let weak = window.as_weak();
                    if let Err(error) =
                        crate::background::spawn_named("riina-log-export", move || {
                            let result = if is_source(&path, &sources) {
                                Err("Choose a file outside the active logs".into())
                            } else {
                                crate::platform::save_preferences(&path, &snapshot)
                                    .map_err(|error| error.to_string())
                            };
                            let _ = crate::background::to_ui(weak, move |window| {
                                window.set_save_in_progress(false);
                                match result {
                                    Ok(()) => window.set_feedback_text(saved.into()),
                                    Err(error) => report(&window, &error),
                                }
                            });
                        })
                    {
                        report(&window, &error);
                    }
                }
                Ok(None) => window.set_save_in_progress(false),
                Err(error) => report(&window, &error),
            }
        }) {
            report(&window, &error);
        }
    });
}

fn is_source(path: &Path, sources: &[std::path::PathBuf]) -> bool {
    let canonical = path.canonicalize().ok();
    sources.iter().any(|source| {
        source == path
            || canonical
                .as_ref()
                .is_some_and(|path| source.canonicalize().is_ok_and(|source| source == *path))
    })
}

fn report(window: &LogViewerWindow, error: &dyn std::fmt::Display) {
    window.set_save_in_progress(false);
    window.set_feedback_text(format!("Could not save logs: {error}").into());
}
