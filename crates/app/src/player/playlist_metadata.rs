//! Small asynchronous tag cache for audio labels in the playlist.

pub(crate) mod durations;
pub(crate) mod resume;
pub(crate) mod series;

use lofty::{config::ParseOptions, file::TaggedFileExt, probe::Probe, tag::ItemKey};
use riina_core::sync::lock;
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Metadata {
    pub(crate) title: String,
    pub(crate) artist: String,
}

#[derive(Default)]
struct Cache {
    entries: HashMap<PathBuf, Option<Metadata>>,
    pending: VecDeque<PathBuf>,
    running: bool,
}

static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
const MAX_CACHE_ENTRIES: usize = 2048;
const MAX_PENDING_TAGS: usize = 128;

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

fn read_pending_tags(cache: &Mutex<Cache>) {
    loop {
        let path = {
            let mut state = lock(cache);
            let Some(path) = state.pending.pop_front() else {
                state.running = false;
                return;
            };
            path
        };
        let metadata = read_metadata(&path);
        lock(cache).entries.insert(path, metadata);
        // Completion also makes room to retry entries beyond the queue limit.
        notify_listeners();
    }
}

pub(crate) fn tags<'a>(
    references: impl IntoIterator<Item = &'a str>,
) -> HashMap<&'a str, Metadata> {
    let cache = CACHE.get_or_init(Mutex::default);
    let mut state = lock(cache);
    let mut ready = HashMap::new();
    for reference in references {
        let Some(path) = local_audio_path(reference) else {
            continue;
        };
        if let Some(metadata) = state.entries.get(&path) {
            if let Some(metadata) = metadata {
                ready.insert(reference, metadata.clone());
            }
        } else if state.pending.len() < MAX_PENDING_TAGS && state.entries.len() < MAX_CACHE_ENTRIES
        {
            state.entries.insert(path.clone(), None);
            state.pending.push_back(path);
        }
    }
    if !state.running && !state.pending.is_empty() {
        state.running = true;
        if let Err(error) =
            crate::background::spawn_named("riina-playlist-tags", move || read_pending_tags(cache))
        {
            for path in std::mem::take(&mut state.pending) {
                state.entries.remove(&path);
            }
            state.running = false;
            crate::diagnostics::warn!("Could not start playlist metadata reader: {error}");
        }
    }
    ready
}

fn notify_listeners() {
    let listeners: Vec<_> = lock(&LISTENERS).values().cloned().collect();
    for listener in listeners {
        listener();
    }
}

fn local_audio_path(reference: &str) -> Option<PathBuf> {
    crate::file_actions::local_media_path(reference).filter(|path| is_supported_audio(path))
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
        .options(
            ParseOptions::new()
                .read_properties(false)
                .read_cover_art(false),
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    use lofty::{
        config::WriteOptions,
        tag::{Accessor, Tag, TagExt, TagType},
    };

    #[test]
    fn tag_only_reads_preserve_artist_and_title_and_release_the_reader() {
        let temp = riina_test_support::TempDir::new("playlist-tag-only").unwrap();
        let path = temp.path().join("tagged.wav");
        std::fs::copy(
            riina_test_support::fixture("external-tone.wav").unwrap(),
            &path,
        )
        .unwrap();
        let mut tag = Tag::new(TagType::RiffInfo);
        tag.set_artist("Fixture Artist".into());
        tag.set_title("Fixture Title".into());
        tag.save_to_path(&path, WriteOptions::default()).unwrap();
        let cache = Mutex::new(Cache {
            entries: HashMap::from([(path.clone(), None)]),
            pending: VecDeque::from([path.clone()]),
            running: true,
        });

        read_pending_tags(&cache);

        let state = lock(&cache);
        assert!(!state.running);
        assert!(state.pending.is_empty());
        assert_eq!(
            state.entries[&path],
            Some(Metadata {
                title: "Fixture Title".into(),
                artist: "Fixture Artist".into(),
            })
        );
    }
}
