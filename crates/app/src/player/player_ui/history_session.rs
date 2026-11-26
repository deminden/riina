//! History recording and synchronization for one player session.
use super::{observation_dispatch::HistoryObservation, recent_activity::RecentActivity};
use crate::{
    PlaybackStore, PlayerWindow,
    playback_history::{self, PlaybackHistoryState},
    playback_history_store, playback_history_window,
    plugin_host::PluginHostHandle,
};
use riina_core::{Preferences, RecentMedia, playback_history::Entry};
use std::{
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

pub(super) struct LoadedHistory {
    path: PathBuf,
    entries: Vec<Entry>,
    write_path: Option<PathBuf>,
}

impl LoadedHistory {
    pub(super) fn load(app: &PlayerWindow, prefs_path: &Path) -> Self {
        let path = playback_history_store::path_for_preferences(prefs_path);
        let mut write_path = Some(path.clone());
        let entries = match playback_history_store::load(&path) {
            Ok(entries) => entries,
            Err(error) => {
                crate::diagnostics::report(
                    app,
                    format!("Could not load playback history: {error}"),
                );
                write_path = None;
                Vec::new()
            }
        };
        Self {
            path,
            entries,
            write_path,
        }
    }

    pub(super) fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub(super) fn start_tracking(
        self,
        prefs_path: &Path,
        prefs: &Preferences,
        recent_documents: RecentDocuments,
    ) -> Tracking {
        let session_id = playback_history::next_history_session_id();
        let state = playback_history::shared_playback_history_state(
            self.path,
            self.entries,
            self.write_path,
            Some(prefs_path.with_file_name("watch-later")),
        );
        Tracking {
            state,
            recording: Arc::new(AtomicBool::new(prefs.record_playback_history)),
            recent_documents,
            session_id,
        }
    }
}

type RecentDocuments = Arc<Mutex<Vec<RecentMedia>>>;

pub(super) struct Tracking {
    pub(super) state: Arc<Mutex<PlaybackHistoryState>>,
    pub(super) recording: Arc<AtomicBool>,
    recent_documents: RecentDocuments,
    session_id: u64,
}

impl Tracking {
    pub(super) fn observe(&self, plugin: Option<PluginHostHandle>) -> HistoryObservation {
        HistoryObservation {
            recent_documents: self.recent_documents.clone(),
            history: self.state.clone(),
            record_history: self.recording.clone(),
            session_id: self.session_id,
            plugin,
            last_plugin_context: Default::default(),
        }
    }

    pub(super) fn install_recording_action(&self, app: &PlayerWindow) {
        let record_history = self.recording.clone();
        app.on_record_playback_history_changed(move |enabled| {
            record_history.store(enabled, Ordering::Release);
        });
    }

    pub(super) fn window_context(
        &self,
        prefs_path: &Path,
        plugin: Option<PluginHostHandle>,
        latest_playback: PlaybackStore,
    ) -> (
        playback_history_window::Archive,
        playback_history_window::HistorySync,
    ) {
        let recent_documents = self.recent_documents.clone();
        let sync = Arc::new(move |history: &[Entry]| {
            sync_plugin(
                plugin.as_ref(),
                &latest_playback,
                &recent_documents,
                history,
            )
        });
        (
            playback_history_window::Archive {
                history: self.state.clone(),
                preferences_path: prefs_path.to_path_buf(),
            },
            sync,
        )
    }

    pub(super) fn clear_hook(
        &self,
        path: PathBuf,
        recent: Rc<RecentActivity>,
        history_window: Rc<playback_history_window::ApplicationHistory>,
    ) -> impl Fn() -> Result<usize, String> + Clone + 'static {
        let playback_history = self.state.clone();
        move || {
            let mut history = playback_history.lock().map_err(|_| {
                "Could not clear playback history: history is unavailable".to_owned()
            })?;
            let removed = history
                .clear(&path)
                .map_err(|error| format!("Could not clear playback history: {error}"))?;
            let entries = history.entries().to_vec();
            drop(history);
            let cleared = recent.clear_for_history();
            let synced = history_window.sync_plugins(&entries);
            cleared
                .and(synced)
                .map_err(|error| format!("Playback history cleared. {error}"))?;
            Ok(removed)
        }
    }
}

fn sync_plugin(
    plugin: Option<&PluginHostHandle>,
    latest_playback: &PlaybackStore,
    recent_documents: &RecentDocuments,
    history: &[Entry],
) -> Result<(), String> {
    let Some(plugin) = plugin else {
        return Ok(());
    };
    let playback = latest_playback
        .lock()
        .map_err(|_| "latest playback lock poisoned".to_owned())?
        .clone();
    let context = crate::plugin_snapshot::player_file_context(
        &playback,
        &recent_documents
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        history,
    );
    plugin.update_player_file_context(context)
}
