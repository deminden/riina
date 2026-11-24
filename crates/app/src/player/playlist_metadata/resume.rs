//! A visible playlist reads saved resume positions from its own profile.

use crate::{PlayerWindow, background, playback_history_store};
use riina_core::{Playback, PlaylistEntry, sync::lock};
use slint::ComponentHandle;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

pub(crate) type Positions = HashMap<String, f64>;

static REVISION: AtomicU64 = AtomicU64::new(0);

pub(crate) fn invalidate() {
    REVISION.fetch_add(1, Ordering::AcqRel);
}

pub(crate) struct Reader {
    directory: PathBuf,
    state: Arc<Mutex<State>>,
}

#[derive(Default)]
struct State {
    playlist: Option<Arc<Vec<PlaylistEntry>>>,
    request: Option<Arc<Request>>,
    saved: Option<(PathPolicy, Arc<Positions>)>,
    running: bool,
}

#[derive(PartialEq, Eq)]
struct Request {
    filenames: HashSet<String>,
    policy: PathPolicy,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct PathPolicy {
    ignore_path: bool,
    revision: u64,
}

impl PathPolicy {
    fn observed(playback: &Playback) -> Option<Self> {
        Some(Self {
            ignore_path: playback.ignore_path_in_watch_later_config?,
            revision: REVISION.load(Ordering::Acquire),
        })
    }
}

impl Reader {
    pub(crate) fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            state: Arc::default(),
        }
    }

    pub(crate) fn positions(&self, playback: &Playback) -> Option<Arc<Positions>> {
        let state = lock(&self.state);
        let (policy, positions) = state.saved.as_ref()?;
        (*policy == PathPolicy::observed(playback)?).then(|| positions.clone())
    }

    pub(crate) fn refresh(&self, app: &PlayerWindow, playback: &Playback) {
        let mut state = lock(&self.state);
        let Some(policy) = PathPolicy::observed(playback)
            .filter(|_| app.get_prefetch_playlist_durations() && !playback.playlist.is_empty())
        else {
            state.clear();
            return;
        };
        if state
            .request
            .as_ref()
            .is_some_and(|previous| previous.policy == policy)
            && state
                .playlist
                .as_ref()
                .is_some_and(|list| Arc::ptr_eq(list, &playback.playlist))
        {
            return;
        }
        state.playlist = Some(playback.playlist.clone());
        let request = Request {
            filenames: playback
                .playlist
                .iter()
                .map(|entry| entry.filename.clone())
                .collect(),
            policy,
        };
        if state.request.as_deref() == Some(&request) {
            return;
        }
        state.request = Some(Arc::new(request));
        if state.running {
            return;
        }
        state.running = true;
        let shared = self.state.clone();
        let directory = self.directory.clone();
        let weak = app.as_weak();
        if let Err(error) = background::spawn_named("riina-playlist-resume", move || {
            if read_latest(&shared, &directory) {
                let _ = background::to_ui(weak, |app| app.invoke_refresh_playlist_metadata());
            }
        }) {
            state.running = false;
            state.clear();
            crate::diagnostics::warn!("Could not read saved playlist positions: {error}");
        }
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        lock(&self.state).clear();
    }
}

impl State {
    fn clear(&mut self) {
        self.request = None;
        self.playlist = None;
    }

    fn is_current(&self, request: &Arc<Request>) -> bool {
        self.request
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, request))
            && request.policy.revision == REVISION.load(Ordering::Acquire)
    }
}

fn read_latest(shared: &Mutex<State>, directory: &Path) -> bool {
    loop {
        let request = {
            let mut state = lock(shared);
            let Some(request) = state
                .request
                .clone()
                .filter(|request| state.is_current(request))
            else {
                state.running = false;
                return false;
            };
            request
        };
        let mut positions = Positions::new();
        for filename in &request.filenames {
            if !lock(shared).is_current(&request) {
                break;
            }
            if let Some(position) = playback_history_store::progress_from_watch_later(
                filename,
                directory,
                request.policy.ignore_path,
            ) {
                positions.insert(filename.clone(), position);
            }
        }
        let mut state = lock(shared);
        if state.is_current(&request) {
            state.saved = Some((request.policy, Arc::new(positions)));
            state.running = false;
            return true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_test_support::TempDir;

    #[test]
    fn the_current_request_replaces_previous_results_and_uses_the_profiles_path_policy() {
        let temp = TempDir::new("playlist-resume-positions").unwrap();
        let directory = temp.path();
        let filename = "folder/Video with spaces.mkv";
        let path = directory.join(playback_history_store::md5_hex(b"Video with spaces.mkv"));
        std::fs::write(path, "start=7.5\n").unwrap();
        let request = Arc::new(Request {
            filenames: HashSet::from([filename.into(), "missing.mkv".into()]),
            policy: PathPolicy {
                ignore_path: true,
                revision: REVISION.load(Ordering::Acquire),
            },
        });
        let shared = Mutex::new(State {
            request: Some(request.clone()),
            saved: Some((
                request.policy,
                Arc::new(Positions::from([("previous.mkv".into(), 1.0)])),
            )),
            running: true,
            ..State::default()
        });
        assert!(read_latest(&shared, directory));
        assert_eq!(
            lock(&shared).saved.as_ref().unwrap().1.as_ref(),
            &Positions::from([(filename.into(), 7.5)])
        );
        assert!(!lock(&shared).running);

        let mut state = lock(&shared);
        state.clear();
        assert!(!state.is_current(&request));
        assert!(state.saved.is_some());
    }
}
