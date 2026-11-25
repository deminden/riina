//! Convert playback observations to MPRIS without consulting the decoder from D-Bus.
use riina_core::{LoopMode, Phase, Playback};
use std::collections::HashMap;
use zbus::zvariant::{ObjectPath, OwnedValue, Str, Value};

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Media {
    pub(super) route: u64,
    pub(super) generation: u64,
    pub(super) title: String,
    pub(super) artist: Option<String>,
    pub(super) album: Option<String>,
    pub(super) length: Option<i64>,
}

impl Media {
    pub(super) fn track_id(&self) -> String {
        format!("/org/riina/player/{}/media/{}", self.route, self.generation)
    }

    pub(super) fn metadata(&self) -> HashMap<&'static str, OwnedValue> {
        let id = self.track_id();
        let Ok(path) = ObjectPath::try_from(id) else {
            return HashMap::new();
        };
        let mut result = HashMap::from([
            ("mpris:trackid", path.into()),
            ("xesam:title", text(&self.title)),
        ]);
        if let Some(length) = self.length {
            result.insert("mpris:length", length.into());
        }
        if let Some(album) = &self.album {
            result.insert("xesam:album", text(album));
        }
        if let Some(artist) = &self.artist
            && let Ok(value) = OwnedValue::try_from(Value::from(vec![artist.as_str()]))
        {
            result.insert("xesam:artist", value);
        }
        result
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Status {
    Playing,
    Paused,
    #[default]
    Stopped,
}

impl Status {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Playing => "Playing",
            Self::Paused => "Paused",
            Self::Stopped => "Stopped",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Snapshot {
    pub(super) media: Option<Media>,
    pub(super) status: Status,
    pub(super) position: i64,
    pub(super) completed_seeks: u64,
    pub(super) rate: f64,
    pub(super) volume: f64,
    pub(super) loop_status: LoopMode,
    pub(super) shuffled: bool,
    pub(super) next: bool,
    pub(super) previous: bool,
    pub(super) seekable: bool,
    pub(super) fullscreen: bool,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            media: None,
            status: Status::Stopped,
            position: 0,
            completed_seeks: 0,
            rate: 1.0,
            volume: 1.0,
            loop_status: LoopMode::Off,
            shuffled: false,
            next: false,
            previous: false,
            seekable: false,
            fullscreen: false,
        }
    }
}

impl Snapshot {
    pub(super) fn read(route: u64, playback: &Playback, fullscreen: bool) -> Self {
        let active = matches!(
            playback.phase,
            Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Seeking | Phase::Ended
        );
        let status = if !active || playback.phase == Phase::Ended {
            Status::Stopped
        } else if playback.paused {
            Status::Paused
        } else {
            Status::Playing
        };
        let current = playback.playlist.iter().position(|entry| entry.current);
        let length = playback.duration.and_then(microseconds);
        let position = playback.position.and_then(microseconds).unwrap_or_default();
        let loop_status = playback.loop_mode();
        let wraps = loop_status == LoopMode::Playlist && playback.playlist.len() > 1;
        let metadata = if playback.audio_status() == riina_core::music_mode::AudioStatus::Audio {
            playback.music_metadata()
        } else {
            playback.file_metadata()
        };
        Self {
            media: active.then(|| Media {
                route,
                generation: playback.generation,
                title: metadata.title.to_owned(),
                artist: metadata.artist.map(str::to_owned),
                album: metadata.album.map(str::to_owned),
                length,
            }),
            status,
            position: length.map_or(position, |length| position.min(length)),
            completed_seeks: playback.completed_navigation_seeks,
            rate: playback.speed,
            volume: if playback.muted {
                0.0
            } else {
                playback.volume / 100.0
            },
            loop_status,
            shuffled: playback.playlist_shuffled,
            next: current.is_some_and(|i| i + 1 < playback.playlist.len() || wraps),
            previous: current.is_some_and(|i| i > 0 || wraps),
            seekable: active && playback.seekable,
            fullscreen,
        }
    }

    pub(super) fn metadata(&self) -> HashMap<&'static str, OwnedValue> {
        self.media
            .as_ref()
            .map_or_else(HashMap::new, Media::metadata)
    }

    pub(super) fn seeked_from(&self, previous: &Self) -> bool {
        let current_id = self
            .media
            .as_ref()
            .map(|media| (media.route, media.generation));
        let previous_id = previous
            .media
            .as_ref()
            .map(|media| (media.route, media.generation));
        current_id.is_some()
            && current_id == previous_id
            && self.completed_seeks != previous.completed_seeks
    }
}

pub(super) fn text(value: &str) -> OwnedValue {
    Str::from(value).into()
}

fn microseconds(seconds: f64) -> Option<i64> {
    (seconds.is_finite() && seconds >= 0.0).then_some((seconds * 1_000_000.0) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changing_active_players_changes_track_identity_without_a_seek_signal() {
        let mut playback = Playback {
            generation: 8,
            phase: Phase::Paused,
            paused: true,
            title: "Fixture".into(),
            artist: Some("Test artist".into()),
            duration: Some(5.0),
            position: Some(1.25),
            volume: 125.0,
            muted: true,
            completed_navigation_seeks: 3,
            ..Playback::default()
        };
        let previous = Snapshot::read(4, &playback, false);
        assert_eq!(previous.status, Status::Paused);
        assert_eq!(previous.position, 1_250_000);
        assert_eq!(previous.volume, 0.0);
        assert!(previous.metadata().contains_key("xesam:artist"));

        playback.position = Some(2.0);
        playback.completed_navigation_seeks = 4;
        let different_player = Snapshot::read(5, &playback, false);
        assert_ne!(different_player.media, previous.media);
        assert!(!different_player.seeked_from(&previous));
        assert!(Snapshot::read(4, &playback, false).seeked_from(&previous));
    }
}
