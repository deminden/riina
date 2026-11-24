//! Owned player snapshot and event diffs exposed to plugin APIs.
use riina_ui::PlayerWindowMode;
use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
    sync::Arc,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WindowModes {
    fullscreen: Option<bool>,
    pip: Option<bool>,
    music_mode: bool,
}

impl WindowModes {
    pub(crate) fn property(self, name: &str) -> Option<serde_json::Value> {
        match name {
            "fullscreen" => Some(serde_json::json!(self.fullscreen)),
            "pip" => Some(serde_json::json!(self.pip)),
            "music-mode" => Some(serde_json::json!(self.music_mode)),
            _ => None,
        }
    }

    fn confirm(&mut self, mode: PlayerWindowMode, active: bool) -> Option<&'static str> {
        let (previous, event) = match mode {
            PlayerWindowMode::Fullscreen => (
                self.fullscreen.replace(active).unwrap_or_default(),
                "iina.window-fs.changed",
            ),
            PlayerWindowMode::PictureInPicture => (
                self.pip.replace(active).unwrap_or_default(),
                "iina.pip.changed",
            ),
            PlayerWindowMode::MusicMode => (
                std::mem::replace(&mut self.music_mode, active),
                "iina.music-mode.changed",
            ),
        };
        (previous != active).then_some(event)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PluginPlaylist {
    pub(crate) active: bool,
    pub(crate) entries: Arc<Vec<riina_core::PlaylistEntry>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PluginPlayerFileContext {
    media_generation: u64,
    media_ready: bool,
    pub(super) current_directory: Option<PathBuf>,
    pub(super) external_tracks: BTreeMap<String, PathBuf>,
    pub(super) core_properties: BTreeMap<String, serde_json::Value>,
    observed_mpv_properties: Arc<BTreeMap<String, serde_json::Value>>,
    mpv_events: Arc<VecDeque<riina_core::MpvEventRecord>>,
    pub(super) chapters: Vec<serde_json::Value>,
    pub(super) playlist: PluginPlaylist,
    pub(super) core_version: serde_json::Value,
    pub(super) core_window_properties: BTreeMap<String, serde_json::Value>,
    pub(super) window_modes: WindowModes,
    pub(super) recent_documents: Vec<serde_json::Value>,
    pub(super) playback_history: Vec<serde_json::Value>,
}

impl Default for PluginPlayerFileContext {
    fn default() -> Self {
        Self::new(None, BTreeMap::new())
    }
}

impl PluginPlayerFileContext {
    pub(crate) fn new(
        current_directory: Option<PathBuf>,
        external_tracks: BTreeMap<String, PathBuf>,
    ) -> Self {
        Self {
            media_generation: 0,
            media_ready: false,
            current_directory,
            external_tracks,
            core_properties: BTreeMap::new(),
            observed_mpv_properties: Arc::default(),
            mpv_events: Arc::default(),
            chapters: Vec::new(),
            playlist: PluginPlaylist::default(),
            core_version: serde_json::Value::Null,
            core_window_properties: BTreeMap::from([
                ("loaded".into(), serde_json::Value::Bool(false)),
                ("frame".into(), serde_json::Value::Null),
                ("ontop".into(), serde_json::Value::Null),
                ("visible".into(), serde_json::Value::Null),
                ("main".into(), serde_json::json!(false)),
                ("sidebar".into(), serde_json::Value::Null),
                ("screens".into(), serde_json::Value::Null),
                ("miniaturized".into(), serde_json::Value::Null),
            ]),
            window_modes: WindowModes {
                fullscreen: None,
                pip: None,
                music_mode: false,
            },
            recent_documents: Vec::new(),
            playback_history: Vec::new(),
        }
    }

    pub(crate) fn with_media_generation(mut self, generation: u64) -> Self {
        self.media_generation = generation;
        self
    }

    pub(super) fn is_media_generation(&self, generation: u64) -> bool {
        self.media_generation == generation
    }

    pub(crate) fn with_media_ready(mut self, ready: bool) -> Self {
        self.media_ready = ready;
        self
    }

    pub(crate) fn with_playlist(mut self, playback: &riina_core::Playback) -> Self {
        use riina_core::Phase;
        self.playlist = PluginPlaylist {
            active: matches!(
                playback.phase,
                Phase::Loading
                    | Phase::Playing
                    | Phase::Paused
                    | Phase::Buffering
                    | Phase::Seeking
                    | Phase::Ended
            ),
            entries: playback.playlist.clone(),
        };
        self
    }

    pub(crate) fn replace_playback_snapshot(&mut self, mut playback: Self) {
        // Window observations arrive independently of playback and media changes.
        playback.core_window_properties = std::mem::take(&mut self.core_window_properties);
        playback.window_modes = self.window_modes;
        *self = playback;
    }

    pub(crate) fn player_events_to(&self, next: &Self) -> Vec<(String, serde_json::Value)> {
        let mut events = Vec::new();
        if self
            .core_version
            .get("mpv")
            .and_then(serde_json::Value::as_str)
            .is_none()
            && next
                .core_version
                .get("mpv")
                .and_then(serde_json::Value::as_str)
                .is_some()
        {
            events.push(("iina.mpv-initialized".into(), serde_json::Value::Null));
        }
        if self.media_generation != next.media_generation {
            events.push(("iina.file-started".into(), serde_json::Value::Null));
        }
        if next.media_ready
            && (self.media_generation != next.media_generation || !self.media_ready)
            && let Some(url) = next
                .core_properties
                .get("url")
                .and_then(serde_json::Value::as_str)
        {
            events.push(("iina.file-loaded".into(), serde_json::json!(url)));
        }
        for (context_property, mpv_property) in [
            ("paused", "pause"),
            ("idle", "idle-active"),
            ("position", "time-pos"),
            ("duration", "duration"),
            ("speed", "speed"),
            ("videoWidth", "width"),
            ("videoHeight", "height"),
            ("title", "media-title"),
            ("audio.id", "aid"),
            ("audio.delay", "audio-delay"),
            ("audio.volume", "volume"),
            ("audio.muted", "mute"),
            ("subtitle.id", "sid"),
            ("subtitle.secondID", "secondary-sid"),
            ("subtitle.delay", "sub-delay"),
        ] {
            if let Some(value) = next.core_properties.get(context_property)
                && self.core_properties.get(context_property) != Some(value)
            {
                events.push((format!("mpv.{mpv_property}.changed"), value.clone()));
            }
        }
        for (property, value) in next.observed_mpv_properties.iter() {
            if self.observed_mpv_properties.get(property) != Some(value) {
                events.push((format!("mpv.{property}.changed"), value.clone()));
            }
        }
        let last_event_sequence = self.mpv_events.back().map_or(0, |event| event.sequence);
        for event in next
            .mpv_events
            .iter()
            .filter(|event| event.sequence > last_event_sequence)
        {
            events.push((format!("mpv.{}", event.name), serde_json::Value::Null));
        }
        events
    }

    pub(crate) fn window_events_to(
        &self,
        next: &BTreeMap<String, serde_json::Value>,
    ) -> Vec<(String, serde_json::Value)> {
        let mut events = Vec::new();
        let previous_frame = self.core_window_properties.get("frame");
        let next_frame = next.get("frame");
        if let (Some(previous), Some(current)) = (previous_frame, next_frame)
            && previous.is_object()
            && current.is_object()
        {
            let changed = |key| previous.get(key) != current.get(key);
            if ["x", "y"].into_iter().any(changed) {
                events.push(("iina.window-moved".into(), current.clone()));
            }
            if ["width", "height"].into_iter().any(changed) {
                events.push(("iina.window-resized".into(), current.clone()));
            }
        }
        if let (Some(previous), Some(current)) = (
            self.core_window_properties
                .get("window-size-adjusted-event"),
            next.get("window-size-adjusted-event"),
        ) && previous != current
            && let Some(frame) = next.get("frame").filter(|frame| frame.is_object())
        {
            events.push(("iina.window-size-adjusted".into(), frame.clone()));
        }
        if let (Some(previous), Some(current)) = (
            self.core_window_properties
                .get("main")
                .and_then(serde_json::Value::as_bool),
            next.get("main").and_then(serde_json::Value::as_bool),
        ) && previous != current
        {
            events.push((
                "iina.window-main.changed".into(),
                serde_json::json!(current),
            ));
        }
        if let (Some(previous), Some(current)) = (
            self.core_window_properties
                .get("miniaturized")
                .and_then(serde_json::Value::as_bool),
            next.get("miniaturized")
                .and_then(serde_json::Value::as_bool),
        ) && previous != current
        {
            events.push((
                if current {
                    "iina.window-miniaturized"
                } else {
                    "iina.window-deminiaturized"
                }
                .into(),
                serde_json::Value::Null,
            ));
        }
        if let Some(previous) = self
            .core_window_properties
            .get("screens")
            .filter(|screens| {
                screens
                    .as_array()
                    .is_some_and(|screens| !screens.is_empty())
            })
            && next
                .get("screens")
                .is_some_and(|current| current != previous)
        {
            events.push(("iina.window-screen.changed".into(), serde_json::Value::Null));
        }
        events
    }

    pub(crate) fn with_observed_mpv_properties(
        mut self,
        properties: Arc<BTreeMap<String, serde_json::Value>>,
    ) -> Self {
        self.observed_mpv_properties = properties;
        self
    }

    pub(crate) fn with_mpv_events(
        mut self,
        events: Arc<VecDeque<riina_core::MpvEventRecord>>,
    ) -> Self {
        self.mpv_events = events;
        self
    }

    pub(crate) fn with_core_snapshot(
        mut self,
        properties: BTreeMap<String, serde_json::Value>,
        chapters: Vec<serde_json::Value>,
    ) -> Self {
        self.core_properties = properties;
        self.chapters = chapters;
        self
    }

    pub(crate) fn with_recent_documents(
        mut self,
        recent_documents: Vec<serde_json::Value>,
    ) -> Self {
        self.recent_documents = recent_documents;
        self
    }

    pub(crate) fn with_playback_history(
        mut self,
        playback_history: Vec<serde_json::Value>,
    ) -> Self {
        self.playback_history = playback_history;
        self
    }

    pub(crate) fn with_core_version(mut self, version: serde_json::Value) -> Self {
        self.core_version = version;
        self
    }

    pub(crate) fn mpv_initialized(&self) -> bool {
        self.core_version
            .get("mpv")
            .and_then(serde_json::Value::as_str)
            .is_some()
    }

    pub(crate) fn set_core_window_properties(
        &mut self,
        properties: BTreeMap<String, serde_json::Value>,
    ) {
        if properties
            .get("loaded")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
        {
            self.window_modes.fullscreen.get_or_insert(false);
            self.window_modes.pip.get_or_insert(false);
        }
        self.core_window_properties = properties;
    }

    pub(crate) fn confirm_window_mode(
        &mut self,
        mode: PlayerWindowMode,
        active: bool,
    ) -> Option<(&'static str, WindowModes)> {
        self.window_modes
            .confirm(mode, active)
            .map(|event| (event, self.window_modes))
    }
}

#[cfg(test)]
mod mpv_event_bridge_tests {
    use super::*;

    #[test]
    fn player_context_delivers_each_native_mpv_event_once_across_coalesced_updates() {
        let initial = PluginPlayerFileContext::default();
        let playback = PluginPlayerFileContext::default().with_mpv_events(
            VecDeque::from([
                riina_core::MpvEventRecord {
                    sequence: 1,
                    name: "start-file".into(),
                },
                riina_core::MpvEventRecord {
                    sequence: 2,
                    name: "file-loaded".into(),
                },
            ])
            .into(),
        );

        assert_eq!(
            initial.player_events_to(&playback),
            vec![
                ("mpv.start-file".into(), serde_json::Value::Null),
                ("mpv.file-loaded".into(), serde_json::Value::Null),
            ]
        );
        assert!(playback.player_events_to(&playback).is_empty());
    }
}
