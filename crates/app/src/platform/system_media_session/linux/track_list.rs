//! The MPRIS queue exposes a small window around the playing entry.
use super::{
    interface::{Endpoint, Operation},
    projection::text,
};
use riina_core::{PlaylistEntry, media_reference::validate_network_url};
use std::{collections::HashMap, ops::Range, path::Path, sync::Arc};
use url::Url;
use zbus::{
    fdo,
    object_server::SignalEmitter,
    zvariant::{ObjectPath, OwnedValue},
};

pub(super) const TRACK_LIST: &str = "org.mpris.MediaPlayer2.TrackList";
pub(super) const NO_TRACK: &str = "/org/mpris/MediaPlayer2/TrackList/NoTrack";
const WINDOW_SIZE: usize = 24;

#[derive(Clone, Debug, Default)]
pub(super) struct Queue {
    route: u64,
    entries: Arc<Vec<PlaylistEntry>>,
    visible: Range<usize>,
}

impl Queue {
    pub(super) fn read(
        route: u64,
        entries: Arc<Vec<PlaylistEntry>>,
        current: Option<usize>,
    ) -> Self {
        let start = current
            .unwrap_or_default()
            .saturating_sub(WINDOW_SIZE / 2)
            .min(entries.len().saturating_sub(WINDOW_SIZE));
        let end = (start + WINDOW_SIZE).min(entries.len());
        Self {
            route,
            entries,
            visible: start..end,
        }
    }

    pub(super) fn entries(&self) -> &[PlaylistEntry] {
        &self.entries[self.visible.clone()]
    }

    pub(super) fn tracks(&self) -> Vec<ObjectPath<'static>> {
        self.entries()
            .iter()
            .filter_map(|entry| self.path(entry.id).ok())
            .collect()
    }

    pub(super) fn path(&self, id: i64) -> zbus::zvariant::Result<ObjectPath<'static>> {
        track_path(self.route, id)
    }

    pub(super) fn same_tracks(&self, other: &Self) -> bool {
        self.route == other.route
            && self
                .entries()
                .iter()
                .map(|entry| entry.id)
                .eq(other.entries().iter().map(|entry| entry.id))
    }

    fn find(&self, path: &ObjectPath<'_>) -> Option<&PlaylistEntry> {
        let (_, code) = path.as_str().rsplit_once('/')?;
        let id = i64::from_ne_bytes(code.parse::<u64>().ok()?.to_ne_bytes());
        (self.path(id).ok().as_ref() == Some(path))
            .then(|| self.entries().iter().find(|entry| entry.id == id))
            .flatten()
    }
}

impl PartialEq for Queue {
    fn eq(&self, other: &Self) -> bool {
        self.route == other.route
            && ((Arc::ptr_eq(&self.entries, &other.entries) && self.visible == other.visible)
                || self.entries() == other.entries())
    }
}

pub(super) fn track_path(route: u64, id: i64) -> zbus::zvariant::Result<ObjectPath<'static>> {
    let code = u64::from_ne_bytes(id.to_ne_bytes());
    ObjectPath::try_from(format!("/org/riina/player/{route}/tracks/{code}"))
}

pub(super) fn title(entry: &PlaylistEntry) -> &str {
    if let Some(title) = entry.title.as_deref().filter(|title| !title.is_empty()) {
        return title;
    }
    if let Some((_, authority_path)) = entry.filename.split_once("://") {
        // Queue metadata never exposes URL credentials, query parameters, or fragments.
        let Some((_, path)) = authority_path
            .split(['?', '#'])
            .next()
            .and_then(|url| url.split_once('/'))
        else {
            return "Network stream";
        };
        return path
            .rsplit('/')
            .find(|part| !part.is_empty())
            .unwrap_or("Network stream");
    }
    Path::new(&entry.filename)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(&entry.filename)
}

pub(super) fn metadata(
    path: ObjectPath<'static>,
    entry: &PlaylistEntry,
) -> HashMap<&'static str, OwnedValue> {
    HashMap::from([
        ("mpris:trackid", path.into()),
        ("xesam:title", text(title(entry))),
    ])
}

pub(super) fn media_reference(uri: &str) -> fdo::Result<String> {
    let invalid = || fdo::Error::InvalidArgs("Unsupported media URI".into());
    let parsed = Url::parse(uri).map_err(|_| invalid())?;
    match parsed.scheme() {
        "file" if parsed.query().is_none() && parsed.fragment().is_none() => parsed
            .to_file_path()
            .ok()
            .and_then(|path| path.into_os_string().into_string().ok())
            .filter(|path| !path.contains('\0'))
            .ok_or_else(invalid),
        "http" | "https" => validate_network_url(uri)
            .map(str::to_owned)
            .map_err(|_| invalid()),
        _ => Err(invalid()),
    }
}

pub(super) struct TrackListInterface(pub(super) Endpoint);

#[zbus::interface(name = "org.mpris.MediaPlayer2.TrackList")]
impl TrackListInterface {
    #[zbus(property(emits_changed_signal = "invalidates"))]
    fn tracks(&self) -> Vec<ObjectPath<'static>> {
        self.0.with(|snapshot| snapshot.queue.tracks())
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn can_edit_tracks(&self) -> bool {
        true
    }

    fn get_tracks_metadata(
        &self,
        track_ids: Vec<ObjectPath<'_>>,
    ) -> fdo::Result<Vec<HashMap<&'static str, OwnedValue>>> {
        self.0.with(|snapshot| {
            let current = snapshot.media.as_ref().and_then(|media| media.track_id());
            track_ids
                .into_iter()
                .map(|path| {
                    let entry = snapshot.queue.find(&path).ok_or_else(|| {
                        fdo::Error::InvalidArgs(
                            "The requested track is outside the current queue".into(),
                        )
                    })?;
                    if current.as_ref() == Some(&path) {
                        Ok(snapshot.metadata())
                    } else {
                        Ok(metadata(path.into_owned(), entry))
                    }
                })
                .collect()
        })
    }

    fn go_to(&self, track_id: ObjectPath<'_>) -> fdo::Result<()> {
        self.0.send_projected(|snapshot| {
            Ok(snapshot
                .queue
                .find(&track_id)
                .map(|entry| Operation::SelectTrack(entry.id)))
        })
    }

    fn add_track(
        &self,
        uri: &str,
        after_track: ObjectPath<'_>,
        set_as_current: bool,
    ) -> fdo::Result<()> {
        let path = media_reference(uri)?;
        self.0.send_projected(|snapshot| {
            let after_id = if after_track.as_str() == NO_TRACK {
                None
            } else {
                Some(
                    snapshot
                        .queue
                        .find(&after_track)
                        .ok_or_else(|| {
                            fdo::Error::InvalidArgs(
                                "The playlist anchor is no longer available".into(),
                            )
                        })?
                        .id,
                )
            };
            Ok(Some(Operation::AddTrack {
                path,
                after_id,
                make_current: set_as_current,
            }))
        })
    }

    fn remove_track(&self, track_id: ObjectPath<'_>) -> fdo::Result<()> {
        self.0.send_projected(|snapshot| {
            Ok(snapshot
                .queue
                .find(&track_id)
                .map(|entry| Operation::RemoveTrack(entry.id)))
        })
    }

    #[zbus(signal)]
    pub(super) async fn track_list_replaced(
        emitter: &SignalEmitter<'_>,
        tracks: Vec<ObjectPath<'_>>,
        current_track: ObjectPath<'_>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub(super) async fn track_metadata_changed(
        emitter: &SignalEmitter<'_>,
        track_id: ObjectPath<'_>,
        metadata: HashMap<&'static str, OwnedValue>,
    ) -> zbus::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_files_have_distinct_keys_that_follow_the_entries() {
        let mut entries: Vec<_> = (0..40)
            .map(|id| PlaylistEntry {
                id,
                filename: "/music/same.flac".into(),
                title: None,
                current: false,
                playing: false,
            })
            .collect();
        let queue = Queue::read(9, entries.clone().into(), Some(30));
        let first = queue.path(30).unwrap();
        let second = queue.path(31).unwrap();
        assert_ne!(first, second);
        assert_eq!(queue.tracks().len(), WINDOW_SIZE);
        assert_eq!(queue.find(&first).unwrap().id, 30);
        assert!(queue.find(&queue.path(0).unwrap()).is_none());
        entries.swap(30, 31);
        let reordered = Queue::read(9, entries.clone().into(), Some(31));
        assert!(!queue.same_tracks(&reordered));
        assert_eq!(reordered.find(&first).unwrap().id, 30);
        assert!(
            Queue::read(10, entries.into(), Some(31))
                .find(&first)
                .is_none()
        );
    }

    #[test]
    fn validates_media_references_and_keeps_private_url_fields_out_of_queue_titles() {
        assert_eq!(
            media_reference("file:///tmp/first%20track.flac").unwrap(),
            "/tmp/first track.flac"
        );
        assert!(media_reference("file://another-host/tmp/movie.mp4").is_err());
        assert!(media_reference("file:///tmp/%00bad.mp4").is_err());
        assert!(media_reference("lavfi://testsrc").is_err());
        let entry = PlaylistEntry {
            id: 1,
            filename: "https://user:secret@host/direct/movie.mp4?token=secret#secret".into(),
            title: None,
            current: false,
            playing: false,
        };
        assert!(media_reference(&entry.filename).is_ok());
        assert_eq!(title(&entry), "movie.mp4");
    }
}
