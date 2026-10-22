//! Application-owned playback history, shared across player windows.
use riina_core::{Phase, Playback};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

static NEXT_HISTORY_SESSION_ID: AtomicU64 = AtomicU64::new(1);
static SHARED_PLAYBACK_HISTORIES: OnceLock<
    Mutex<HashMap<PathBuf, Arc<Mutex<PlaybackHistoryState>>>>,
> = OnceLock::new();
static SHARED_HISTORY_MERGE_FLAGS: OnceLock<Mutex<HashMap<PathBuf, Arc<AtomicBool>>>> =
    OnceLock::new();

pub(crate) fn next_history_session_id() -> u64 {
    NEXT_HISTORY_SESSION_ID
        .fetch_add(1, Ordering::Relaxed)
        .max(1)
}

pub(crate) fn shared_playback_history_state(
    key: PathBuf,
    entries: Vec<riina_core::playback_history::Entry>,
    persistence_path: Option<PathBuf>,
    watch_later_directory: Option<PathBuf>,
) -> Arc<Mutex<PlaybackHistoryState>> {
    let histories = SHARED_PLAYBACK_HISTORIES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut histories = histories
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    histories
        .entry(key)
        .or_insert_with(|| {
            Arc::new(Mutex::new(PlaybackHistoryState::with_persistence(
                entries,
                persistence_path,
                watch_later_directory,
            )))
        })
        .clone()
}

pub(crate) fn shared_history_merge_flag(preferences_path: &Path) -> Arc<AtomicBool> {
    let flags = SHARED_HISTORY_MERGE_FLAGS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut flags = flags
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    flags
        .entry(preferences_path.to_path_buf())
        .or_insert_with(|| Arc::new(AtomicBool::new(false)))
        .clone()
}

#[derive(Debug, Clone, Default)]
pub(crate) struct PlaybackHistoryState {
    entries: Vec<riina_core::playback_history::Entry>,
    revision: u64,
    observed_sessions: HashMap<u64, (u64, String)>,
    persistence_path: Option<PathBuf>,
    watch_later_directory: Option<PathBuf>,
    pending_progress_paths: HashSet<String>,
    persistence_error: Option<String>,
}

impl PlaybackHistoryState {
    pub(crate) fn with_persistence(
        entries: Vec<riina_core::playback_history::Entry>,
        path: Option<PathBuf>,
        watch_later_directory: Option<PathBuf>,
    ) -> Self {
        let pending_progress_paths = entries.iter().map(|entry| entry.path.clone()).collect();
        Self {
            entries,
            revision: 1,
            observed_sessions: HashMap::new(),
            persistence_path: path,
            watch_later_directory,
            pending_progress_paths,
            persistence_error: None,
        }
    }

    pub(crate) fn entries(&self) -> &[riina_core::playback_history::Entry] {
        &self.entries
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.pending_progress_paths.clear();
        self.revision = self.revision.wrapping_add(1);
    }

    pub(crate) fn remove_paths<'a>(
        &mut self,
        paths: impl IntoIterator<Item = &'a str>,
    ) -> Result<usize, String> {
        let paths: HashSet<_> = paths.into_iter().collect();
        let mut retained = self.entries.clone();
        retained.retain(|entry| !paths.contains(entry.path.as_str()));
        let removed = self.entries.len() - retained.len();
        if removed == 0 {
            return Ok(0);
        }
        if let Some(persistence_path) = &self.persistence_path {
            crate::playback_history_store::save(persistence_path, &retained)?;
        }
        self.entries = retained;
        self.pending_progress_paths
            .retain(|path| !paths.contains(path.as_str()));
        self.revision = self.revision.wrapping_add(1);
        Ok(removed)
    }

    pub(crate) fn take_persistence_error(&mut self) -> Option<String> {
        self.persistence_error.take()
    }

    pub(crate) fn observe(&mut self, session_id: u64, playback: &Playback, record_history: bool) {
        self.refresh_saved_progress(playback);
        if !matches!(
            playback.phase,
            Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Seeking | Phase::Ended
        ) || playback.generation == 0
        {
            return;
        }
        let Some(media) = playback.playlist.iter().find(|entry| entry.current) else {
            return;
        };
        let path = playback_media_url(&media.filename).unwrap_or_else(|| media.filename.clone());
        let observed = (playback.generation, path.clone());
        if self.observed_sessions.get(&session_id) == Some(&observed) {
            // A playable snapshot can precede duration discovery. Refresh that
            // metadata without recording another visit or losing saved progress.
            if record_history
                && let Some(duration) = playback
                    .duration
                    .filter(|value| value.is_finite() && *value > 0.0)
                && let Some(entry) = self.entries.iter_mut().find(|entry| entry.path == path)
                && entry.duration != duration
            {
                entry.duration = duration;
                self.save_change();
            }
            return;
        }
        self.observed_sessions.insert(session_id, observed);
        if !record_history {
            return;
        }
        let duration = playback
            .duration
            .filter(|duration| duration.is_finite() && *duration >= 0.0)
            .unwrap_or(0.0);
        let date_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
            .unwrap_or(0);
        self.entries.retain(|entry| entry.path != path);
        self.entries.insert(
            0,
            riina_core::playback_history::Entry {
                path,
                date_ms,
                // IINA reads this from mpv's watch-later file when history is
                // restored. A new history row has no saved progress yet.
                progress: None,
                duration,
            },
        );
        self.save_change();
    }

    fn save_change(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        if let Some(path) = &self.persistence_path
            && let Err(error) = crate::playback_history_store::save(path, &self.entries)
        {
            self.persistence_error = Some(error);
        }
    }

    fn refresh_saved_progress(&mut self, playback: &Playback) {
        let (Some(directory), Some(ignore_path)) = (
            self.watch_later_directory.as_deref(),
            playback.ignore_path_in_watch_later_config,
        ) else {
            return;
        };
        if self.pending_progress_paths.is_empty() {
            return;
        }

        let paths = std::mem::take(&mut self.pending_progress_paths);
        let mut changed = false;
        for entry in &mut self.entries {
            if paths.contains(&entry.path) {
                let progress = crate::playback_history_store::progress_from_watch_later(
                    &entry.path,
                    directory,
                    ignore_path,
                );
                if entry.progress != progress {
                    entry.progress = progress;
                    changed = true;
                }
            }
        }
        if changed {
            self.save_change();
        }
    }
}

pub(crate) fn playback_media_url(value: &str) -> Option<String> {
    if Path::new(value).is_absolute() {
        return url::Url::from_file_path(value)
            .ok()
            .map(|url| url.to_string());
    }
    url::Url::parse(value).ok().map(|url| url.to_string())
}

#[cfg(test)]
mod tests;
