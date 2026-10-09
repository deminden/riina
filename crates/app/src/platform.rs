use std::{
    io::Write,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
};
use windows::{
    Win32::Storage::FileSystem::{MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW},
    core::PCWSTR,
};

pub fn preferences_path() -> std::io::Result<PathBuf> {
    let data = std::env::var_os("LOCALAPPDATA")
        .ok_or_else(|| std::io::Error::other("LOCALAPPDATA is unavailable"))?;
    let directory = PathBuf::from(data).join("wiina");
    std::fs::create_dir_all(&directory)?;
    Ok(directory.join("preferences.json"))
}

pub fn save_preferences(path: &Path, contents: &str) -> std::io::Result<()> {
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    let mut file = std::fs::File::create(&temporary)?;
    file.write_all(contents.as_bytes())?;
    file.sync_all()?;
    drop(file);
    let source: Vec<u16> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: both UTF-16 strings remain NUL-terminated and live through the call.
    // Atomic replacement leaves the existing preferences intact if the move fails.
    unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(destination.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(std::io::Error::other)
}
