//! A visible playlist reads saved resume positions from its own profile.

use crate::{PlayerWindow, background, playback_history_store};
use riina_core::{FileFeedback, Phase, Playback, PlaylistEntry, sync::lock};
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
    playback: PlaybackState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PlaybackState {
    generation: u64,
    loading: bool,
    stopped: bool,
}

impl From<&Playback> for PlaybackState {
    fn from(playback: &Playback) -> Self {
        // Loading a file can consume its saved position. Replacing or
        // stopping playback can write the position of the previous file.
        Self {
            generation: playback.generation,
            loading: playback.file_feedback == FileFeedback::Loading,
            stopped: playback.phase == Phase::Stopped,
        }
    }
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
        let media = PlaybackState::from(playback);
        if state
            .request
            .as_ref()
            .is_some_and(|previous| previous.policy == policy && previous.playback == media)
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
            playback: media,
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
        let stream = "https://MEDIA.example:443/play/../video.mkv";
        let key = playback_history_store::watch_later_key(filename, true).unwrap();
        let path = directory.join(key.as_str());
        std::fs::write(path, "start=7.5\n").unwrap();
        let stream_key = playback_history_store::watch_later_key(stream, true).unwrap();
        let stream_path = directory.join(stream_key.as_str());
        std::fs::write(stream_path, "# video.mkv\nstart=15\n").unwrap();
        let request = Arc::new(Request {
            filenames: HashSet::from([filename.into(), stream.into(), "missing.mkv".into()]),
            policy: PathPolicy {
                ignore_path: true,
                revision: REVISION.load(Ordering::Acquire),
            },
            playback: PlaybackState::from(&Playback::default()),
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
            &Positions::from([(filename.into(), 7.5), (stream.into(), 15.0)])
        );
        assert!(!lock(&shared).running);

        let mut state = lock(&shared);
        state.clear();
        assert!(!state.is_current(&request));
        assert!(state.saved.is_some());
    }

    #[test]
    fn the_same_playlist_can_refresh_after_reopening_or_stopping_a_file() {
        let mut playback = Playback {
            generation: 1,
            phase: Phase::Playing,
            ..Playback::default()
        };
        let playing = PlaybackState::from(&playback);
        playback.position = Some(37.0);
        playback.phase = Phase::Paused;
        assert_eq!(PlaybackState::from(&playback), playing);
        playback.phase = Phase::Stopped;
        assert_ne!(PlaybackState::from(&playback), playing);
        playback.phase = Phase::Loading;
        playback.generation += 1;
        playback.file_feedback = FileFeedback::Loading;
        let loading = PlaybackState::from(&playback);
        assert_ne!(loading, playing);
        // Slow loads can outlast the playlist timer. Playback restart is
        // after mpv has finished applying and removing the resume file.
        playback.phase = Phase::Playing;
        assert_eq!(PlaybackState::from(&playback), loading);
        playback.file_feedback = FileFeedback::Restarted(std::time::Instant::now());
        assert_ne!(PlaybackState::from(&playback), loading);
    }
}
