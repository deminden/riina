//! Capture side effects cannot be coalesced with disposable playback snapshots.
use riina_core::Playback;
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
