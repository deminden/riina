//! History migration, recording and synchronization for one player session.
use super::observation_dispatch::HistoryObservation;
use crate::{
    PlaybackStore, PlayerWindow,
    playback_history::{self, PlaybackHistoryState},
    playback_history_store, playback_history_window,
    plugin_host::PluginHostHandle,
};
use riina_core::{Preferences, RecentMedia, playback_history::Entry};
use riina_mpv::CommandSender;
use std::{
    path::{Path, PathBuf},
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
    pub(super) fn load(app: &PlayerWindow, prefs_path: &Path, prefs: &mut Preferences) -> Self {
        let path = playback_history_store::path_for_preferences(prefs_path);
        let mut write_path = Some(path.clone());
        let entries = match playback_history_store::load(&path, &prefs.playback_history) {
            Ok(entries) => {
                prefs.playback_history.clone_from(&entries);
                if !path.exists()
                    && let Err(error) = playback_history_store::save(&path, &entries)
                {
                    app.set_status(format!("Could not migrate playback history: {error}").into());
                    write_path = None;
                }
                entries
            }
            Err(error) => {
                app.set_status(format!("Could not load playback history: {error}").into());
                write_path = None;
                prefs.playback_history.clone()
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

    pub(super) fn start_tracking(self, prefs_path: &Path, prefs: &Preferences) -> Tracking {
        let plugin_recent_media = Arc::new(Mutex::new(prefs.recent_media.clone()));
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
            cleared: playback_history::shared_history_merge_flag(prefs_path),
            plugin_recent_media,
            session_id,
        }
    }
}

type PluginRecentMedia = Arc<Mutex<Vec<RecentMedia>>>;

pub(super) struct Tracking {
    pub(super) state: Arc<Mutex<PlaybackHistoryState>>,
    pub(super) recording: Arc<AtomicBool>,
    pub(super) cleared: Arc<AtomicBool>,
    plugin_recent_media: PluginRecentMedia,
    session_id: u64,
}

impl Tracking {
    pub(super) fn observe(&self, plugin: Option<PluginHostHandle>) -> HistoryObservation {
        HistoryObservation {
            recent_media: self.plugin_recent_media.clone(),
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

    pub(super) fn install_window(
        &self,
        app: &PlayerWindow,
        player: &CommandSender,
        prefs_path: &Path,
        plugin: Option<PluginHostHandle>,
        latest_playback: PlaybackStore,
    ) -> playback_history_window::Guard {
        let recent_media = self.plugin_recent_media.clone();
        let sync = Arc::new(move |history: Vec<Entry>| {
            sync_plugin(plugin.as_ref(), &latest_playback, &recent_media, &history)
        });
        playback_history_window::install(
            app,
            self.state.clone(),
            player.clone(),
            prefs_path.to_path_buf(),
            self.cleared.clone(),
            sync,
        )
    }

    pub(super) fn clear_hook(
        &self,
        path: PathBuf,
        plugin: Option<PluginHostHandle>,
        latest_playback: PlaybackStore,
    ) -> impl Fn() -> Result<usize, String> + Clone + 'static {
        let playback_history = self.state.clone();
        let history_cleared = self.cleared.clone();
        let recent_media = self.plugin_recent_media.clone();
        move || {
            let mut history = playback_history
                .lock()
                .map_err(|_| "playback history lock poisoned".to_owned())?;
            let removed = crate::utilities_preferences::clear_playback_history(&path)?;
            history.clear();
            history_cleared.store(true, Ordering::Release);
            let entries = history.entries().to_vec();
            drop(history);
            sync_plugin(plugin.as_ref(), &latest_playback, &recent_media, &entries)?;
            Ok(removed)
        }
    }
}

fn sync_plugin(
    plugin: Option<&PluginHostHandle>,
    latest_playback: &PlaybackStore,
    recent_media: &PluginRecentMedia,
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
        &mut recent_media
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        history,
    );
    plugin.update_player_file_context(context)
}
