//! Share a single duration reader across visible playlists.

use riina_core::sync::lock;
use std::{
    cell::Cell,
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

const MAX_CACHED_PATHS: usize = 2048;

#[derive(Default)]
struct State {
    windows: BTreeMap<u64, Vec<PathBuf>>,
    requested: HashSet<PathBuf>,
    pending: VecDeque<PathBuf>,
    results: HashMap<PathBuf, Option<f64>>,
}

impl State {
    fn rebuild_queue(&mut self) {
        self.requested.clear();
        self.pending.clear();
        for path in self.windows.values().flatten() {
            if self.requested.insert(path.clone()) && !self.results.contains_key(path) {
                self.pending.push_back(path.clone());
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
    ready: Condvar,
}

static CACHE: OnceLock<Arc<Cache>> = OnceLock::new();

fn cache() -> &'static Arc<Cache> {
    CACHE.get_or_init(|| {
        let cache = Arc::new(Cache::default());
        let worker = cache.clone();
        if let Err(error) =
            crate::background::spawn_named("riina-playlist-durations", move || worker.run())
        {
            crate::diagnostics::warn!("Could not start the playlist duration reader: {error}");
        }
        cache
    })
}

impl Cache {
    fn next_path(&self) -> PathBuf {
        let mut state = lock(&self.state);
        loop {
            while let Some(path) = state.pending.pop_front() {
                if !state.results.contains_key(&path) {
                    return path;
                }
            }
            state = self
                .ready
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    fn run(&self) {
        let library = crate::platform::playback_library();
        loop {
            let path = self.next_path();
            let cancelled = Cell::new(false);
            let duration = riina_mpv::read_local_duration(&library, &path, || {
                let state = lock(&self.state);
                let cancel = !state.requested.contains(&path) || state.results.contains_key(&path);
                cancelled.set(cancel);
                cancel
            });
            let published = {
                let mut state = lock(&self.state);
                if cancelled.get()
                    || !state.requested.contains(&path)
                    || state.results.contains_key(&path)
                {
                    false
                } else {
                    state.results.insert(path, duration);
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
    pub(crate) fn update(&mut self, paths: Vec<PathBuf>) {
        if self.0.is_none() && paths.is_empty() {
            return;
        }
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = *self
            .0
            .get_or_insert_with(|| NEXT_ID.fetch_add(1, Ordering::Relaxed));
        let cache = cache();
        let mut state = lock(&cache.state);
        if state.windows.get(&id) != Some(&paths) {
            state.windows.insert(id, paths);
            state.rebuild_queue();
            cache.ready.notify_one();
        }
    }
}

impl Drop for Demand {
    fn drop(&mut self) {
        if let (Some(id), Some(cache)) = (self.0, CACHE.get()) {
            let mut state = lock(&cache.state);
            state.windows.remove(&id);
            state.rebuild_queue();
        }
    }
}

pub(crate) fn local_path(reference: &str) -> Option<PathBuf> {
    let path = crate::file_actions::local_media_path(reference)?;
    if !riina_core::file_selection::is_supported_media(path.to_str()?) {
        return None;
    }
    if path.is_absolute() {
        Some(path)
    } else {
        std::path::absolute(path).ok()
    }
}

pub(crate) fn lookup(path: &Path) -> Option<f64> {
    lock(&CACHE.get()?.state)
        .results
        .get(path)
        .copied()
        .flatten()
}

pub(crate) fn remember(path: PathBuf, seconds: f64) {
    let publish = {
        let mut state = lock(&cache().state);
        if state.results.get(&path) == Some(&Some(seconds)) {
            return;
        }
        state.trim_cache();
        let publish = state.requested.contains(&path);
        state.results.insert(path, Some(seconds));
        publish
    };
    if publish {
        super::notify_listeners();
    }
}
