//! Small asynchronous tag cache for audio labels in the playlist.

pub(crate) mod durations;
pub(crate) mod resume;
pub(crate) mod series;

use lofty::{file::TaggedFileExt, probe::Probe, tag::ItemKey};
use riina_core::sync::lock;
use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
        mpsc::{SyncSender, TrySendError, sync_channel},
    },
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Metadata {
    pub(crate) title: String,
    pub(crate) artist: String,
}

struct Cache {
    entries: Mutex<HashMap<PathBuf, Option<Metadata>>>,
    requests: SyncSender<PathBuf>,
}

static CACHE: OnceLock<Cache> = OnceLock::new();
const MAX_CACHE_ENTRIES: usize = 2048;

type Listener = Arc<dyn Fn() + Send + Sync>;
static LISTENERS: Mutex<BTreeMap<u64, Listener>> = Mutex::new(BTreeMap::new());

pub(crate) struct Subscription(u64);

pub(crate) fn subscribe(listener: impl Fn() + Send + Sync + 'static) -> Subscription {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    lock(&LISTENERS).insert(id, Arc::new(listener));
    Subscription(id)
}

impl Drop for Subscription {
    fn drop(&mut self) {
        lock(&LISTENERS).remove(&self.0);
    }
}

fn cache() -> &'static Cache {
    CACHE.get_or_init(|| {
        let (requests, receiver) = sync_channel::<PathBuf>(128);
        let cache = Cache {
            entries: Mutex::new(HashMap::new()),
            requests,
        };
        let worker = crate::background::spawn_named("riina-playlist-tags", move || {
            while let Ok(path) = receiver.recv() {
                let metadata = read_metadata(&path);
                let Some(cache) = CACHE.get() else { continue };
                if let Some(entry) = lock(&cache.entries).get_mut(&path) {
                    *entry = metadata;
                } else {
                    continue;
                }
                // A completed request can also make room to retry a full queue.
                notify_listeners();
            }
        });
        if let Err(error) = worker {
            crate::diagnostics::warn!("Could not start playlist metadata reader: {error}");
        }
        cache
    })
}

fn notify_listeners() {
    let listeners: Vec<_> = lock(&LISTENERS).values().cloned().collect();
    for listener in listeners {
        listener();
    }
}

pub(crate) fn lookup(reference: &str) -> Option<Metadata> {
    let path = local_audio_path(reference)?;
    let cache = cache();
    let mut entries = lock(&cache.entries);
    if let Some(metadata) = entries.get(&path) {
        return metadata.clone();
    }
    if entries.len() >= MAX_CACHE_ENTRIES {
        return None;
    }
    entries.insert(path.clone(), None);
    match cache.requests.try_send(path.clone()) {
        Ok(()) => None,
        Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
            entries.remove(&path);
            None
        }
    }
}

fn local_audio_path(reference: &str) -> Option<PathBuf> {
    let bytes = reference.as_bytes();
    let windows_drive_path =
        bytes.get(1) == Some(&b':') && bytes.first().is_some_and(u8::is_ascii_alphabetic);
    if windows_drive_path {
        let path = PathBuf::from(reference);
        return is_supported_audio(&path).then_some(path);
    }
    if let Ok(url) = url::Url::parse(reference) {
        if url.scheme() != "file" {
            return None;
        }
        let path = url.to_file_path().ok()?;
        return is_supported_audio(&path).then_some(path);
    }
    let path = Path::new(reference);
    is_supported_audio(path).then(|| path.to_path_buf())
}

fn is_supported_audio(path: &Path) -> bool {
    path.extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|extension| {
            riina_core::file_selection::SUPPORTED_AUDIO_EXTENSIONS
                .iter()
                .any(|supported| extension.eq_ignore_ascii_case(supported))
        })
}

fn read_metadata(path: &Path) -> Option<Metadata> {
    let tagged = Probe::open(path)
        .ok()?
        .guess_file_type()
        .ok()?
        .read()
        .ok()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
    let title = tag.get_string(ItemKey::TrackTitle)?.trim().to_owned();
    let artist = tag
        .get_string(ItemKey::TrackArtist)
        .or_else(|| tag.get_string(ItemKey::AlbumArtist))?
        .trim()
        .to_owned();
    (!title.is_empty() && !artist.is_empty()).then_some(Metadata { title, artist })
}
