//! Save the observed playlist as an M3U8 file from the File menu.

use crate::{NativeVideoHost, PlayerWindow, background, platform};
use riina_core::PlaylistEntry;
use slint::ComponentHandle;
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub(crate) fn install(
    app: &PlayerWindow,
    host: Option<slint::Weak<NativeVideoHost>>,
    playback: &crate::PlaybackStore,
) {
    let weak = app.as_weak();
    let playback = playback.clone();
    let pending = Arc::new(AtomicBool::new(false));
    app.global::<riina_ui::PlayerMenuActions>()
        .on_save_current_playlist(move || {
            if let Some(app) = weak.upgrade() {
                save_current_playlist(&app, host.clone(), &playback, &pending);
            }
        });
}

fn save_current_playlist(
    app: &PlayerWindow,
    host: Option<slint::Weak<NativeVideoHost>>,
    playback: &crate::PlaybackStore,
    pending: &Arc<AtomicBool>,
) {
    let Some(reservation) = ExportReservation::take(pending) else {
        return;
    };
    match playback.lock() {
        Ok(state) if !state.playlist.is_empty() => {}
        Ok(_) => return app.set_status("There is no playlist to save".into()),
        Err(_) => return app.set_status("Could not read the current playlist".into()),
    }
    let playback = playback.clone();
    platform::save_playlist_dialog(app, host, move |app, destination| {
        let path = match destination {
            Ok(Some(path)) => path,
            Ok(None) => return,
            Err(error) => return report(app, Err(error)),
        };
        // The playlist may have changed while the user chose a destination.
        let entries = match playback.lock() {
            Ok(state) => state.playlist.clone(),
            Err(_) => return report(app, Err("Could not read the current playlist".into())),
        };
        let weak = app.as_weak();
        if let Err(error) = background::spawn_named("playlist-export", move || {
            let result = export(&path, &entries);
            let _ = background::to_ui(weak, move |app| {
                report(&app, result);
                drop(reservation);
            });
        }) {
            report(app, Err(error.to_string()));
        }
    });
}

struct ExportReservation(Arc<AtomicBool>);

impl ExportReservation {
    fn take(pending: &Arc<AtomicBool>) -> Option<Self> {
        if pending.swap(true, Ordering::AcqRel) {
            return None;
        }
        Some(Self(pending.clone()))
    }
}

impl Drop for ExportReservation {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

fn export(path: &Path, entries: &[PlaylistEntry]) -> Result<(), String> {
    if entries.is_empty() {
        return Err("There is no playlist to save".into());
    }
    let mut content = String::new();
    for entry in entries {
        if entry.filename.is_empty() || entry.filename.contains(['\r', '\n', '\0']) {
            return Err("The playlist contains a path that cannot be saved as M3U8".into());
        }
        content.push_str(&entry.filename);
        content.push('\n');
    }
    platform::settings_file::save(path, &content).map_err(|error| error.to_string())
}

fn report(app: &PlayerWindow, result: Result<(), String>) {
    match result {
        Ok(()) => app.set_status("Playlist saved".into()),
        Err(error) => crate::diagnostics::report(app, format!("Could not save playlist: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_test_support::TempDir;

    fn entry(id: i64, filename: &str) -> PlaylistEntry {
        PlaylistEntry {
            id,
            filename: filename.into(),
            title: None,
            current: false,
            playing: false,
        }
    }

    #[test]
    fn export_replaces_a_file_with_ordered_utf8_and_keeps_it_on_invalid_input() {
        let directory = TempDir::new("playlist-export").unwrap();
        let path = directory.path().join("Séance.m3u8");
        std::fs::write(&path, "old playlist").unwrap();
        let entries = [
            entry(1, "C:/Videos/Café A.mkv"),
            entry(2, "https://example.test/live"),
            entry(3, "C:/Videos/Café A.mkv"),
        ];
        export(&path, &entries).unwrap();
        let expected = "C:/Videos/Café A.mkv\nhttps://example.test/live\nC:/Videos/Café A.mkv\n";
        assert_eq!(std::fs::read_to_string(&path).unwrap(), expected);

        let invalid = entry(4, "invalid\nentry");
        assert!(export(&path, &[entries[0].clone(), invalid]).is_err());
        assert!(export(&path, &[]).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), expected);
    }

    #[test]
    fn an_in_flight_export_blocks_only_its_own_players_next_save() {
        let first = Arc::new(AtomicBool::new(false));
        let second = Arc::new(AtomicBool::new(false));
        let exporting = ExportReservation::take(&first).unwrap();
        assert!(ExportReservation::take(&first).is_none());
        assert!(ExportReservation::take(&second).is_some());
        drop(exporting);
        assert!(ExportReservation::take(&first).is_some());
    }
}
