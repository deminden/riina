//! Filesystem and snapshot helpers shared by workspace tests.

use std::{
    ffi::OsStr,
    fmt::Debug,
    fs, io,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(label: &str) -> io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        if Path::new(label).file_name() != Some(OsStr::new(label)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Expected a directory label",
            ));
        }
        let root = std::env::temp_dir();
        loop {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = root.join(format!("riina-{label}-{}-{id}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // The name was acquired exclusively. Cleanup can also run during a test panic.
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn workspace_root() -> io::Result<PathBuf> {
    let current = std::env::current_dir()?;
    // Cargo runs tests from their package directory. Reused binaries may
    // have been compiled in a different checkout.
    workspace_from(&current)
        .or_else(|| workspace_from(Path::new(env!("CARGO_MANIFEST_DIR"))))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Run the test from the Riina workspace",
            )
        })?
        .canonicalize()
}

fn workspace_from(directory: &Path) -> Option<&Path> {
    directory.ancestors().find(|candidate| {
        candidate.join("crates/test-support/Cargo.toml").is_file()
            && candidate.join("tests/fixtures/manifest.json").is_file()
    })
}

pub fn fixture(name: &str) -> io::Result<PathBuf> {
    Ok(workspace_root()?.join("tests/fixtures").join(name))
}

#[track_caller]
#[expect(
    clippy::panic,
    reason = "A stopped test worker must fail the waiting test"
)]
pub fn wait_for<T: Debug>(
    receiver: &mpsc::Receiver<T>,
    timeout: Duration,
    predicate: impl Fn(&T) -> bool,
) -> T {
    let deadline = Instant::now() + timeout;
    let caller = std::panic::Location::caller();
    let mut last = None;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(
            !remaining.is_zero(),
            "Timed out at {caller}; last observed state: {last:?}"
        );
        let state = receiver.recv_timeout(remaining).unwrap_or_else(|error| {
            let reason = match error {
                mpsc::RecvTimeoutError::Timeout => "Timed out",
                mpsc::RecvTimeoutError::Disconnected => "Snapshot stream closed",
            };
            panic!("{reason} at {caller}; last observed state: {last:?}");
        });
        if predicate(&state) {
            return state;
        }
        last = Some(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_nearest_checkout_with_its_own_fixtures() -> io::Result<()> {
        let directory = TempDir::new("worktree-fixtures")?;
        let root = directory.path();
        let checkout = root.join("other-checkout");
        let package = checkout.join("crates/mpv");
        for workspace in [root, checkout.as_path()] {
            let support = workspace.join("crates/test-support");
            let fixtures = workspace.join("tests/fixtures");
            fs::create_dir_all(&support)?;
            fs::create_dir_all(&fixtures)?;
            fs::write(support.join("Cargo.toml"), "")?;
            fs::write(fixtures.join("manifest.json"), "{}")?;
        }
        fs::create_dir_all(&package)?;

        assert_eq!(workspace_from(&package), Some(checkout.as_path()));
        assert_eq!(workspace_from(&root.join("crates/core")), Some(root));
        Ok(())
    }

    #[test]
    fn temp_dirs_are_isolated_and_remove_files_on_drop() -> io::Result<()> {
        let first = TempDir::new("same-label")?;
        let second = TempDir::new("same-label")?;
        assert_ne!(first.path(), second.path());
        let file = first.path().join("fixture");
        fs::write(&file, b"test")?;
        drop(first);
        assert!(!file.exists());
        assert!(second.path().is_dir());
        Ok(())
    }

    #[test]
    fn unwinding_releases_temp_dir() -> io::Result<()> {
        let directory = TempDir::new("panicking-test")?;
        let path = directory.path().to_owned();
        let result = std::panic::catch_unwind(move || {
            let _directory = directory;
            panic!("intentional test failure");
        });
        assert!(result.is_err());
        assert!(!path.exists());
        Ok(())
    }

    #[test]
    #[should_panic(expected = "Snapshot stream closed")]
    fn snapshot_wait_stops_when_the_sender_exits() {
        let (sender, receiver) = mpsc::channel();
        sender.send("last snapshot").unwrap();
        drop(sender);
        wait_for(&receiver, Duration::from_secs(1), |_| false);
    }
}
