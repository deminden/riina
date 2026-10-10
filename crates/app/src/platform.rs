//! Native services are isolated here; core state and the shared UI own behavior.
pub(crate) mod language;

pub fn screenshot_directory() -> std::io::Result<std::path::PathBuf> {
    let directory = dirs::picture_dir()
        .ok_or_else(|| std::io::Error::other("The Pictures directory is unavailable"))?
        .join("Screenshots");
    std::fs::create_dir_all(&directory)?;
    Ok(directory)
}

pub fn open_directory(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use ::windows::{
            Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
            core::{PCWSTR, w},
        };
        use std::os::windows::ffi::OsStrExt;
        let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: the owned directory string is NUL-terminated through the call;
        // the user's menu action opens the system file manager without a shell.
        let result = unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                PCWSTR(path.as_ptr()),
                None,
                None,
                SW_SHOWNORMAL,
            )
        };
        if result.0 as isize <= 32 {
            return Err(std::io::Error::other("Could not open screenshot folder"));
        }
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        let program = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        std::process::Command::new(program).arg(path).spawn()?;
    }
    Ok(())
}
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub(crate) use windows::{
    NativePresentation, file_dialog, native_window, open_dialog, services::*,
};

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod unix;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) use unix::{NativePresentation, open_dialog, services::*};
