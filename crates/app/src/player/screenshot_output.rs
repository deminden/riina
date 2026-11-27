//! Capture side effects cannot be coalesced with disposable playback snapshots.
use riina_core::Playback;
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Mutex,
};
pub(super) mod decode;
pub(crate) mod presentation;
/// The worker retains this owner through native teardown, including captures
/// whose completion never reaches a closing UI event loop.
#[derive(Default)]
pub(crate) struct Scratch {
    directory: Mutex<Option<tempfile::TempDir>>,
}
impl Scratch {
    pub(crate) fn directory(&self) -> io::Result<PathBuf> {
        let mut owned = self
            .directory
            .lock()
            .map_err(|_| io::Error::other("Screenshot temporary directory lock failed"))?;
        if let Some(directory) = &*owned {
            return Ok(directory.path().to_path_buf());
        }
        let mut builder = tempfile::Builder::new();
        builder.prefix("riina-captures-");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            builder.permissions(fs::Permissions::from_mode(0o700));
        }
        let directory = builder.tempdir()?;
        let path = directory.path().to_path_buf();
        *owned = Some(directory);
        Ok(path)
    }

    fn existing_directory(&self) -> Option<PathBuf> {
        Some(self.directory.lock().ok()?.as_ref()?.path().to_path_buf())
    }

    pub(crate) fn remove_completed(&self, paths: &[String]) {
        if paths.is_empty() {
            return;
        }
        let Some(directory) = self
            .existing_directory()
            .and_then(|path| path.canonicalize().ok())
        else {
            return;
        };
        // Settings can change while a screenshot is being written. Remove
        // only completed output inside the directory this player owns.
        for path in paths {
            if let Ok(path) = Path::new(path).canonicalize()
                && path.starts_with(&directory)
                && let Err(error) = fs::remove_file(path)
                && error.kind() != io::ErrorKind::NotFound
            {
                crate::diagnostics::warn!("Could not remove a clipboard-only screenshot: {error}");
            }
        }
    }

    fn owns(&self, path: &Path) -> bool {
        let Some(directory) = self.existing_directory() else {
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
        if let Ok(directory) = self.directory.get_mut()
            && let Some(directory) = directory.take()
            && let Err(error) = directory.close()
            && error.kind() != io::ErrorKind::NotFound
        {
            crate::diagnostics::warn!("Could not remove the private capture directory: {error}");
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
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
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
