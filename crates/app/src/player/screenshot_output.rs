//! Capture side effects cannot be coalesced with disposable playback snapshots.
use riina_core::Playback;
pub(super) mod decode;
pub(crate) mod presentation;
/// The worker retains this owner through native teardown, including captures
/// whose completion never reaches a closing UI event loop.
#[derive(Default)]
pub(crate) struct Scratch {
    directory: std::sync::Mutex<Option<std::path::PathBuf>>,
}
impl Scratch {
    pub(crate) fn directory(&self) -> std::io::Result<std::path::PathBuf> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let mut owned = self
            .directory
            .lock()
            .map_err(|_| std::io::Error::other("Screenshot temporary directory lock failed"))?;
        if let Some(path) = &*owned {
            return Ok(path.clone());
        }
        let root = std::env::temp_dir();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(std::io::Error::other)?
            .as_nanos();
        loop {
            let path = root.join(format!(
                "riina-captures-{}-{stamp}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => {
                    *owned = Some(path.clone());
                    return Ok(path);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }

    pub(crate) fn remove_completed(&self, paths: &[String]) {
        if paths.is_empty() {
            return;
        }
        let Ok(Some(directory)) = self.directory.lock().map(|owned| owned.clone()) else {
            return;
        };
        let Ok(directory) = directory.canonicalize() else {
            return;
        };
        // Settings can change while a screenshot is being written. Remove
        // only completed output inside the directory this player owns.
        for path in paths {
            if let Ok(path) = std::path::Path::new(path).canonicalize()
                && path.starts_with(&directory)
                && let Err(error) = std::fs::remove_file(path)
                && error.kind() != std::io::ErrorKind::NotFound
            {
                crate::diagnostics::warn!("Could not remove a clipboard-only screenshot: {error}");
            }
        }
    }

    fn owns(&self, path: &std::path::Path) -> bool {
        let Ok(Some(directory)) = self.directory.lock().map(|owned| owned.clone()) else {
            return false;
        };
        path.starts_with(&directory)
            || directory
                .canonicalize()
                .ok()
                .zip(path.canonicalize().ok())
                .is_some_and(|(directory, path)| path.starts_with(directory))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        if let Ok(Some(path)) = self.directory.get_mut() {
            // Only the exclusively created directory is owned. Never recurse
            // through a configured save folder or a directory found by scanning.
            if let Err(error) = std::fs::remove_dir_all(path)
                && error.kind() != std::io::ErrorKind::NotFound
            {
                crate::diagnostics::warn!(
                    "Could not remove the private capture directory: {error}"
                );
            }
        }
    }
}
#[derive(Default)]
pub(crate) struct Completions {
    count: u64,
    completed: Vec<Capture>,
}

#[derive(Debug)]
pub(crate) struct Capture {
    pub(crate) generation: u64,
    pub(crate) sequence: u64,
    pub(crate) path: String,
}
impl Completions {
    pub(crate) fn observe(&mut self, state: &Playback) {
        if self.count != state.completed_screenshots {
            self.count = state.completed_screenshots;
            if let Some(path) = &state.screenshot_file {
                self.completed.push(Capture {
                    generation: state.generation,
                    sequence: self.count,
                    path: path.clone(),
                });
            }
        }
    }
    pub(crate) fn take(&mut self) -> Vec<Capture> {
        std::mem::take(&mut self.completed)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scratch_outlives_ui_and_removes_unreported_worker_output() {
        let ui = std::sync::Arc::new(Scratch::default());
        let worker = ui.clone();
        let other_session = Scratch::default();
        let path = ui.directory().unwrap();
        let other = other_session.directory().unwrap();
        assert_ne!(path, other);
        assert_eq!(worker.directory().unwrap(), path);
        drop(ui);
        assert!(path.is_dir());
        // Simulate native output after UI teardown without a delivered reply.
        std::fs::write(path.join("undelivered.png"), b"frame").unwrap();
        std::fs::write(other.join("retained.png"), b"other session").unwrap();
        drop(worker);
        assert!(!path.exists());
        assert_eq!(
            std::fs::read(other.join("retained.png")).unwrap(),
            b"other session"
        );
        drop(other_session);
        assert!(!other.exists());
    }
    #[test]
    fn cleanup_removes_only_confirmed_files_owned_by_that_players_scratch() {
        let scratch = Scratch::default();
        let own_directory = scratch.directory().unwrap();
        let other_scratch = Scratch::default();
        let other_directory = other_scratch.directory().unwrap();
        let capture = own_directory.join("confirmed.png");
        let unrelated = own_directory.join("unreported.png");
        let other = other_directory.join("saved.png");
        std::fs::write(&capture, b"confirmed").unwrap();
        std::fs::write(&unrelated, b"unreported").unwrap();
        std::fs::write(&other, b"preserve").unwrap();
        scratch.remove_completed(&[
            capture.to_string_lossy().into_owned(),
            other.to_string_lossy().into_owned(),
        ]);
        assert!(!capture.exists());
        assert!(unrelated.is_file());
        assert_eq!(std::fs::read(other).unwrap(), b"preserve");
    }
    #[test]
    fn completed_paths_survive_snapshot_coalescing_and_failures_without_replay() {
        let mut queue = Completions::default();
        let mut state = Playback::default();
        queue.observe(&state);
        for (count, path) in [(1, "first.png"), (2, "second.png")] {
            state.completed_screenshots = count;
            state.screenshot_file = Some(path.into());
            queue.observe(&state);
            queue.observe(&state);
        }
        state.notices.push(
            riina_core::NoticeSeverity::Error,
            "next capture failed".into(),
        );
        queue.observe(&state);
        state.begin_load();
        queue.observe(&state);
        let captures = queue.take();
        assert_eq!(
            captures.iter().map(|c| c.path.as_str()).collect::<Vec<_>>(),
            ["first.png", "second.png"]
        );
        assert_eq!(captures[1].sequence, 2);
        assert!(queue.take().is_empty());
        state.completed_screenshots = 3;
        state.screenshot_file = Some("new-source.png".into());
        queue.observe(&state);
        let captures = queue.take();
        assert_eq!(captures[0].path, "new-source.png");
        assert_eq!(captures[0].generation, state.generation);
    }
}
