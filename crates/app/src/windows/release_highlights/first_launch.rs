//! Claim the automatic introduction once per profile and release version.
use std::{
    fs::{self, File, OpenOptions, TryLockError},
    io,
    path::{Path, PathBuf},
};

pub(super) struct FirstLaunch {
    marker: PathBuf,
    _guard: File,
}

impl FirstLaunch {
    pub(super) fn claim(profile: &Path, version: &str) -> io::Result<Option<Self>> {
        let directory = profile.join("release-highlights");
        fs::create_dir_all(&directory)?;
        let marker = directory.join(format!("{version}.seen"));
        if marker.try_exists()? {
            return Ok(None);
        }
        let guard = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join("startup.lock"))?;
        match guard.try_lock() {
            Ok(()) => Ok((!marker.try_exists()?).then_some(Self {
                marker,
                _guard: guard,
            })),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(error)) => Err(error),
        }
    }

    pub(super) fn record_shown(self) -> io::Result<()> {
        // Hold the process lock until the first successful show is recorded.
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.marker)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_test_support::TempDir;

    #[test]
    fn a_version_is_remembered_once_after_the_guide_opens() -> io::Result<()> {
        let profile = TempDir::new("guide-first-launch")?;
        let spot = profile.path();
        let pending = FirstLaunch::claim(spot, "4.2.1")?.unwrap();
        assert!(FirstLaunch::claim(spot, "4.2.1")?.is_none());
        drop(pending);
        FirstLaunch::claim(spot, "4.2.1")?.unwrap().record_shown()?;
        assert!(FirstLaunch::claim(spot, "4.2.1")?.is_none());
        assert!(FirstLaunch::claim(spot, "4.2.2")?.is_some());
        Ok(())
    }
}
