//! Move a file or directory without replacing a destination created by a concurrent writer.
use std::{io, path::Path};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::replace;

#[cfg(unix)]
pub(crate) use std::fs::rename as replace;

#[cfg(windows)]
pub(crate) fn move_new(source: &Path, destination: &Path) -> io::Result<()> {
    use ::windows::{
        Win32::Storage::FileSystem::{MOVE_FILE_FLAGS, MoveFileExW},
        core::PCWSTR,
    };
    use std::os::windows::ffi::OsStrExt;

    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    // SAFETY: the validated paths are NUL-terminated and borrowed only for this call.
    unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(destination.as_ptr()),
            MOVE_FILE_FLAGS(0),
        )
    }
    .map_err(|error| match error.code().0 as u32 {
        0x8007_0050 | 0x8007_00b7 => io::ErrorKind::AlreadyExists.into(),
        _ => io::Error::other(error),
    })
}

#[cfg(unix)]
pub(crate) fn move_new(source: &Path, destination: &Path) -> io::Result<()> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};

    let source = CString::new(source.as_os_str().as_bytes())?;
    let destination = CString::new(destination.as_os_str().as_bytes())?;
    // SAFETY: the two C strings are live and terminated for the native rename.
    let result = unsafe {
        #[cfg(target_os = "linux")]
        let result = libc::renameat2(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            destination.as_ptr(),
            libc::RENAME_NOREPLACE,
        );
        #[cfg(target_os = "macos")]
        let result = libc::renamex_np(source.as_ptr(), destination.as_ptr(), libc::RENAME_EXCL);
        result
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
