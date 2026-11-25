//! Application-owned playback history, shared across player windows.
pub(crate) mod progress;

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

#[derive(Debug, Default)]
pub(crate) struct PlaybackHistoryState {
    entries: Vec<riina_core::playback_history::Entry>,
    revision: u64,
    observed_sessions: HashMap<u64, (u64, String)>,
    persistence_path: Option<PathBuf>,
    watch_later_directory: Option<PathBuf>,
    progress: progress::Refresh,
    persistence_error: Option<String>,
}

impl PlaybackHistoryState {
    pub(crate) fn with_persistence(
        mut entries: Vec<riina_core::playback_history::Entry>,
        path: Option<PathBuf>,
        watch_later_directory: Option<PathBuf>,
    ) -> Self {
        if watch_later_directory.is_some() {
            // The archive may contain older cached positions. mpv owns their current values.
            for entry in &mut entries {
                entry.progress = None;
            }
        }
        Self {
            entries,
            revision: 1,
            observed_sessions: HashMap::new(),
            persistence_path: path,
            watch_later_directory,
            progress: progress::Refresh::default(),
            persistence_error: None,
        }
    }

    pub(crate) fn entries(&self) -> &[riina_core::playback_history::Entry] {
        &self.entries
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn files_changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.progress.cancel();
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
        self.revision = self.revision.wrapping_add(1);
        Ok(removed)
    }

    pub(crate) fn take_persistence_error(&mut self) -> Option<String> {
        self.persistence_error.take()
    }

    pub(crate) fn observe(&mut self, session_id: u64, playback: &Playback, record_history: bool) {
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
            // Decoder metadata can arrive after the playable snapshot.
            if record_history
                && let Some(entry) = self.entries.iter_mut().find(|entry| entry.path == path)
            {
                let duration = playback
                    .duration
                    .filter(|value| value.is_finite() && *value > 0.0)
                    .unwrap_or(entry.duration);
                let title = entry
                    .title
                    .is_none()
                    .then(|| playback_history_title(playback, &media.filename))
                    .flatten();
                if entry.duration != duration || title.is_some() {
                    entry.duration = duration;
                    if let Some(title) = title {
                        entry.title = Some(title.to_owned());
                    }
                    self.save_change();
                }
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
        self.progress.entry_replaced(&path);
        self.entries.retain(|entry| entry.path != path);
        self.entries.insert(
            0,
            riina_core::playback_history::Entry {
                path,
                title: playback_history_title(playback, &media.filename).map(str::to_owned),
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
}

fn playback_history_title<'a>(playback: &'a Playback, reference: &str) -> Option<&'a str> {
    let title = playback.title.as_str();
    if title.trim().is_empty() || title == reference {
        return None;
    }
    let is_filename = if let Some(path) = crate::file_actions::local_media_path(reference) {
        path.file_name().is_some_and(|name| name == title)
    } else {
        url::Url::parse(reference).ok().is_some_and(|url| {
            url.path_segments()
                .and_then(Iterator::last)
                .is_some_and(|encoded| {
                    encoded == title
                        || percent_encoding::percent_decode_str(encoded)
                            .decode_utf8()
                            .is_ok_and(|name| name == title)
                })
        })
    };
    (!is_filename).then_some(title)
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
