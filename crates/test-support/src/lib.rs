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
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
}

pub fn fixture(name: &str) -> io::Result<PathBuf> {
    Ok(workspace_root()?.join("tests/fixtures").join(name))
}

#[track_caller]
pub fn wait_for<T: Debug>(
    receiver: &mpsc::Receiver<T>,
    timeout: Duration,
    predicate: impl Fn(&T) -> bool,
) -> T {
    let deadline = Instant::now() + timeout;
    let caller = std::panic::Location::caller();
    let mut last = None;
    loop {
        assert!(
            Instant::now() < deadline,
            "Timed out at {caller}; last observed state: {last:?}"
        );
        if let Ok(state) = receiver.recv_timeout(Duration::from_millis(200)) {
            if predicate(&state) {
                return state;
            }
            last = Some(state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
