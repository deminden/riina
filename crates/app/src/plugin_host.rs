//! Application-lifetime owner for enabled plugin entry sessions.

use crate::{
    plugin_files::PluginPlayerFileContext,
    plugin_runtime::{HostEvent, PluginRuntimeSession},
};
use riina_core::{
    Command, Phase, Playback, TrackKind,
    plugin::{Manifest, OverlayEffect, WindowAction},
};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::Duration,
};

impl PluginPlayerFileContext {
    fn from_playback(playback: &Playback) -> Self {
        let current_media = playback
            .playlist
            .iter()
            .find(|entry| entry.current)
            .map(|entry| entry.filename.clone());
        let current_directory = current_media
            .as_deref()
            .and_then(|filename| Path::new(filename).parent())
            .filter(|path| path.is_absolute())
            .map(Path::to_path_buf);
        let external_tracks = playback
            .tracks
            .iter()
            .filter_map(|track| {
                let alias = match track.kind {
                    TrackKind::Video => "@video",
                    TrackKind::Audio => "@audio",
                    TrackKind::Sub => "@sub",
                };
                let path = track.external_filename.as_deref()?;
                Some((format!("{alias}/{}", track.id), PathBuf::from(path)))
            })
            .collect();
        let video = playback.video.as_ref();
        let is_network_resource = current_media.as_deref().is_some_and(|value| {
            riina_core::media_reference::is_network_url_candidate(value)
                || url::Url::parse(value).is_ok_and(|url| url.scheme() == "smb")
        });
        let properties = BTreeMap::from([
            ("paused".into(), serde_json::json!(playback.paused)),
            (
                "idle".into(),
                serde_json::json!(current_media.is_none() || playback.phase == Phase::Stopped),
            ),
            ("position".into(), finite_json(playback.position)),
            ("duration".into(), finite_json(playback.duration)),
            ("speed".into(), finite_json(Some(playback.speed))),
            (
                "videoWidth".into(),
                video.map_or(serde_json::Value::Null, |video| serde_json::json!(video.w)),
            ),
            (
                "videoHeight".into(),
                video.map_or(serde_json::Value::Null, |video| serde_json::json!(video.h)),
            ),
            (
                "isNetworkResource".into(),
                serde_json::json!(is_network_resource),
            ),
            (
                "url".into(),
                current_media
                    .as_ref()
                    .and_then(|value| playback_media_url(value))
                    .map_or(serde_json::Value::Null, |value| serde_json::json!(value)),
            ),
            ("title".into(), serde_json::json!(playback.title.as_str())),
            (
                "audio.id".into(),
                selected_track_id(playback, TrackKind::Audio, 0),
            ),
            (
                "audio.delay".into(),
                serde_json::json!(playback.audio_delay),
            ),
            ("audio.volume".into(), serde_json::json!(playback.volume)),
            ("audio.muted".into(), serde_json::json!(playback.muted)),
            (
                "audio.tracks".into(),
                serialized_tracks(playback, TrackKind::Audio),
            ),
            (
                "audio.currentTrack".into(),
                current_track(playback, TrackKind::Audio, 0),
            ),
            (
                "subtitle.id".into(),
                selected_track_id(playback, TrackKind::Sub, 0),
            ),
            (
                "subtitle.secondID".into(),
                selected_track_id(playback, TrackKind::Sub, 1),
            ),
            (
                "subtitle.delay".into(),
                serde_json::json!(playback.subtitle_delay),
            ),
            (
                "subtitle.tracks".into(),
                serialized_tracks(playback, TrackKind::Sub),
            ),
            (
                "subtitle.currentTrack".into(),
                current_track(playback, TrackKind::Sub, 0),
            ),
            (
                "video.id".into(),
                selected_track_id(playback, TrackKind::Video, 0),
            ),
            (
                "video.tracks".into(),
                serialized_tracks(playback, TrackKind::Video),
            ),
            (
                "video.currentTrack".into(),
                current_track(playback, TrackKind::Video, 0),
            ),
        ]);
        let chapters = playback
            .chapters
            .iter()
            .map(|chapter| {
                serde_json::json!({
                    "title": chapter.title,
                    "start": finite_json(Some(chapter.time)),
                })
            })
            .collect();
        Self::new(current_directory, external_tracks).with_core_snapshot(properties, chapters)
    }
}

fn selected_track_id(playback: &Playback, kind: TrackKind, lane: i64) -> serde_json::Value {
    playback
        .tracks
        .iter()
        .find(|track| {
            track.kind == kind && track.selected && track.main_selection.unwrap_or(0) == lane
        })
        .map_or(serde_json::Value::Null, |track| serde_json::json!(track.id))
}

fn current_track(playback: &Playback, kind: TrackKind, lane: i64) -> serde_json::Value {
    playback
        .tracks
        .iter()
        .find(|track| {
            track.kind == kind && track.selected && track.main_selection.unwrap_or(0) == lane
        })
        .map_or(serde_json::Value::Null, track_to_json)
}

fn serialized_tracks(playback: &Playback, kind: TrackKind) -> serde_json::Value {
    serde_json::Value::Array(
        playback
            .tracks
            .iter()
            .filter(|track| track.kind == kind)
            .map(track_to_json)
            .collect(),
    )
}

fn track_to_json(track: &riina_core::Track) -> serde_json::Value {
    serde_json::json!({
        "id": track.id,
        "title": track.title,
        "formattedTitle": formatted_track_title(track),
        "lang": track.lang,
        "codec": track.codec,
        "isDefault": track.is_default,
        "isForced": track.forced,
        "isSelected": track.selected,
        "isExternal": track.external,
        "demuxW": track.width,
        "demuxH": track.height,
        "demuxChannelCount": track.channels,
        "demuxChannels": track.channel_layout,
        "demuxSamplerate": track.sample_rate,
        "demuxFPS": track.frame_rate,
    })
}

fn formatted_track_title(track: &riina_core::Track) -> String {
    let language = track
        .lang
        .as_deref()
        .filter(|language| *language != "und")
        .map(|language| {
            crate::language_names::LanguageNames::current().display_name(Some(language))
        })
        .filter(|language| !language.is_empty())
        .map_or_else(String::new, |language| format!("[{language}] "));
    let mut details = track.codec.iter().cloned().collect::<Vec<_>>();
    match track.kind {
        TrackKind::Video => {
            if let Some((width, height)) = track.width.zip(track.height) {
                details.push(format!("{width}×{height}"));
            }
            if let Some(rate) = track.frame_rate {
                details.push(format!("{}fps", track_number(rate)));
            }
        }
        TrackKind::Audio => {
            if let Some(channels) = track.channels {
                details.push(format!("{channels}ch"));
            }
            if let Some(rate) = track.sample_rate {
                details.push(format!("{}kHz", track_number(rate as f64 / 1000.0)));
            }
        }
        TrackKind::Sub => {}
    }
    if track.is_default {
        details.push("Default".into());
    }
    let title = track.title.as_deref().unwrap_or("No Title");
    let details = if details.is_empty() {
        String::new()
    } else {
        format!(" ({})", details.join(", "))
    };
    format!("#{} {language}{title}{details}", track.id)
}

fn track_number(value: f64) -> String {
    let rounded = (value * 1000.0).round() / 1000.0;
    if rounded == 0.0 {
        "0".into()
    } else {
        rounded.to_string()
    }
}

fn playback_media_url(value: &str) -> Option<String> {
    if Path::new(value).is_absolute() {
        return url::Url::from_file_path(value)
            .ok()
            .map(|url| url.to_string());
    }
    if riina_core::media_reference::is_network_url_candidate(value)
        || url::Url::parse(value).is_ok_and(|url| url.scheme() == "smb")
    {
        return url::Url::parse(value).ok().map(|url| url.to_string());
    }
    url::Url::parse(value).ok().map(|url| url.to_string())
}

fn finite_json(value: Option<f64>) -> serde_json::Value {
    value
        .filter(|value| value.is_finite())
        .map_or(serde_json::Value::Null, |value| serde_json::json!(value))
}

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

#[cfg(test)]
pub(crate) fn player_file_context(
    playback: &Playback,
    recent_media: &mut Vec<riina_core::RecentMedia>,
) -> PluginPlayerFileContext {
    player_file_context_with_history(
        playback,
        recent_media,
        &mut PlaybackHistoryState::default(),
        false,
        0,
    )
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

    pub(crate) fn remove_path(&mut self, path: &str) -> Result<bool, String> {
        let mut retained = self.entries.clone();
        retained.retain(|entry| entry.path != path);
        if retained.len() == self.entries.len() {
            return Ok(false);
        }
        if let Some(persistence_path) = &self.persistence_path {
            crate::playback_history_store::save(persistence_path, &retained)?;
        }
        self.entries = retained;
        self.pending_progress_paths.remove(path);
        self.revision = self.revision.wrapping_add(1);
        Ok(true)
    }

    pub(crate) fn take_persistence_error(&mut self) -> Option<String> {
        self.persistence_error.take()
    }

    fn observe(
        &mut self,
        session_id: u64,
        playback: &Playback,
        _recent_media: &[riina_core::RecentMedia],
        record_history: bool,
    ) {
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
        if changed
            && let Some(path) = &self.persistence_path
            && let Err(error) = crate::playback_history_store::save(path, &self.entries)
        {
            self.persistence_error = Some(error);
        }
        if changed {
            self.revision = self.revision.wrapping_add(1);
        }
    }
}

pub(crate) fn player_file_context_with_history(
    playback: &Playback,
    recent_media: &mut Vec<riina_core::RecentMedia>,
    history: &mut PlaybackHistoryState,
    record_history: bool,
    history_session_id: u64,
) -> PluginPlayerFileContext {
    history.observe(history_session_id, playback, recent_media, record_history);
    if !matches!(playback.phase, Phase::Loading | Phase::Failed)
        && let Some(entry) = playback.playlist.iter().find(|entry| entry.current)
    {
        let position = playback
            .position
            .filter(|position| position.is_finite() && *position >= 0.0)
            .or_else(|| {
                recent_media
                    .iter()
                    .find(|media| media.path == entry.filename)
                    .map(|media| media.position)
            })
            .unwrap_or(0.0);
        let title = entry
            .title
            .as_ref()
            .filter(|title| !title.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| {
                if playback.title.trim().is_empty() {
                    Path::new(&entry.filename).file_name().map_or_else(
                        || entry.filename.clone(),
                        |name| name.to_string_lossy().into(),
                    )
                } else {
                    playback.title.clone()
                }
            });
        recent_media.retain(|media| media.path != entry.filename);
        recent_media.insert(
            0,
            riina_core::RecentMedia {
                path: entry.filename.clone(),
                title,
                position,
            },
        );
        recent_media.truncate(10);
    }
    PluginPlayerFileContext::from_playback(playback)
        .with_core_version(version_snapshot(playback.mpv_version.as_deref()))
        .with_recent_documents(recent_document_values(recent_media))
        .with_playback_history(playback_history_values(history.entries()))
}

fn version_snapshot(mpv_version: Option<&str>) -> serde_json::Value {
    serde_json::json!({
        "iina": env!("CARGO_PKG_VERSION"),
        "build": option_env!("RIINA_BUILD_NUMBER").unwrap_or("development"),
        "mpv": mpv_version,
    })
}

pub(crate) fn recent_document_values(
    recent_media: &[riina_core::RecentMedia],
) -> Vec<serde_json::Value> {
    recent_media
        .iter()
        .filter_map(|media| {
            let url = playback_media_url(&media.path)?;
            let parsed = url::Url::parse(&url).ok()?;
            let name = media_url_name(&parsed)?;
            Some(serde_json::json!({"name": name, "url": url}))
        })
        .collect()
}

pub(crate) fn playback_history_values(
    entries: &[riina_core::playback_history::Entry],
) -> Vec<serde_json::Value> {
    entries
        .iter()
        .filter_map(|entry| {
            let url = playback_media_url(&entry.path)?;
            let parsed = url::Url::parse(&url).ok()?;
            let name = media_url_name(&parsed)?;
            Some(serde_json::json!({
                "name": name,
                "url": url,
                "date": entry.date_ms,
                "progress": finite_json(entry.progress),
                "duration": entry.duration,
            }))
        })
        .collect()
}

fn media_url_name(url: &url::Url) -> Option<String> {
    url.to_file_path()
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .or_else(|| {
            url.path_segments()
                .and_then(Iterator::last)
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
        })
}

const MAX_PENDING_MESSAGES: usize = 256;
// Timers and cross-context callbacks produce events while no host command arrives.
const HOST_EVENT_POLL_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PluginHostMessage {
    Log(String),
    ShowOsd(String),
    ShowAlert(String, String),
    Status(String),
    PlayerCommand(Command),
    SetUiVisibility(bool),
    WindowAction(WindowAction),
    Overlay {
        plugin_identifier: String,
        plugin_directory: PathBuf,
        effect: OverlayEffect,
    },
    RemoveOverlay(String),
}

#[derive(Clone)]
pub(crate) struct PluginHostHandle {
    commands: mpsc::SyncSender<PluginCommand>,
    messages: Arc<Mutex<Vec<PluginHostMessage>>>,
}

impl PluginHostHandle {
    pub(crate) fn enable(&self, manifest: Manifest, directory: PathBuf) -> Result<(), String> {
        self.commands
            .try_send(PluginCommand::Enable {
                manifest: Box::new(manifest),
                directory,
            })
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => "Plugin host is busy".to_owned(),
                mpsc::TrySendError::Disconnected(_) => "Plugin host has stopped".to_owned(),
            })
    }

    pub(crate) fn disable(&self, identifier: String) -> Result<(), String> {
        self.commands
            .try_send(PluginCommand::Disable(identifier))
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => "Plugin host is busy".to_owned(),
                mpsc::TrySendError::Disconnected(_) => "Plugin host has stopped".to_owned(),
            })
    }

    pub(crate) fn update_player_file_context(
        &self,
        context: PluginPlayerFileContext,
    ) -> Result<(), String> {
        self.commands
            .try_send(PluginCommand::UpdatePlayerFileContext(context))
            .map_err(command_error)
    }

    pub(crate) fn update_window_properties(
        &self,
        properties: BTreeMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        self.commands
            .try_send(PluginCommand::UpdateWindowProperties(properties))
            .map_err(command_error)
    }

    pub(crate) fn suspend_for_update(&self, identifier: String) -> Result<(), String> {
        let (response, result) = mpsc::sync_channel(1);
        self.commands
            .try_send(PluginCommand::SuspendForUpdate {
                identifier,
                response,
            })
            .map_err(command_error)?;
        result
            .recv_timeout(Duration::from_secs(30))
            .map_err(|error| format!("Plugin host did not stop the plugin for update: {error}"))?
    }

    pub(crate) fn activate_after_update(
        &self,
        manifest: Manifest,
        directory: PathBuf,
    ) -> Result<(), String> {
        let (response, result) = mpsc::sync_channel(1);
        self.commands
            .try_send(PluginCommand::ActivateAfterUpdate {
                manifest: Box::new(manifest),
                directory,
                response,
            })
            .map_err(command_error)?;
        result
            .recv_timeout(Duration::from_secs(30))
            .map_err(|error| format!("Plugin host did not restart the updated plugin: {error}"))?
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn deliver_overlay_message(
        &self,
        identifier: String,
        name: String,
        data: serde_json::Value,
    ) -> Result<(), String> {
        self.commands
            .try_send(PluginCommand::DeliverOverlayMessage {
                identifier,
                name,
                data,
            })
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => "Plugin host is busy".to_owned(),
                mpsc::TrySendError::Disconnected(_) => "Plugin host has stopped".to_owned(),
            })
    }

    pub(crate) fn drain_messages(&self) -> Result<Vec<PluginHostMessage>, String> {
        let mut messages = self
            .messages
            .lock()
            .map_err(|_| "Plugin host message queue was poisoned".to_owned())?;
        Ok(std::mem::take(&mut *messages))
    }
}

pub(crate) struct PluginHost {
    handle: PluginHostHandle,
    worker: Option<JoinHandle<()>>,
}

pub(crate) fn is_enabled_plugin(preferences_path: &Path, identifier: &str) -> bool {
    enabled_packages(
        &preferences_path.with_file_name("Plugins"),
        &preferences_path.with_file_name("plugin-enabled.json"),
    )
    .iter()
    .any(|plugin| plugin.manifest.identifier == identifier)
}

impl PluginHost {
    pub(crate) fn start(
        preferences_path: &Path,
        recent_media: &[riina_core::RecentMedia],
        playback_history: &[riina_core::playback_history::Entry],
    ) -> Result<Self, String> {
        let plugins_directory = preferences_path.with_file_name("Plugins");
        let permissions_path = preferences_path.with_file_name("plugin-permissions.json");
        let enabled_path = preferences_path.with_file_name("plugin-enabled.json");
        let (commands, receiver) = mpsc::sync_channel(64);
        let messages = Arc::new(Mutex::new(Vec::new()));
        let worker_messages = messages.clone();
        let initial_player_context = PluginPlayerFileContext::default()
            .with_core_version(version_snapshot(None))
            .with_recent_documents(recent_document_values(recent_media))
            .with_playback_history(playback_history_values(playback_history));
        let worker = std::thread::Builder::new()
            .name("riina-plugin-host".into())
            .spawn(move || {
                run_host(
                    plugins_directory,
                    permissions_path,
                    enabled_path,
                    worker_messages,
                    initial_player_context,
                    receiver,
                );
            })
            .map_err(|error| format!("Could not start plugin host: {error}"))?;
        Ok(Self {
            handle: PluginHostHandle { commands, messages },
            worker: Some(worker),
        })
    }

    pub(crate) fn handle(&self) -> PluginHostHandle {
        self.handle.clone()
    }
}

impl Drop for PluginHost {
    fn drop(&mut self) {
        let _ = self.handle.commands.send(PluginCommand::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

enum PluginCommand {
    Enable {
        manifest: Box<Manifest>,
        directory: PathBuf,
    },
    Disable(String),
    SuspendForUpdate {
        identifier: String,
        response: mpsc::SyncSender<Result<(), String>>,
    },
    ActivateAfterUpdate {
        manifest: Box<Manifest>,
        directory: PathBuf,
        response: mpsc::SyncSender<Result<(), String>>,
    },
    UpdatePlayerFileContext(PluginPlayerFileContext),
    UpdateWindowProperties(BTreeMap<String, serde_json::Value>),
    #[cfg(target_os = "windows")]
    DeliverOverlayMessage {
        identifier: String,
        name: String,
        data: serde_json::Value,
    },
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PluginInstanceKey {
    identifier: String,
    global: bool,
}

impl PluginInstanceKey {
    fn player(identifier: &str) -> Self {
        Self {
            identifier: identifier.to_owned(),
            global: false,
        }
    }

    fn global(identifier: &str) -> Self {
        Self {
            identifier: identifier.to_owned(),
            global: true,
        }
    }
}

fn run_host(
    plugins_directory: PathBuf,
    permissions_path: PathBuf,
    enabled_path: PathBuf,
    messages: Arc<Mutex<Vec<PluginHostMessage>>>,
    initial_player_context: PluginPlayerFileContext,
    commands: mpsc::Receiver<PluginCommand>,
) {
    let mut sessions = BTreeMap::<PluginInstanceKey, PluginRuntimeSession>::new();
    let mut plugin_directories = BTreeMap::<String, PathBuf>::new();
    let mut player_context = initial_player_context;
    let mut deferred = Vec::new();
    for plugin in enabled_packages(&plugins_directory, &enabled_path) {
        loop {
            match commands.try_recv() {
                Ok(PluginCommand::Shutdown) => return,
                Ok(PluginCommand::UpdatePlayerFileContext(context)) => {
                    player_context = context;
                }
                Ok(PluginCommand::UpdateWindowProperties(properties)) => {
                    player_context.set_core_window_properties(properties);
                }
                Ok(command) => deferred.push(command),
                Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected) => break,
            }
        }
        if let Err(error) = activate(
            &plugin.manifest,
            &plugin.directory,
            &permissions_path,
            &messages,
            &mut sessions,
            &mut plugin_directories,
            &player_context,
        ) {
            record(&messages, PluginHostMessage::Status(error));
        }
        pump_session_events(&messages, &mut sessions, &plugin_directories);
    }
    for command in deferred {
        if handle_command(
            command,
            &permissions_path,
            &messages,
            &mut sessions,
            &mut plugin_directories,
            &mut player_context,
        ) {
            return;
        }
    }

    loop {
        match commands.recv_timeout(HOST_EVENT_POLL_INTERVAL) {
            Ok(command) => {
                if handle_command(
                    command,
                    &permissions_path,
                    &messages,
                    &mut sessions,
                    &mut plugin_directories,
                    &mut player_context,
                ) {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        pump_session_events(&messages, &mut sessions, &plugin_directories);
    }
}

fn handle_command(
    command: PluginCommand,
    permissions_path: &Path,
    messages: &Arc<Mutex<Vec<PluginHostMessage>>>,
    sessions: &mut BTreeMap<PluginInstanceKey, PluginRuntimeSession>,
    plugin_directories: &mut BTreeMap<String, PathBuf>,
    player_context: &mut PluginPlayerFileContext,
) -> bool {
    match command {
        PluginCommand::Enable {
            manifest,
            directory,
        } => {
            if let Err(error) = activate(
                &manifest,
                &directory,
                permissions_path,
                messages,
                sessions,
                plugin_directories,
                player_context,
            ) {
                record(messages, PluginHostMessage::Status(error));
            }
        }
        PluginCommand::Disable(identifier) => {
            sessions.retain(|key, _| key.identifier != identifier);
            plugin_directories.remove(&identifier);
            record(messages, PluginHostMessage::RemoveOverlay(identifier));
        }
        PluginCommand::SuspendForUpdate {
            identifier,
            response,
        } => {
            sessions.retain(|key, _| key.identifier != identifier);
            plugin_directories.remove(&identifier);
            record(messages, PluginHostMessage::RemoveOverlay(identifier));
            let _ = response.send(Ok(()));
        }
        PluginCommand::ActivateAfterUpdate {
            manifest,
            directory,
            response,
        } => {
            let result = activate(
                &manifest,
                &directory,
                permissions_path,
                messages,
                sessions,
                plugin_directories,
                player_context,
            );
            if let Err(error) = &result {
                record(messages, PluginHostMessage::Status(error.clone()));
            }
            let _ = response.send(result);
        }
        PluginCommand::UpdatePlayerFileContext(context) => {
            *player_context = context;
            for (key, session) in sessions.iter() {
                if !key.global {
                    session.update_player_file_context(player_context.clone());
                }
            }
        }
        PluginCommand::UpdateWindowProperties(properties) => {
            player_context.set_core_window_properties(properties);
            for (key, session) in sessions.iter() {
                if !key.global {
                    session.update_player_file_context(player_context.clone());
                }
            }
        }
        #[cfg(target_os = "windows")]
        PluginCommand::DeliverOverlayMessage {
            identifier,
            name,
            data,
        } => {
            if let Some(session) = sessions.get(&PluginInstanceKey::player(&identifier))
                && let Err(error) = session.deliver_overlay_message(name, data)
            {
                record(messages, PluginHostMessage::Status(error));
            }
        }
        PluginCommand::Shutdown => return true,
    }
    pump_session_events(messages, sessions, plugin_directories);
    false
}

fn command_error(error: mpsc::TrySendError<PluginCommand>) -> String {
    match error {
        mpsc::TrySendError::Full(_) => "Plugin host is busy".to_owned(),
        mpsc::TrySendError::Disconnected(_) => "Plugin host has stopped".to_owned(),
    }
}

fn pump_session_events(
    messages: &Arc<Mutex<Vec<PluginHostMessage>>>,
    sessions: &mut BTreeMap<PluginInstanceKey, PluginRuntimeSession>,
    plugin_directories: &BTreeMap<String, PathBuf>,
) {
    let keys = sessions.keys().cloned().collect::<Vec<_>>();
    for key in keys {
        let Some(session) = sessions.get(&key) else {
            continue;
        };
        let events = match session.drain_events() {
            Ok(events) => events,
            Err(error) => {
                record(messages, PluginHostMessage::Status(error));
                continue;
            }
        };
        for event in events {
            match event {
                HostEvent::PluginMessage(name, data) => {
                    let peer_key = if key.global {
                        PluginInstanceKey::player(&key.identifier)
                    } else {
                        PluginInstanceKey::global(&key.identifier)
                    };
                    if let Some(peer) = sessions.get(&peer_key)
                        && let Err(error) = peer.deliver_plugin_message(name, data)
                    {
                        record(messages, PluginHostMessage::Status(error));
                    }
                }
                HostEvent::Overlay(effect) => {
                    if let Some(plugin_directory) = plugin_directories.get(&key.identifier) {
                        record(
                            messages,
                            PluginHostMessage::Overlay {
                                plugin_identifier: key.identifier.clone(),
                                plugin_directory: plugin_directory.clone(),
                                effect,
                            },
                        );
                    }
                }
                event => record(messages, host_message(&key.identifier, event)),
            }
        }
    }
}

struct InstalledPlugin {
    manifest: Manifest,
    directory: PathBuf,
}

fn enabled_packages(directory: &Path, enabled_path: &Path) -> Vec<InstalledPlugin> {
    let enabled = fs::read_to_string(enabled_path)
        .ok()
        .and_then(|text| serde_json::from_str::<BTreeSet<String>>(&text).ok())
        .unwrap_or_default();
    let mut plugins = fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            (path
                .extension()
                .is_some_and(|extension| extension == "iinaplugin"))
            .then(|| {
                Manifest::load_from_directory(&path)
                    .ok()
                    .map(|manifest| (path, manifest))
            })
            .flatten()
        })
        .filter(|(_, manifest)| enabled.contains(&manifest.identifier))
        .map(|(directory, manifest)| InstalledPlugin {
            manifest,
            directory,
        })
        .collect::<Vec<_>>();
    plugins.sort_by(|left, right| left.manifest.identifier.cmp(&right.manifest.identifier));
    plugins
}

fn activate(
    manifest: &Manifest,
    directory: &Path,
    permissions_path: &Path,
    messages: &Arc<Mutex<Vec<PluginHostMessage>>>,
    sessions: &mut BTreeMap<PluginInstanceKey, PluginRuntimeSession>,
    plugin_directories: &mut BTreeMap<String, PathBuf>,
    player_context: &PluginPlayerFileContext,
) -> Result<(), String> {
    let player_key = PluginInstanceKey::player(&manifest.identifier);
    if sessions.contains_key(&player_key) {
        return Ok(());
    }
    if !crate::plugin_preferences::current_runtime_supports(manifest) {
        return Err(format!(
            "{} is enabled, but requests plugin capabilities that are unavailable",
            manifest.name
        ));
    }
    let grants = load_grants(permissions_path, &manifest.identifier)?;
    let global = manifest
        .global_entry
        .as_ref()
        .map(|_| {
            PluginRuntimeSession::start_package_global_entry(directory, manifest, &grants)
                .map_err(|error| format!("Could not start global {}: {error}", manifest.name))
        })
        .transpose()?;
    let session = PluginRuntimeSession::start_package_entry_with_player_context(
        directory,
        manifest,
        &grants,
        player_context.clone(),
    )
    .map_err(|error| format!("Could not start {}: {error}", manifest.name))?;
    if let Some(global) = global {
        sessions.insert(PluginInstanceKey::global(&manifest.identifier), global);
    }
    sessions.insert(player_key, session);
    plugin_directories.insert(manifest.identifier.clone(), directory.to_path_buf());
    record(
        messages,
        PluginHostMessage::Status(format!("{} is active.", manifest.name)),
    );
    Ok(())
}

fn load_grants(path: &Path, identifier: &str) -> Result<Vec<String>, String> {
    let grants: BTreeMap<String, Vec<String>> = match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map_err(|error| error.to_string())?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
        Err(error) => return Err(error.to_string()),
    };
    Ok(grants.get(identifier).cloned().unwrap_or_default())
}

fn host_message(plugin_name: &str, event: HostEvent) -> PluginHostMessage {
    match event {
        HostEvent::Log(message) => PluginHostMessage::Log(format!("{plugin_name}: {message}")),
        HostEvent::ShowOsd(message) => PluginHostMessage::ShowOsd(message),
        HostEvent::ShowAlert(message) => {
            PluginHostMessage::ShowAlert(plugin_name.to_owned(), message)
        }
        HostEvent::CoreCommand(command) => PluginHostMessage::PlayerCommand(command),
        HostEvent::SetUiVisibility(visible) => PluginHostMessage::SetUiVisibility(visible),
        HostEvent::WindowAction(action) => PluginHostMessage::WindowAction(action),
        HostEvent::Overlay(_) => unreachable!("overlay effects carry plugin package identity"),
        HostEvent::PluginMessage(_, _) => {
            unreachable!("plugin messages are routed to their paired runtime")
        }
    }
}

fn record(messages: &Mutex<Vec<PluginHostMessage>>, message: PluginHostMessage) {
    if let Ok(mut messages) = messages.lock()
        && messages.len() < MAX_PENDING_MESSAGES
    {
        messages.push(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_files::PluginFiles;
    use riina_core::{Chapter, PlaylistEntry, Track, TrackKind};

    #[test]
    fn playback_history_records_a_loaded_media_once_per_generation() {
        let mut state = PlaybackHistoryState::default();
        let media = "https://media.example/sample.mp4";
        let mut playback = Playback {
            generation: 1,
            phase: Phase::Playing,
            position: Some(15.0),
            duration: Some(80.0),
            ..Playback::default()
        };
        playback.playlist.push(PlaylistEntry {
            id: 1,
            filename: media.into(),
            title: None,
            current: true,
            playing: true,
        });

        let recent_media = [riina_core::RecentMedia {
            path: media.into(),
            title: "Sample".into(),
            position: 12.0,
        }];
        state.observe(1, &playback, &recent_media, true);
        let first = state.entries().first().unwrap().clone();
        assert_eq!(first.path, media);
        assert_eq!(first.progress, None);
        assert_eq!(first.duration, 80.0);
        assert!(first.date_ms > 0);

        playback.position = Some(42.0);
        state.observe(1, &playback, &recent_media, true);
        assert_eq!(state.entries(), std::slice::from_ref(&first));

        playback.generation = 2;
        state.observe(1, &playback, &recent_media, true);
        assert_eq!(state.entries().len(), 1);
        assert_eq!(state.entries()[0].path, media);
        assert!(state.entries()[0].date_ms >= first.date_ms);
    }

    #[test]
    fn playback_history_is_saved_when_a_media_generation_is_first_observed() {
        let path = std::env::temp_dir().join(format!(
            "riina-history-observed-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut state =
            PlaybackHistoryState::with_persistence(Vec::new(), Some(path.clone()), None);
        let mut playback = Playback {
            generation: 1,
            phase: Phase::Playing,
            duration: Some(90.0),
            ..Playback::default()
        };
        playback.playlist.push(PlaylistEntry {
            id: 1,
            filename: "C:/media/movie.mkv".into(),
            title: None,
            current: true,
            playing: true,
        });

        player_file_context_with_history(&playback, &mut Vec::new(), &mut state, true, 1);

        let saved = crate::playback_history_store::load(&path, &[]).unwrap();
        assert_eq!(saved, state.entries());
        assert_eq!(saved.len(), 1);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn restored_playback_history_reads_progress_from_mpv_watch_later() {
        let root = std::env::temp_dir().join(format!(
            "riina-history-watch-later-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let watch_later = root.join("watch-later");
        std::fs::create_dir_all(&watch_later).unwrap();
        let archive_path = root.join("playback-history.json");
        let media = "https://media.example/movie.mkv";
        let entry = riina_core::playback_history::Entry {
            path: media.into(),
            date_ms: 1,
            progress: None,
            duration: 90.0,
        };
        crate::playback_history_store::save(&archive_path, std::slice::from_ref(&entry)).unwrap();
        let key = crate::playback_history_store::md5_hex(media.as_bytes());
        std::fs::write(watch_later.join(key), "start=42.25\n").unwrap();
        let mut state = PlaybackHistoryState::with_persistence(
            vec![entry],
            Some(archive_path.clone()),
            Some(watch_later),
        );
        let mut playback = Playback {
            generation: 1,
            phase: Phase::Playing,
            ignore_path_in_watch_later_config: Some(false),
            ..Playback::default()
        };
        playback.playlist.push(PlaylistEntry {
            id: 1,
            filename: media.into(),
            title: None,
            current: true,
            playing: true,
        });

        player_file_context_with_history(&playback, &mut Vec::new(), &mut state, false, 1);

        assert_eq!(state.entries()[0].progress, Some(42.25));
        assert_eq!(
            crate::playback_history_store::load(&archive_path, &[]).unwrap()[0].progress,
            Some(42.25)
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn playback_history_api_values_use_iina_field_names() {
        let entries = playback_history_values(&[riina_core::playback_history::Entry {
            path: "https://media.example/sample.mp4".into(),
            date_ms: 1_750_000_000_000,
            progress: Some(12.5),
            duration: 80.0,
        }]);
        assert_eq!(
            entries,
            [serde_json::json!({
                "name": "sample.mp4",
                "url": "https://media.example/sample.mp4",
                "date": 1_750_000_000_000_u64,
                "progress": 12.5,
                "duration": 80.0
            })]
        );
    }

    #[test]
    fn recent_documents_preserve_recency_names_and_absolute_urls() {
        let path = std::env::temp_dir().join("Riina Recent Clip.mp4");
        let expected_url = url::Url::from_file_path(&path).unwrap().to_string();
        let entries = recent_document_values(&[
            riina_core::RecentMedia {
                path: path.to_string_lossy().into_owned(),
                title: "Recent Clip".into(),
                position: 0.0,
            },
            riina_core::RecentMedia {
                path: "https://media.example/second.mkv".into(),
                title: "Second".into(),
                position: 0.0,
            },
        ]);
        assert_eq!(
            entries[0],
            serde_json::json!({"name":"Riina Recent Clip.mp4","url":expected_url})
        );
        assert_eq!(
            entries[1],
            serde_json::json!({"name":"second.mkv","url":"https://media.example/second.mkv"})
        );
    }

    #[test]
    fn core_ui_visibility_messages_reach_the_player_ui() {
        assert_eq!(
            host_message("plugin", HostEvent::SetUiVisibility(true)),
            PluginHostMessage::SetUiVisibility(true)
        );
    }

    #[test]
    fn playback_snapshots_supply_core_status_and_chapters() {
        let mut playback = Playback {
            phase: Phase::Playing,
            mpv_version: Some("mpv 0.41.0-git".into()),
            position: Some(12.5),
            duration: Some(80.0),
            paused: true,
            speed: 1.5,
            title: "Sample title".into(),
            volume: 82.0,
            audio_delay: 0.125,
            subtitle_delay: -0.25,
            muted: true,
            tracks: vec![Track {
                id: 4,
                kind: TrackKind::Audio,
                title: Some("Commentary".into()),
                lang: Some("en".into()),
                codec: Some("aac".into()),
                selected: true,
                main_selection: Some(0),
                external: false,
                albumart: false,
                external_filename: None,
                is_default: true,
                forced: false,
                channels: Some(2),
                channel_layout: Some("stereo".into()),
                sample_rate: Some(48_000),
                width: None,
                height: None,
                frame_rate: None,
            }],
            ..Playback::default()
        };
        playback.playlist.push(PlaylistEntry {
            id: 1,
            filename: "https://media.example/sample.mp4".into(),
            title: None,
            current: true,
            playing: true,
        });
        playback.chapters.push(Chapter {
            time: 0.0,
            title: Some("Opening".into()),
        });
        let files = PluginFiles::temporary("org.example.snapshot-source");
        files.set_player_context(player_file_context(&playback, &mut Vec::new()));
        let version = files.core_version().unwrap();
        assert_eq!(version["iina"], env!("CARGO_PKG_VERSION"));
        assert!(
            version["build"]
                .as_str()
                .is_some_and(|build| !build.is_empty())
        );
        assert_eq!(version["mpv"], "mpv 0.41.0-git");
        assert_eq!(
            files.core_recent_documents().unwrap(),
            [serde_json::json!({
                "name": "sample.mp4",
                "url": "https://media.example/sample.mp4"
            })]
        );

        assert_eq!(
            files.core_status_property("paused").unwrap(),
            Some(serde_json::json!(true))
        );
        assert_eq!(
            files.core_status_property("position").unwrap(),
            Some(serde_json::json!(12.5))
        );
        assert_eq!(
            files.core_status_property("duration").unwrap(),
            Some(serde_json::json!(80.0))
        );
        assert_eq!(
            files.core_status_property("isNetworkResource").unwrap(),
            Some(serde_json::json!(true))
        );
        assert_eq!(
            files.core_status_property("idle").unwrap(),
            Some(serde_json::json!(false))
        );
        assert_eq!(
            files.core_status_property("audio.id").unwrap(),
            Some(serde_json::json!(4))
        );
        assert_eq!(
            files.core_status_property("audio.delay").unwrap(),
            Some(serde_json::json!(0.125))
        );
        assert_eq!(
            files.core_status_property("audio.volume").unwrap(),
            Some(serde_json::json!(82.0))
        );
        assert_eq!(
            files.core_status_property("audio.muted").unwrap(),
            Some(serde_json::json!(true))
        );
        assert_eq!(
            files.core_status_property("audio.currentTrack").unwrap(),
            Some(serde_json::json!({
                "id": 4,
                "title": "Commentary",
                "formattedTitle": "#4 [English] Commentary (aac, 2ch, 48kHz, Default)",
                "lang": "en",
                "codec": "aac",
                "isDefault": true,
                "isForced": false,
                "isSelected": true,
                "isExternal": false,
                "demuxW": null,
                "demuxH": null,
                "demuxChannelCount": 2,
                "demuxChannels": "stereo",
                "demuxSamplerate": 48000,
                "demuxFPS": null,
            }))
        );
        assert_eq!(
            files.core_status_property("audio.tracks").unwrap(),
            Some(serde_json::json!([{
                "id": 4,
                "title": "Commentary",
                "formattedTitle": "#4 [English] Commentary (aac, 2ch, 48kHz, Default)",
                "lang": "en",
                "codec": "aac",
                "isDefault": true,
                "isForced": false,
                "isSelected": true,
                "isExternal": false,
                "demuxW": null,
                "demuxH": null,
                "demuxChannelCount": 2,
                "demuxChannels": "stereo",
                "demuxSamplerate": 48000,
                "demuxFPS": null,
            }]))
        );
        assert_eq!(
            files.core_chapters().unwrap(),
            [serde_json::json!({"title":"Opening", "start":0.0})]
        );
    }
}
