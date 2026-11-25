//! File checks must not hold the outline, even when a share is unreachable.
use riina_core::sync::lock;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub(super) type Files = HashMap<PathBuf, bool>;

#[derive(Clone, Default)]
pub(super) struct Availability(Arc<Mutex<State>>);

#[derive(Default)]
struct State {
    epoch: u64,
    revision: u64,
    wanted: HashSet<PathBuf>,
    results: Files,
    lanes: [Lane; 2],
}

#[derive(Default)]
struct Lane {
    pending: VecDeque<PathBuf>,
    active: Option<Probe>,
    running: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct Probe {
    path: PathBuf,
    epoch: u64,
}

impl Availability {
    pub(super) fn snapshot(&self) -> Files {
        lock(&self.0).results.clone()
    }

    pub(super) fn revision(&self) -> u64 {
        lock(&self.0).revision
    }

    pub(super) fn clear(&self) {
        let mut state = lock(&self.0);
        state.epoch = state.epoch.wrapping_add(1);
        state.revision = state.revision.wrapping_add(1);
        state.wanted.clear();
        state.results.clear();
        for lane in &mut state.lanes {
            lane.pending.clear();
        }
    }

    pub(super) fn request(&self, paths: Vec<PathBuf>) {
        let mut state = lock(&self.0);
        state.request(paths);
        for (index, lane) in state.lanes.iter_mut().enumerate() {
            if lane.running || lane.pending.is_empty() {
                continue;
            }
            lane.running = true;
            let checks = self.clone();
            if let Err(error) =
                crate::background::spawn_named("riina-history-file-check", move || {
                    checks.run(index);
                })
            {
                lane.running = false;
                crate::diagnostics::warn!("Could not check Playback History files: {error}");
            }
        }
    }

    fn run(&self, index: usize) {
        loop {
            let Some(probe) = lock(&self.0).next(index) else {
                return;
            };
            let available = probe.path.is_file();
            lock(&self.0).finish(index, probe, available);
        }
    }
}

impl State {
    fn request(&mut self, paths: Vec<PathBuf>) {
        let wanted = paths.iter().cloned().collect::<HashSet<_>>();
        if self.wanted == wanted {
            return;
        }
        self.wanted = wanted;
        self.results.retain(|path, _| self.wanted.contains(path));
        for lane in &mut self.lanes {
            lane.pending.clear();
        }
        let mut queued = HashSet::new();
        for path in paths {
            let lane = &mut self.lanes[lane_for(&path)];
            if !self.results.contains_key(&path)
                && !lane
                    .active
                    .as_ref()
                    .is_some_and(|active| active.epoch == self.epoch && active.path == path)
                && queued.insert(path.clone())
            {
                lane.pending.push_back(path);
            }
        }
    }

    fn next(&mut self, index: usize) -> Option<Probe> {
        let lane = &mut self.lanes[index];
        let Some(path) = lane.pending.pop_front() else {
            lane.running = false;
            return None;
        };
        let probe = Probe {
            path,
            epoch: self.epoch,
        };
        lane.active = Some(probe.clone());
        Some(probe)
    }

    fn finish(&mut self, index: usize, probe: Probe, available: bool) {
        self.lanes[index].active = None;
        if probe.epoch == self.epoch && self.wanted.contains(&probe.path) {
            self.results.insert(probe.path, available);
            self.revision = self.revision.wrapping_add(1);
        }
    }
}

fn lane_for(path: &Path) -> usize {
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        // A disconnected UNC share must not delay probes for the local drive.
        usize::from(matches!(
            path.components().next(),
            Some(Component::Prefix(prefix))
                if matches!(prefix.kind(), Prefix::UNC(..) | Prefix::VerbatimUNC(..))
        ))
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obsolete_file_checks_cannot_repopulate_a_closed_or_filtered_outline() {
        let checks = Availability::default();
        let local = PathBuf::from("visible.mkv");
        let hidden = PathBuf::from("hidden.mkv");
        let old = {
            let mut state = lock(&checks.0);
            state.request(vec![hidden.clone(), local.clone()]);
            let pending = state.next(0).unwrap();
            state.request(vec![local.clone()]);
            pending
        };
        let first = {
            let mut state = lock(&checks.0);
            state.finish(0, old, true);
            assert!(state.results.is_empty());
            state.next(0).unwrap()
        };
        checks.clear();
        let mut state = lock(&checks.0);
        state.request(vec![local.clone()]);
        state.finish(0, first, false);
        assert!(state.results.is_empty());
        let current = state.next(0).unwrap();
        state.finish(0, current, true);
        assert_eq!(state.results.get(&local), Some(&true));
    }
}
