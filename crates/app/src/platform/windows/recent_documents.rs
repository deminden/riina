use std::{
    io,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
};
use windows::{
    Win32::{
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize,
        },
        UI::Shell::{
            ApplicationDestinations, IApplicationDestinations, SHARD_PATHW, SHAddToRecentDocs,
            SetCurrentProcessExplicitAppUserModelID,
        },
    },
    core::PCWSTR,
};

const APP_USER_MODEL_ID: &str = "org.riina.player";

pub fn initialize() -> io::Result<()> {
    let app_id = wide(APP_USER_MODEL_ID);
    // SAFETY: app_id is NUL-terminated UTF-16 and survives the process-identity call.
    unsafe { SetCurrentProcessExplicitAppUserModelID(PCWSTR(app_id.as_ptr())) }
        .map_err(io::Error::other)
}

pub fn remember(path: &Path) -> io::Result<()> {
    let Some(path) = recent_media_file_path(path) else {
        return Ok(());
    };
    let path = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    // SAFETY: SHARD_PATHW receives a live, NUL-terminated UTF-16 filesystem path.
    unsafe { SHAddToRecentDocs(SHARD_PATHW.0 as u32, Some(path.as_ptr().cast())) };
    Ok(())
}

fn recent_media_file_path(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        path.canonicalize().ok()
    } else {
        None
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

pub fn clear() -> io::Result<()> {
    crate::background::spawn_named("clear-recent-destinations", clear_on_shell_apartment)?
        .join()
        .map_err(|_| io::Error::other("Windows Shell recent-document worker panicked"))?
}

fn clear_on_shell_apartment() -> io::Result<()> {
    // Jump List interfaces require COM. Use a dedicated STA so the caller's UI
    // apartment and its event loop are unaffected by Shell initialization.
    // SAFETY: the reserved argument is null and the guard balances success on this thread.
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
        .ok()
        .map_err(io::Error::other)?;
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            // SAFETY: the guard is created only after successful initialization on this thread.
            unsafe { CoUninitialize() };
        }
    }
    let _apartment = Apartment;
    // SAFETY: COM is initialized on this thread and the Shell class supports this interface.
    let destinations: IApplicationDestinations =
        unsafe { CoCreateInstance(&ApplicationDestinations, None, CLSCTX_INPROC_SERVER) }
            .map_err(io::Error::other)?;
    let app_id = wide(APP_USER_MODEL_ID);
    // SAFETY: the explicit app identity matches the process ID used for recent
    // file additions; the COM interface remains retained for both calls.
    unsafe {
        destinations
            .SetAppID(windows::core::PCWSTR(app_id.as_ptr()))
            .map_err(io::Error::other)?;
        destinations
            .RemoveAllDestinations()
            .map_err(io::Error::other)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use riina_test_support::TempDir;
    use std::fs;

    #[test]
    fn recent_document_tracking_ignores_urls_and_missing_paths() {
        let directory = TempDir::new("recent-document").unwrap();
        let temp = directory.path().join("recent.mkv");
        assert!(super::recent_media_file_path(&temp).is_none());

        fs::write(&temp, b"fixture").unwrap();
        assert_eq!(
            super::recent_media_file_path(&temp),
            Some(temp.canonicalize().unwrap())
        );
    }
}
