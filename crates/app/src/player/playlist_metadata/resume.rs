//! A visible playlist reads saved resume positions from its own profile.

use crate::{PlayerWindow, background, playback_history_store};
use riina_core::{Playback, PlaylistEntry, sync::lock};
use slint::ComponentHandle;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicU64, Ordering},
    },
};

pub(crate) type Positions = HashMap<String, f64>;

type Revisions = Mutex<HashMap<PathBuf, Weak<AtomicU64>>>;
static REVISIONS: OnceLock<Revisions> = OnceLock::new();

pub(crate) fn files_changed(directory: &Path) {
    if let Some(revision) = REVISIONS
        .get()
        .and_then(|profiles| lock(profiles).get(directory).and_then(Weak::upgrade))
    {
        revision.fetch_add(1, Ordering::AcqRel);
        super::notify_listeners();
    }
}

pub(crate) struct Reader {
    directory: PathBuf,
    revision: Arc<AtomicU64>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PathPolicy {
    ignore_path: bool,
    revision: u64,
}

impl Reader {
    pub(crate) fn new(directory: PathBuf) -> Self {
        let mut profiles = lock(REVISIONS.get_or_init(Mutex::default));
        profiles.retain(|_, revision| revision.strong_count() > 0);
        let registered = profiles.entry(directory.clone()).or_default();
        let revision = registered.upgrade().unwrap_or_else(|| {
            let revision = Arc::new(AtomicU64::new(0));
            *registered = Arc::downgrade(&revision);
            revision
        });
        Self {
            directory,
            revision,
            state: Arc::default(),
        }
    }

    fn policy(&self, playback: &Playback) -> Option<PathPolicy> {
        Some(PathPolicy {
            ignore_path: playback.ignore_path_in_watch_later_config?,
            revision: self.revision.load(Ordering::Acquire),
        })
    }

    pub(crate) fn positions(&self, playback: &Playback) -> Option<Arc<Positions>> {
        let state = lock(&self.state);
        let (policy, positions) = state.saved.as_ref()?;
        (*policy == self.policy(playback)?).then(|| positions.clone())
    }

    pub(crate) fn refresh(&self, app: &PlayerWindow, playback: &Playback) {
        let mut state = lock(&self.state);
        let Some(policy) = self
            .policy(playback)
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
        let revision = self.revision.clone();
        let weak = app.as_weak();
        if let Err(error) = background::spawn_named("riina-playlist-resume", move || {
            if read_latest(&shared, &directory, &revision) {
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

    fn is_current(&self, request: &Arc<Request>, revision: &AtomicU64) -> bool {
        self.request
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, request))
            && request.policy.revision == revision.load(Ordering::Acquire)
    }
}

fn read_latest(shared: &Mutex<State>, directory: &Path, revision: &AtomicU64) -> bool {
    loop {
        let request = {
            let mut state = lock(shared);
            let Some(request) = state
                .request
                .clone()
                .filter(|request| state.is_current(request, revision))
            else {
                state.running = false;
                return false;
            };
            request
        };
        let mut positions = Positions::new();
        for filename in &request.filenames {
            if !lock(shared).is_current(&request, revision) {
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
        if state.is_current(&request, revision) {
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
        let stream = "https://MEDIA.example:443/play/../video.mkv";
        let key = playback_history_store::watch_later_key(filename, true).unwrap();
        let path = directory.join(key.as_str());
        std::fs::write(path, "start=7.5\n").unwrap();
        let stream_key = playback_history_store::watch_later_key(stream, true).unwrap();
        let stream_path = directory.join(stream_key.as_str());
        std::fs::write(stream_path, "# video.mkv\nstart=15\n").unwrap();
        let revision = AtomicU64::new(0);
        let request = Arc::new(Request {
            filenames: HashSet::from([filename.into(), stream.into(), "missing.mkv".into()]),
            policy: PathPolicy {
                ignore_path: true,
                revision: 0,
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
        assert!(read_latest(&shared, directory, &revision));
        assert_eq!(
            lock(&shared).saved.as_ref().unwrap().1.as_ref(),
            &Positions::from([(filename.into(), 7.5), (stream.into(), 15.0)])
        );
        assert!(!lock(&shared).running);

        let mut state = lock(&shared);
        state.clear();
        assert!(!state.is_current(&request, &revision));
        assert!(state.saved.is_some());
    }

    #[test]
    fn a_file_change_invalidates_old_results_only_for_the_shared_profile() {
        let temp = TempDir::new("playlist-resume-profile").unwrap();
        let directory = temp.path().join("one");
        let peer = Reader::new(directory.clone());
        let reader = Reader::new(directory.clone());
        let other = Reader::new(temp.path().join("two"));
        let playback = Playback {
            ignore_path_in_watch_later_config: Some(false),
            ..Playback::default()
        };
        let request = Arc::new(Request {
            filenames: HashSet::from(["movie.mkv".into()]),
            policy: reader.policy(&playback).unwrap(),
        });
        for reader in [&reader, &other] {
            let mut state = lock(&reader.state);
            state.request = Some(request.clone());
            state.saved = Some((request.policy, Arc::default()));
        }
        assert!(reader.positions(&playback).is_some());
        files_changed(&directory);
        assert!(reader.positions(&playback).is_none());
        assert_eq!(peer.policy(&playback), reader.policy(&playback));
        assert!(!read_latest(&reader.state, &directory, &reader.revision));
        assert!(other.positions(&playback).is_some());
    }
}
