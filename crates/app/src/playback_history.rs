//! Application-owned playback history, shared across player windows.
pub(crate) mod progress;

use crate::playback_history_store;
use riina_core::{Phase, Playback, playback_history::Entry};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

static NEXT_HISTORY_SESSION_ID: AtomicU64 = AtomicU64::new(1);
static SHARED_PLAYBACK_HISTORIES: OnceLock<
    Mutex<HashMap<PathBuf, Arc<Mutex<PlaybackHistoryState>>>>,
> = OnceLock::new();

pub(crate) fn next_history_session_id() -> u64 {
    NEXT_HISTORY_SESSION_ID
        .fetch_add(1, Ordering::Relaxed)
        .max(1)
}

pub(crate) fn shared_playback_history_state(
    key: PathBuf,
    entries: Vec<Entry>,
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

#[derive(Debug, Default)]
pub(crate) struct PlaybackHistoryState {
    entries: Vec<Entry>,
    revision: u64,
    observed_sessions: HashMap<u64, Visit>,
    persistence_path: Option<PathBuf>,
    watch_later_directory: Option<PathBuf>,
    progress: progress::Refresh,
    persistence_error: Option<String>,
}

#[derive(Debug)]
struct Visit {
    generation: u64,
    filename: String,
    recorded_path: Option<String>,
    duration: Option<f64>,
    title: String,
}

impl PlaybackHistoryState {
    pub(crate) fn with_persistence(
        entries: Vec<Entry>,
        path: Option<PathBuf>,
        watch_later_directory: Option<PathBuf>,
    ) -> Self {
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

    pub(crate) fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn files_changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    pub(crate) fn clear(&mut self, preferences_path: &Path) -> Result<usize, String> {
        let path = playback_history_store::path_for_preferences(preferences_path);
        let removed = playback_history_store::clear(&path)?;
        self.entries.clear();
        for visit in self.observed_sessions.values_mut() {
            visit.recorded_path = None;
        }
        self.progress.cancel();
        self.revision = self.revision.wrapping_add(1);
        Ok(removed)
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
            playback_history_store::save(persistence_path, &retained)?;
        }
        self.entries = retained;
        self.revision = self.revision.wrapping_add(1);
        Ok(removed)
    }

    pub(crate) fn take_persistence_error(&mut self) -> Option<String> {
        self.persistence_error.take()
    }

    pub(crate) fn release_session(&mut self, session_id: u64) {
        self.observed_sessions.remove(&session_id);
    }

    pub(crate) fn observe(&mut self, session_id: u64, playback: &Playback, record_history: bool) {
        if !matches!(
            playback.phase,
            Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Seeking | Phase::Ended
        ) || playback.generation == 0
        {
            return;
        }
        let Some(media) = playback.playlist.iter().find(|entry| {
            playback
                .active_playlist_entry
                .map_or(entry.current, |id| entry.id == id)
        }) else {
            return;
        };
        let duration = playback
            .duration
            .filter(|value| value.is_finite() && *value > 0.0);
        if let Some(visit) = self.observed_sessions.get_mut(&session_id)
            && visit.generation == playback.generation
            && visit.filename == media.filename
        {
            let Some(path) = visit.recorded_path.as_deref().filter(|_| record_history) else {
                return;
            };
            if visit.duration == duration && visit.title == playback.title {
                return;
            }
            // Decoder metadata can arrive after the first playable snapshot.
            visit.duration = duration;
            visit.title.clone_from(&playback.title);
            if let Some(entry) = self.entries.iter_mut().find(|entry| entry.path == path) {
                let duration = duration.unwrap_or(entry.duration);
                let title = if entry.title.is_none() {
                    playback_history_title(playback, &media.filename)
                } else {
                    None
                };
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
        let recorded_path = record_history.then(|| history_path(&media.filename));
        if let Some(path) = &recorded_path {
            for visit in self.observed_sessions.values_mut() {
                if visit.recorded_path.as_ref() == Some(path) {
                    visit.recorded_path = None;
                }
            }
        }
        self.observed_sessions.insert(
            session_id,
            Visit {
                generation: playback.generation,
                filename: media.filename.clone(),
                recorded_path: recorded_path.clone(),
                duration,
                title: playback.title.clone(),
            },
        );
        let Some(path) = recorded_path else {
            return;
        };
        let ignore_path = playback
            .ignore_path_in_watch_later_config
            .unwrap_or_default();
        let watch_later_key = playback_history_store::watch_later_key(&media.filename, ignore_path);
        let date_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
            .unwrap_or(0);
        self.progress.entry_replaced(&path);
        self.entries.retain(|entry| {
            entry.path != path
                && watch_later_key
                    .as_ref()
                    .is_none_or(|key| entry.watch_later_key.as_ref() != Some(key))
        });
        self.entries.insert(
            0,
            Entry {
                path,
                title: playback_history_title(playback, &media.filename).map(str::to_owned),
                watch_later_key,
                date_ms,
                // IINA reads this from mpv's watch-later file when history is
                // restored. A new history row has no saved progress yet.
                progress: None,
                duration: duration.unwrap_or_default(),
            },
        );
        self.save_change();
    }

    fn save_change(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        if let Some(path) = &self.persistence_path
            && let Err(error) = playback_history_store::save(path, &self.entries)
        {
            self.persistence_error = Some(error);
        }
    }
}

fn history_path(filename: &str) -> String {
    if playback_history_store::mpv_url_scheme(filename)
        .is_some_and(|scheme| !scheme.eq_ignore_ascii_case("file"))
    {
        // Even equivalent network URLs can refer to separate mpv resume files.
        filename.to_owned()
    } else {
        playback_media_url(filename).unwrap_or_else(|| filename.to_owned())
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
