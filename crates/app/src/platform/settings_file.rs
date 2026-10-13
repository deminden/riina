//! Only complete, synced replacements may become the active preferences file.
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub(crate) fn save(path: &Path, contents: &str) -> std::io::Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let (pending, mut file) = loop {
        let sequence = NEXT.fetch_add(1, Ordering::Relaxed);
        let candidate = path.with_extension(format!("{}.{sequence}.tmp", std::process::id()));
        // Exclusive creation defines cleanup ownership. Concurrent sessions and
        // leftovers from an earlier process must never be truncated or removed.
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => break (PendingReplacement(candidate), file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };
    file.write_all(contents.as_bytes())?;
    file.sync_all()?;
    drop(file);
    replace(&pending.0, path)?;
    #[cfg(target_os = "linux")]
    if let Some(parent) = path.parent() {
        std::fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
}

struct PendingReplacement(PathBuf);
impl Drop for PendingReplacement {
    fn drop(&mut self) {
        // Successful replacement already consumed this name. Failure removes only
        // the exclusively owned temporary file, preserving the previous settings.
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(target_os = "windows")]
fn replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    // Windows can reject overlapping replacements of the same destination even
    // when each writer owns a distinct source. Serialize the native commit within
    // this process; external sharing/permission errors still reach the caller.
    static COMMIT: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _commit = COMMIT
        .lock()
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    use std::os::windows::ffi::OsStrExt;
    use windows::{
        Win32::Storage::FileSystem::{
            MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
        },
        core::PCWSTR,
    };
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    for retry in 0..=5 {
        // SAFETY: both owned UTF-16 strings remain terminated and live through
        // this same-directory atomic replacement; no native pointer escapes.
        match unsafe {
            MoveFileExW(
                PCWSTR(source.as_ptr()),
                PCWSTR(destination.as_ptr()),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } {
            Ok(()) => return Ok(()),
            Err(error)
                if retry < 5 && matches!(error.code().0 as u32, 0x8007_0005 | 0x8007_0020) =>
            {
                // Defender and indexers can briefly open a just-synced temp file.
                // Retry only the Win32 access/sharing errors, then preserve the
                // previous settings and report persistent failures normally.
                std::thread::sleep(std::time::Duration::from_millis(2_u64 << retry));
            }
            Err(error) => return Err(std::io::Error::other(error)),
        }
    }
    unreachable!("replacement retry loop always returns or exhausts its retries")
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    std::fs::rename(source, destination)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn directory(name: &str) -> PathBuf {
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/settings-test")
            .join(format!(
                "{name}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[test]
    fn concurrent_saves_publish_complete_settings_and_preserve_foreign_temporary_files() {
        let directory = directory("concurrent");
        let path = directory.join("preferences.json");
        let foreign = path.with_extension(format!("{}.tmp", std::process::id()));
        std::fs::write(&foreign, b"another operation owns this file").unwrap();
        let payloads: Vec<String> = (0..8).map(|index| serde_json::json!({"writer":index,"template":"Снимок 字-%F-%n","data":"abc".repeat(8192)}).to_string()).collect();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(payloads.len()));
        let threads: Vec<_> = payloads
            .iter()
            .cloned()
            .map(|payload| {
                let path = path.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    save(&path, &payload).unwrap();
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(payloads.contains(&saved));
        serde_json::from_str::<serde_json::Value>(&saved).unwrap();
        assert_eq!(
            std::fs::read(&foreign).unwrap(),
            b"another operation owns this file"
        );
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 2);
    }

    #[test]
    fn failed_replacement_preserves_destination_and_removes_owned_temporary_file() {
        let directory = directory("failure");
        let path = directory.join("preferences.json");
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("sentinel"), b"preserve").unwrap();
        assert!(save(&path, "replacement").is_err());
        assert_eq!(std::fs::read(path.join("sentinel")).unwrap(), b"preserve");
        assert_eq!(std::fs::read_dir(directory).unwrap().count(), 1);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn read_only_settings_remain_intact_after_failed_save() {
        let directory = directory("readonly");
        let path = directory.join("preferences.json");
        save(&path, "original").unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        let original_permissions = permissions.clone();
        permissions.set_readonly(true);
        std::fs::set_permissions(&path, permissions).unwrap();
        let result = save(&path, "replacement");
        std::fs::set_permissions(&path, original_permissions).unwrap();
        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "original");
        assert_eq!(std::fs::read_dir(directory).unwrap().count(), 1);
    }
}
