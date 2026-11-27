//! Share a single duration reader across visible playlists.

use riina_core::sync::lock;
use std::{
    cell::Cell,
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

const MAX_CACHED_PATHS: usize = 2048;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Source {
    Local(PathBuf),
    Stream(String),
}

impl Source {
    pub(crate) fn from_filename(filename: &str) -> Option<Self> {
        if let Some(path) = crate::file_actions::local_media_path(filename) {
            if path.is_absolute() {
                Some(Self::Local(path))
            } else {
                std::path::absolute(path).ok().map(Self::Local)
            }
        } else {
            riina_core::media_reference::validate_network_url(filename)
                .ok()
                .map(|url| Self::Stream(url.into()))
        }
    }

    fn probe_path(&self) -> Option<&Path> {
        let Self::Local(path) = self else { return None };
        riina_core::file_selection::is_supported_media(path.to_str()?).then_some(path.as_path())
    }
}

#[derive(Default)]
struct State {
    windows: BTreeMap<u64, Vec<Source>>,
    requested: HashSet<Source>,
    pending: VecDeque<PathBuf>,
    results: HashMap<Source, Option<f64>>,
    running: bool,
}

impl State {
    fn rebuild_queue(&mut self) {
        self.requested.clear();
        self.pending.clear();
        for source in self.windows.values().flatten() {
            if self.requested.insert(source.clone())
                && !self.results.contains_key(source)
                && let Some(path) = source.probe_path()
            {
                self.pending.push_back(path.to_owned());
            }
        }
        self.trim_cache();
    }

    fn trim_cache(&mut self) {
        if self.results.len() >= MAX_CACHED_PATHS {
            self.results.retain(|path, _| self.requested.contains(path));
        }
    }
}

#[derive(Default)]
struct Cache {
    state: Mutex<State>,
}

static CACHE: OnceLock<Arc<Cache>> = OnceLock::new();

fn cache() -> &'static Arc<Cache> {
    CACHE.get_or_init(Arc::default)
}

impl Cache {
    fn schedule(self: &Arc<Self>, state: &mut State) {
        if state.running || state.pending.is_empty() {
            return;
        }
        state.running = true;
        let worker = self.clone();
        if let Err(error) =
            crate::background::spawn_named("riina-playlist-durations", move || worker.run())
        {
            state.running = false;
            crate::diagnostics::warn!("Could not start the playlist duration reader: {error}");
        }
    }

    fn next_path(&self) -> Option<PathBuf> {
        let mut state = lock(&self.state);
        let next = state.pending.pop_front();
        if next.is_none() {
            // Release the reservation with the queue locked so new demand can restart it.
            state.running = false;
        }
        next
    }

    fn run(&self) {
        let library = crate::platform::playback_library();
        while let Some(path) = self.next_path() {
            let source = Source::Local(path.clone());
            let cancelled = Cell::new(false);
            let duration = riina_mpv::read_local_duration(&library, &path, || {
                let state = lock(&self.state);
                let cancel =
                    !state.requested.contains(&source) || state.results.contains_key(&source);
                cancelled.set(cancel);
                cancel
            });
            let published = {
                let mut state = lock(&self.state);
                if cancelled.get()
                    || !state.requested.contains(&source)
                    || state.results.contains_key(&source)
                {
                    false
                } else {
                    state.results.insert(source, duration);
                    true
                }
            };
            if published {
                super::notify_listeners();
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct Demand(Option<u64>);

impl Demand {
    pub(crate) fn update(&mut self, sources: Vec<Source>) {
        if sources.is_empty() {
            self.withdraw();
            return;
        }
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = *self
            .0
            .get_or_insert_with(|| NEXT_ID.fetch_add(1, Ordering::Relaxed));
        let cache = cache();
        let mut state = lock(&cache.state);
        if state.windows.get(&id) != Some(&sources) {
            state.windows.insert(id, sources);
            state.rebuild_queue();
        }
        cache.schedule(&mut state);
    }

    fn withdraw(&mut self) {
        if let (Some(id), Some(cache)) = (self.0.take(), CACHE.get()) {
            let mut state = lock(&cache.state);
            state.windows.remove(&id);
            state.rebuild_queue();
        }
    }
}

impl Drop for Demand {
    fn drop(&mut self) {
        self.withdraw();
    }
}

pub(crate) fn lookup(source: &Source) -> Option<f64> {
    lock(&CACHE.get()?.state)
        .results
        .get(source)
        .copied()
        .flatten()
}

pub(crate) fn remember(source: Source, seconds: f64) {
    let publish = {
        let mut state = lock(&cache().state);
        if state.results.get(&source) == Some(&Some(seconds)) {
            return;
        }
        state.trim_cache();
        let publish = state.requested.contains(&source);
        state.results.insert(source, Some(seconds));
        publish
    };
    if publish {
        super::notify_listeners();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_local_media_uses_the_probe_and_an_empty_queue_releases_the_reader() {
        let local = Source::from_filename("next.mkv").unwrap();
        let stream = Source::from_filename("https://media.example/stream.mkv?cut=1").unwrap();
        let cache = Cache::default();
        let mut state = lock(&cache.state);
        state.windows.insert(1, vec![local.clone(), stream.clone()]);
        state.windows.insert(2, vec![local.clone()]);
        state.rebuild_queue();
        assert_eq!(state.requested, HashSet::from([local.clone(), stream]));
        assert_eq!(state.pending.len(), 1);
        state.running = true;
        drop(state);

        assert_eq!(cache.next_path().as_deref(), local.probe_path());
        assert!(lock(&cache.state).running);
        assert!(cache.next_path().is_none());
        assert!(!lock(&cache.state).running);
    }
}
