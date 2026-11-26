//! Serve the MPRIS interfaces from the latest immutable player projection.
use super::{
    projection::{Snapshot, Status},
    track_list,
};
use riina_core::{LoopMode, sync::lock};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::mpsc;
use zbus::{
    fdo,
    object_server::SignalEmitter,
    zvariant::{ObjectPath, OwnedValue},
};

pub(super) const OBJECT: &str = "/org/mpris/MediaPlayer2";
pub(super) const ROOT: &str = "org.mpris.MediaPlayer2";
pub(super) const PLAYER: &str = "org.mpris.MediaPlayer2.Player";

#[derive(Debug)]
pub(super) enum Operation {
    Raise,
    Quit,
    Fullscreen(bool),
    Play,
    Pause,
    PlayPause,
    Stop,
    Next,
    Previous,
    Seek(i64),
    Position(i64),
    SelectTrack(i64),
    RemoveTrack(i64),
    AddTrack {
        path: String,
        after_id: Option<i64>,
        make_current: bool,
    },
    Loop(LoopMode),
    Rate(f64),
    Shuffle(bool),
    Volume(f64),
}

#[derive(Debug)]
pub(super) struct Request {
    pub(super) route: u64,
    pub(super) generation: u64,
    pub(super) operation: Operation,
}

#[derive(Clone)]
pub(super) struct Endpoint {
    snapshot: Arc<Mutex<Snapshot>>,
    requests: mpsc::Sender<Request>,
}

impl Endpoint {
    pub(super) fn new(snapshot: Arc<Mutex<Snapshot>>, requests: mpsc::Sender<Request>) -> Self {
        Self { snapshot, requests }
    }

    pub(super) fn with<R>(&self, read: impl FnOnce(&Snapshot) -> R) -> R {
        read(&lock(&self.snapshot))
    }

    fn send(&self, operation: Operation) -> fdo::Result<()> {
        self.send_when(operation, |_| true)
    }

    fn send_when(
        &self,
        operation: Operation,
        accepts: impl FnOnce(&Snapshot) -> bool,
    ) -> fdo::Result<()> {
        self.send_projected(|snapshot| Ok(accepts(snapshot).then_some(operation)))
    }

    pub(super) fn send_projected(
        &self,
        project: impl FnOnce(&Snapshot) -> fdo::Result<Option<Operation>>,
    ) -> fdo::Result<()> {
        self.with(|snapshot| {
            let Some(media) = snapshot.media.as_ref() else {
                return Ok(());
            };
            let Some(operation) = project(snapshot)? else {
                return Ok(());
            };
            self.requests
                .try_send(Request {
                    route: media.route,
                    generation: media.generation,
                    operation,
                })
                .map_err(|_| {
                    fdo::Error::LimitsExceeded("The player cannot accept another request".into())
                })
        })
    }
}

pub(super) struct RootInterface(pub(super) Endpoint);

#[zbus::interface(name = "org.mpris.MediaPlayer2")]
impl RootInterface {
    fn raise(&self) -> fdo::Result<()> {
        self.0.send(Operation::Raise)
    }

    fn quit(&self) -> fdo::Result<()> {
        self.0.send(Operation::Quit)
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn can_quit(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn fullscreen(&self) -> bool {
        self.0.with(|state| state.fullscreen)
    }

    #[zbus(property)]
    fn set_fullscreen(&self, active: bool) -> fdo::Result<()> {
        self.0.send(Operation::Fullscreen(active))
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn can_set_fullscreen(&self) -> bool {
        true
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn can_raise(&self) -> bool {
        true
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn has_track_list(&self) -> bool {
        true
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn identity(&self) -> &'static str {
        "Riina"
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn desktop_entry(&self) -> &'static str {
        "org.riina.player"
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn supported_uri_schemes(&self) -> Vec<&'static str> {
        vec!["file", "http", "https"]
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn supported_mime_types(&self) -> Vec<&'static str> {
        vec![
            "audio/aac",
            "audio/flac",
            "audio/mpeg",
            "audio/ogg",
            "audio/wav",
            "video/mp4",
            "video/mpeg",
            "video/webm",
            "video/x-matroska",
            "video/x-msvideo",
        ]
    }
}

pub(super) struct PlayerInterface(pub(super) Endpoint);

#[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
impl PlayerInterface {
    fn next(&self) -> fdo::Result<()> {
        self.0.send_when(Operation::Next, |snapshot| snapshot.next)
    }

    fn previous(&self) -> fdo::Result<()> {
        self.0
            .send_when(Operation::Previous, |snapshot| snapshot.previous)
    }

    fn pause(&self) -> fdo::Result<()> {
        if !self.can_pause() {
            return Err(fdo::Error::NotSupported(
                "Current media cannot pause".into(),
            ));
        }
        self.0.send(Operation::Pause)
    }

    fn play_pause(&self) -> fdo::Result<()> {
        self.0.send(Operation::PlayPause)
    }

    fn stop(&self) -> fdo::Result<()> {
        self.0.send(Operation::Stop)
    }

    fn play(&self) -> fdo::Result<()> {
        self.0.send(Operation::Play)
    }

    fn seek(&self, offset: i64) -> fdo::Result<()> {
        self.0
            .send_when(Operation::Seek(offset), |snapshot| snapshot.seekable)
    }

    fn set_position(&self, track_id: ObjectPath<'_>, position: i64) -> fdo::Result<()> {
        self.0.send_when(Operation::Position(position), |snapshot| {
            snapshot.seekable
                && position >= 0
                && snapshot.media.as_ref().is_some_and(|media| {
                    media.track_id().is_some_and(|current| current == track_id)
                        && media.length.is_none_or(|length| position <= length)
                })
        })
    }

    fn open_uri(&self, uri: &str) -> fdo::Result<()> {
        let path = track_list::media_reference(uri)?;
        self.0.send_projected(|snapshot| {
            Ok(Some(Operation::AddTrack {
                path,
                after_id: snapshot.media.as_ref().and_then(|media| media.entry),
                make_current: true,
            }))
        })
    }

    #[zbus(signal)]
    pub(super) async fn seeked(emitter: &SignalEmitter<'_>, position: i64) -> zbus::Result<()>;

    #[zbus(property)]
    fn playback_status(&self) -> &'static str {
        self.0.with(|state| state.status.as_str())
    }

    #[zbus(property)]
    fn loop_status(&self) -> &'static str {
        self.0.with(|state| loop_label(state.loop_status))
    }

    #[zbus(property)]
    fn set_loop_status(&self, status: &str) -> fdo::Result<()> {
        let mode = match status {
            "None" => LoopMode::Off,
            "Track" => LoopMode::File,
            "Playlist" => LoopMode::Playlist,
            _ => return Err(fdo::Error::InvalidArgs("Invalid loop status".into())),
        };
        self.0.send(Operation::Loop(mode))
    }

    #[zbus(property)]
    fn rate(&self) -> f64 {
        self.0.with(|state| state.rate)
    }

    #[zbus(property)]
    fn set_rate(&self, value: f64) -> fdo::Result<()> {
        if value == 0.0 {
            return self.0.send(Operation::Pause);
        }
        if !(self.minimum_rate()..=self.maximum_rate()).contains(&value) {
            return Err(fdo::Error::InvalidArgs(
                "Playback rate outside the supported range".into(),
            ));
        }
        self.0.send(Operation::Rate(value))
    }

    #[zbus(property)]
    fn shuffle(&self) -> bool {
        self.0.with(|state| state.shuffled)
    }

    #[zbus(property)]
    fn set_shuffle(&self, value: bool) -> fdo::Result<()> {
        self.0.send(Operation::Shuffle(value))
    }

    #[zbus(property)]
    fn metadata(&self) -> HashMap<&'static str, OwnedValue> {
        self.0.with(Snapshot::metadata)
    }

    #[zbus(property)]
    fn volume(&self) -> f64 {
        self.0.with(|state| state.volume)
    }

    #[zbus(property)]
    fn set_volume(&self, value: f64) -> fdo::Result<()> {
        if !value.is_finite() {
            return Err(fdo::Error::InvalidArgs(
                "Volume must be a finite value".into(),
            ));
        }
        self.0.send(Operation::Volume(value.max(0.0)))
    }

    #[zbus(property(emits_changed_signal = "false"))]
    fn position(&self) -> i64 {
        self.0.with(|state| state.position)
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn minimum_rate(&self) -> f64 {
        0.01
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn maximum_rate(&self) -> f64 {
        100.0
    }

    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        self.0.with(|state| state.next)
    }

    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        self.0.with(|state| state.previous)
    }

    #[zbus(property)]
    fn can_play(&self) -> bool {
        self.0.with(|state| state.media.is_some())
    }

    #[zbus(property)]
    fn can_pause(&self) -> bool {
        self.0.with(|state| state.status != Status::Stopped)
    }

    #[zbus(property)]
    fn can_seek(&self) -> bool {
        self.0.with(|state| state.seekable)
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn can_control(&self) -> bool {
        true
    }
}

pub(super) const fn loop_label(mode: LoopMode) -> &'static str {
    match mode {
        LoopMode::Off => "None",
        LoopMode::File => "Track",
        LoopMode::Playlist => "Playlist",
    }
}
