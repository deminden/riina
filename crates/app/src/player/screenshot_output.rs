//! Capture side effects cannot be coalesced with disposable playback snapshots.
use riina_core::Playback;
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
}
impl Drop for Scratch {
    fn drop(&mut self) {
        if let Ok(Some(path)) = self.directory.get_mut() {
            // Only the exclusively created directory is owned. Never recurse
            // through a configured save folder or a directory found by scanning.
            let _ = std::fs::remove_dir_all(path);
        }
    }
}
#[derive(Default)]
pub(crate) struct Completions {
    count: u64,
    paths: Vec<String>,
}
impl Completions {
    pub(crate) fn observe(&mut self, state: &Playback) {
        if self.count != state.completed_screenshots {
            self.count = state.completed_screenshots;
            if let Some(path) = &state.screenshot_file {
                self.paths.push(path.clone());
            }
        }
    }
    pub(crate) fn take(&mut self) -> Vec<String> {
        std::mem::take(&mut self.paths)
    }
}
pub(crate) fn copy(app: &crate::PlayerWindow, paths: &[String]) -> Option<String> {
    if !app.get_screenshot_copy_to_clipboard() {
        return None;
    }
    let mut error = None;
    for path in paths {
        // Unsupported image decoding uses the normal screenshot feedback path;
        // it must not introduce an extra error message absent from the reference.
        let Ok(image) = slint::Image::load_from_path(std::path::Path::new(path)) else {
            continue;
        };
        if let Err(reason) = crate::platform::clipboard::copy_image(&image) {
            error = Some(reason);
        }
    }
    error
}
pub(crate) fn cleanup(app: &crate::PlayerWindow, paths: &[String]) {
    cleanup_paths(
        riina_core::screenshot::Destinations {
            save_to_file: app.get_screenshot_save_to_file(),
            copy_to_clipboard: app.get_screenshot_copy_to_clipboard(),
        },
        paths,
    );
}

fn cleanup_paths(destinations: riina_core::screenshot::Destinations, paths: &[String]) {
    if destinations.save_to_file || !destinations.copy_to_clipboard {
        return;
    }
    // Native completion supplies each exact filename. Decode/preview/copy must
    // precede removal; never find the "latest" file in a user's screenshot folder.
    for path in paths {
        let _ = std::fs::remove_file(path);
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
    fn clipboard_only_cleanup_removes_confirmed_files_and_preserves_unrelated_files() {
        use riina_core::screenshot::Destinations;
        let directory = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/screenshot-cleanup-test")
            .join(format!(
                "{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(&directory).unwrap();
        let capture = directory.join("confirmed.png");
        let other = directory.join("unrelated.png");
        std::fs::write(&capture, b"confirmed").unwrap();
        std::fs::write(&other, b"preserve").unwrap();
        let paths = vec![capture.to_string_lossy().into_owned()];
        for copy_to_clipboard in [false, true] {
            cleanup_paths(
                Destinations {
                    save_to_file: true,
                    copy_to_clipboard,
                },
                &paths,
            );
            assert!(capture.is_file());
        }
        cleanup_paths(
            Destinations {
                save_to_file: false,
                copy_to_clipboard: false,
            },
            &paths,
        );
        assert!(capture.is_file());
        cleanup_paths(
            Destinations {
                save_to_file: false,
                copy_to_clipboard: true,
            },
            &paths,
        );
        assert!(!capture.exists());
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
        state.error = Some("next capture failed".into());
        queue.observe(&state);
        state.begin_load();
        queue.observe(&state);
        assert_eq!(queue.take(), ["first.png", "second.png"]);
        assert!(queue.take().is_empty());
        state.completed_screenshots = 3;
        state.screenshot_file = Some("new-source.png".into());
        queue.observe(&state);
        assert_eq!(queue.take(), ["new-source.png"]);
    }
}
