//! Read mpv's saved positions without blocking playback or the History window.
use super::{PlaybackHistoryState, SHARED_PLAYBACK_HISTORIES};
use crate::{background, playback_history_store};
use riina_core::sync::lock;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{Arc, Mutex},
};

type Archive = Arc<Mutex<PlaybackHistoryState>>;

#[derive(Debug, Default)]
pub(super) struct Refresh {
    configured_policy: bool,
    observed_policy: Option<bool>,
    epoch: u64,
    pending: bool,
    running: bool,
    replaced: HashSet<String>,
}

impl Refresh {
    pub(super) fn cancel(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
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
    let _ = request(archive, RefreshReason::Open);
}

pub(crate) fn observe_path_policy(archive: &Archive, ignore_path: bool) {
    let _ = request(archive, RefreshReason::Observed { ignore_path });
}

pub(crate) fn files_cleared(preferences_path: &Path, all_removed: bool) {
    let Some(histories) = SHARED_PLAYBACK_HISTORIES.get() else {
        return;
    };
    let key = playback_history_store::path_for_preferences(preferences_path);
    let archive = lock(histories).get(&key).cloned();
    if let Some(archive) = archive {
        let _ = request(&archive, RefreshReason::FilesCleared { all_removed });
    }
}

enum RefreshReason {
    Open,
    Observed { ignore_path: bool },
    FilesCleared { all_removed: bool },
}

fn request(archive: &Archive, reason: RefreshReason) -> Option<background::TaskHandle<()>> {
    let mut state = lock(archive);
    let directory = state.watch_later_directory.clone()?;
    let refresh = &mut state.progress;
    match reason {
        RefreshReason::Observed { ignore_path } => {
            if refresh.observed_policy == Some(ignore_path) {
                return None;
            }
            refresh.observed_policy = Some(ignore_path);
        }
        RefreshReason::Open => {}
        RefreshReason::FilesCleared { all_removed: true } => {
            // A scan that already read a deleted position must never publish it.
            refresh.cancel();
            if state.entries.iter().any(|entry| entry.progress.is_some()) {
                for entry in &mut state.entries {
                    entry.progress = None;
                }
                state.files_changed();
            }
            return None;
        }
        RefreshReason::FilesCleared { all_removed: false } => {}
    }
    if state.entries.is_empty() || !state.progress.schedule() {
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
        for chunk in scan.paths.chunks(64) {
            if lock(archive).progress.epoch != scan.epoch {
                break;
            }
            for path in chunk {
                let position =
                    playback_history_store::progress_from_watch_later(path, directory, scan.policy);
                positions.insert(path.as_str(), position);
            }
        }
        scan.publish(archive, &positions);
    }
}

struct Scan {
    epoch: u64,
    policy: bool,
    paths: Vec<String>,
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
            policy: refresh.observed_policy.unwrap_or(refresh.configured_policy),
            paths: state
                .entries
                .iter()
                .map(|entry| entry.path.clone())
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

pub(crate) fn configure_path_policy(
    archive: &Archive,
    settings: &riina_core::advanced_preferences::Settings,
) {
    lock(archive).progress.configured_policy = settings.enabled
        && settings
            .user_options
            .iter()
            .rev()
            .find(|option| option.name == "ignore-path-in-watch-later-config")
            .is_some_and(|option| option.value == "yes");
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_core::{advanced_preferences, playback_history::Entry};
    use riina_test_support::TempDir;

    fn entry(path: String) -> Entry {
        Entry {
            path,
            title: None,
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
        let history = [entry(
            super::super::playback_media_url(native.to_str().unwrap()).unwrap(),
        )];
        playback_history_store::save(&archive_path, &history).unwrap();
        let archive = Arc::new(Mutex::new(PlaybackHistoryState::with_persistence(
            history.to_vec(),
            Some(archive_path.clone()),
            Some(directory.to_path_buf()),
        )));
        assert_eq!(lock(&archive).entries()[0].progress, None);

        let key = playback_history_store::md5_hex(b"movie.mp4").to_ascii_uppercase();
        let saved = directory.join(key);
        configure_path_policy(
            &archive,
            &advanced_preferences::Settings {
                enabled: true,
                user_options: vec![advanced_preferences::MpvOption {
                    name: "ignore-path-in-watch-later-config".into(),
                    value: "yes".into(),
                }],
                ..advanced_preferences::Settings::default()
            },
        );
        for position in [42.25, 58.0] {
            std::fs::write(&saved, format!("start={position}\n")).unwrap();
            request(&archive, RefreshReason::Open)
                .unwrap()
                .join()
                .unwrap();
            assert_eq!(lock(&archive).entries()[0].progress, Some(position));
        }

        request(&archive, RefreshReason::Observed { ignore_path: false })
            .unwrap()
            .join()
            .unwrap();
        assert_eq!(lock(&archive).entries()[0].progress, None);
        assert_eq!(
            playback_history_store::load(&archive_path, &[]).unwrap(),
            history
        );
    }

    #[test]
    fn clearing_progress_rejects_old_results_and_keeps_history_rows() {
        let temp = TempDir::new("history-clear-progress").unwrap();
        let preferences = temp.path().join("preferences.json");
        let path = playback_history_store::path_for_preferences(&preferences);
        let media = "https://media.example/clip.mkv";
        let archive = super::super::shared_playback_history_state(
            path,
            vec![entry(media.into())],
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
}
