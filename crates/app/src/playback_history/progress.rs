//! Read mpv's saved positions without blocking playback or the History window.
use super::{PlaybackHistoryState, SHARED_PLAYBACK_HISTORIES};
use crate::{background, playback_history_store};
use riina_core::{playback_history::WatchLaterKey, sync::lock};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{Arc, Mutex},
};

type Archive = Arc<Mutex<PlaybackHistoryState>>;

#[derive(Debug, Default)]
pub(super) struct Refresh {
    epoch: u64,
    dirty: bool,
    pending: bool,
    running: bool,
    replaced: HashSet<String>,
}

impl Refresh {
    pub(super) fn cancel(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        self.dirty = false;
        self.pending = false;
        self.replaced.clear();
    }

    pub(super) fn entry_replaced(&mut self, path: &str) {
        if self.running {
            self.replaced.insert(path.to_owned());
        }
    }

    fn schedule(&mut self) -> bool {
        self.cancel();
        self.pending = true;
        !std::mem::replace(&mut self.running, true)
    }
}

pub(crate) fn refresh_on_open(archive: &Archive) {
    let _ = request(archive, false);
}

pub(crate) fn refresh_if_changed(archive: &Archive) {
    let _ = request(archive, true);
}

pub(crate) fn files_changed(archive: &Archive) {
    let mut state = lock(archive);
    // A scan begun before the edit may already have read an obsolete position.
    state.progress.cancel();
    state.progress.dirty = true;
}

pub(crate) fn files_cleared(preferences_path: &Path, all_removed: bool) {
    let Some(histories) = SHARED_PLAYBACK_HISTORIES.get() else {
        return;
    };
    let key = playback_history_store::path_for_preferences(preferences_path);
    let archive = lock(histories).get(&key).cloned();
    let Some(archive) = archive else { return };
    if !all_removed {
        files_changed(&archive);
        return;
    }
    let mut state = lock(&archive);
    // A scan that already read a deleted position must never publish it.
    state.progress.cancel();
    if state.entries.iter().any(|entry| entry.progress.is_some()) {
        for entry in &mut state.entries {
            entry.progress = None;
        }
        state.files_changed();
    }
}

fn request(archive: &Archive, only_if_dirty: bool) -> Option<background::TaskHandle<()>> {
    let mut state = lock(archive);
    let dirty = std::mem::take(&mut state.progress.dirty);
    if only_if_dirty && !dirty {
        return None;
    }
    let directory = state.watch_later_directory.clone()?;
    if state
        .entries
        .iter()
        .all(|entry| entry.watch_later_key.is_none())
        || !state.progress.schedule()
    {
        return None;
    }
    let shared = archive.clone();
    match background::spawn_named("riina-history-saved-progress", move || {
        run(&shared, &directory)
    }) {
        Ok(worker) => Some(worker),
        Err(error) => {
            state.progress.running = false;
            state.progress.cancel();
            crate::diagnostics::warn!("Could not read Playback History progress: {error}");
            None
        }
    }
}

fn run(archive: &Archive, directory: &Path) {
    while let Some(scan) = Scan::take(archive) {
        let mut positions = HashMap::new();
        for chunk in scan.entries.chunks(64) {
            if lock(archive).progress.epoch != scan.epoch {
                break;
            }
            for (path, key) in chunk {
                let position = playback_history_store::progress_for_key(key, directory);
                positions.insert(path.as_str(), position);
            }
        }
        scan.publish(archive, &positions);
    }
}

struct Scan {
    epoch: u64,
    entries: Vec<(String, WatchLaterKey)>,
}

impl Scan {
    fn take(archive: &Archive) -> Option<Self> {
        let mut state = lock(archive);
        let refresh = &mut state.progress;
        if !std::mem::take(&mut refresh.pending) {
            refresh.running = false;
            return None;
        }
        refresh.replaced.clear();
        Some(Self {
            epoch: refresh.epoch,
            entries: state
                .entries
                .iter()
                .filter_map(|entry| Some((entry.path.clone(), entry.watch_later_key.clone()?)))
                .collect(),
        })
    }

    fn publish(&self, archive: &Archive, positions: &HashMap<&str, Option<f64>>) {
        let mut state = lock(archive);
        if state.progress.epoch != self.epoch {
            return;
        }
        let replaced = std::mem::take(&mut state.progress.replaced);
        let mut changed = false;
        for entry in &mut state.entries {
            if !replaced.contains(&entry.path)
                && let Some(&position) = positions.get(entry.path.as_str())
                && entry.progress != position
            {
                entry.progress = position;
                changed = true;
            }
        }
        if changed {
            state.files_changed();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_core::playback_history::Entry;
    use riina_test_support::TempDir;

    fn entry(path: String) -> Entry {
        Entry {
            path,
            title: None,
            watch_later_key: None,
            date_ms: 1,
            progress: Some(99.0),
            duration: 90.0,
        }
    }

    #[test]
    fn reopening_refreshes_watch_later_even_without_a_player() {
        let temp = TempDir::new("history-progress").unwrap();
        let directory = temp.path();
        let archive_path = directory.join("playback-history.json");
        let native = directory.join("movie.mp4");
        let media = native.to_str().unwrap();
        let key = playback_history_store::watch_later_key(media, true).unwrap();
        let saved = directory.join(key.as_str());
        let history = [Entry {
            watch_later_key: Some(key),
            ..entry(super::super::playback_media_url(media).unwrap())
        }];
        playback_history_store::save(&archive_path, &history).unwrap();
        let archive = Arc::new(Mutex::new(PlaybackHistoryState::with_persistence(
            history.to_vec(),
            Some(archive_path.clone()),
            Some(directory.to_path_buf()),
        )));
        assert_eq!(lock(&archive).entries()[0].progress, None);

        for position in [42.25, 58.0] {
            std::fs::write(&saved, format!("start={position}\n")).unwrap();
            request(&archive, false).unwrap().join().unwrap();
            assert_eq!(lock(&archive).entries()[0].progress, Some(position));
        }

        std::fs::remove_file(saved).unwrap();
        request(&archive, false).unwrap().join().unwrap();
        assert_eq!(lock(&archive).entries()[0].progress, None);
        assert_eq!(
            playback_history_store::load(&archive_path).unwrap(),
            history
        );
    }

    #[test]
    fn progress_is_only_loaded_for_an_entry_with_a_recorded_key() {
        let temp = TempDir::new("history-keyed-progress").unwrap();
        let directory = temp.path();
        let media = directory.join("movie.mp4");
        let media = media.to_str().unwrap();
        let key = playback_history_store::watch_later_key(media, true).unwrap();
        let full_key = playback_history_store::watch_later_key(media, false).unwrap();
        std::fs::write(directory.join(key.as_str()), "start=12\n").unwrap();
        std::fs::write(directory.join(full_key.as_str()), "start=34\n").unwrap();

        let old = entry(media.into());
        let recorded = Entry {
            watch_later_key: Some(key),
            ..entry(directory.join("moved/movie.mp4").to_str().unwrap().into())
        };
        let archive = Arc::new(Mutex::new(PlaybackHistoryState::with_persistence(
            vec![old, recorded],
            None,
            Some(directory.into()),
        )));
        request(&archive, false).unwrap().join().unwrap();
        let history = lock(&archive);
        assert_eq!(history.entries()[0].progress, None);
        assert_eq!(history.entries()[1].progress, Some(12.0));
    }

    #[test]
    fn clearing_progress_rejects_old_results_and_keeps_history_rows() {
        let temp = TempDir::new("history-clear-progress").unwrap();
        let preferences = temp.path().join("preferences.json");
        let path = playback_history_store::path_for_preferences(&preferences);
        let media = "https://media.example/clip.mkv";
        let archive = super::super::shared_playback_history_state(
            path,
            vec![Entry {
                watch_later_key: playback_history_store::watch_later_key(media, false),
                ..entry(media.into())
            }],
            None,
            Some(temp.path().join("watch-later")),
        );
        assert!(lock(&archive).progress.schedule());
        let scan = Scan::take(&archive).unwrap();
        let results = HashMap::from([(media, Some(12.0))]);
        scan.publish(&archive, &results);
        assert_eq!(lock(&archive).entries()[0].progress, Some(12.0));

        files_cleared(&preferences, true);
        scan.publish(&archive, &results);
        assert_eq!(lock(&archive).entries()[0].progress, None);
        assert_eq!(lock(&archive).entries()[0].path, media);
        assert!(Scan::take(&archive).is_none());
    }

    #[test]
    fn native_edits_coalesce_until_visible_refresh_and_discard_an_inflight_read() {
        let temp = TempDir::new("history-native-edit").unwrap();
        let directory = temp.path();
        let media = "https://media.example/clip.mkv";
        let key = playback_history_store::watch_later_key(media, false).unwrap();
        let saved = directory.join(key.as_str());
        let archive = Arc::new(Mutex::new(PlaybackHistoryState::with_persistence(
            vec![Entry {
                watch_later_key: Some(key),
                ..entry(media.into())
            }],
            None,
            Some(directory.into()),
        )));
        assert!(request(&archive, true).is_none());
        assert!(lock(&archive).progress.schedule());
        let old_scan = Scan::take(&archive).unwrap();
        std::fs::write(&saved, "start=27\n").unwrap();
        files_changed(&archive);
        std::fs::write(&saved, "start=41\n").unwrap();
        files_changed(&archive);
        old_scan.publish(&archive, &HashMap::from([(media, Some(12.0))]));
        assert_eq!(lock(&archive).entries()[0].progress, None);
        assert!(Scan::take(&archive).is_none());

        request(&archive, true).unwrap().join().unwrap();
        assert_eq!(lock(&archive).entries()[0].progress, Some(41.0));
        assert!(request(&archive, true).is_none());

        std::fs::remove_file(saved).unwrap();
        files_changed(&archive);
        request(&archive, true).unwrap().join().unwrap();
        assert_eq!(lock(&archive).entries()[0].progress, None);
    }
}
