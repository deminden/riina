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
    unsafe { SetCurrentProcessExplicitAppUserModelID(PCWSTR(app_id.as_ptr())) }
        .map_err(io::Error::other)
}

pub fn remember(path: &str) -> io::Result<()> {
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

fn recent_media_file_path(path: &str) -> Option<PathBuf> {
    let path = Path::new(path);
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
    std::thread::Builder::new()
        .name("clear-recent-destinations".into())
        .spawn(clear_on_shell_apartment)?
        .join()
        .map_err(|_| io::Error::other("Windows Shell recent-document worker panicked"))?
}

fn clear_on_shell_apartment() -> io::Result<()> {
    // Jump List interfaces require COM. Use a dedicated STA so the caller's UI
    // apartment and its event loop are unaffected by Shell initialization.
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
        .ok()
        .map_err(io::Error::other)?;
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe { CoUninitialize() };
        }
    }
    let _apartment = Apartment;
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
    use std::{fs, time::SystemTime};

    #[test]
    fn recent_document_tracking_ignores_urls_and_missing_paths() {
        let temp = std::env::temp_dir().join(format!(
            "riina-recent-document-{}-{}.mkv",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = temp.to_string_lossy().into_owned();

        assert!(super::recent_media_file_path("https://example.com/video.mkv").is_none());
        assert!(super::recent_media_file_path(&path).is_none());

        fs::write(&temp, b"fixture").unwrap();
        assert_eq!(
            super::recent_media_file_path(&path),
            Some(temp.canonicalize().unwrap())
        );
        fs::remove_file(temp).unwrap();
    }
}
