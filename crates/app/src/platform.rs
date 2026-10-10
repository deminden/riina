//! Native services are isolated here; core state and the shared UI own behavior.
pub(crate) mod language;

pub fn screenshot_directory(configured: Option<&str>) -> std::io::Result<std::path::PathBuf> {
    let directory = if let Some(path) = configured {
        if path == "~" || path.starts_with("~/") || path.starts_with("~\\") {
            let home = dirs::home_dir()
                .ok_or_else(|| std::io::Error::other("The home directory is unavailable"))?;
            if path == "~" {
                home
            } else {
                home.join(&path[2..])
            }
        } else {
            std::path::PathBuf::from(path)
        }
    } else {
        dirs::picture_dir()
            .ok_or_else(|| std::io::Error::other("The Pictures directory is unavailable"))?
            .join("Screenshots")
    };
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

pub fn reveal_file(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use ::windows::{
            Win32::{
                System::Com::{
                    COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize,
                },
                UI::Shell::{SHOpenFolderAndSelectItems, SHParseDisplayName},
            },
            core::{HRESULT, PCWSTR},
        };
        use std::os::windows::ffi::OsStrExt;
        let text: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: COM is balanced only when this call acquired an initialization.
        // A pre-existing different apartment already satisfies Shell's COM need.
        let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        let owns_com = initialized.is_ok();
        if !owns_com && initialized != HRESULT(0x80010106_u32 as i32) {
            return Err(std::io::Error::other(initialized.to_string()));
        }
        let mut item = std::ptr::null_mut();
        // SAFETY: the terminated path and output PIDL remain live. Zero selected
        // children means the absolute PIDL itself is the file to reveal.
        let result = unsafe {
            let parsed = SHParseDisplayName(PCWSTR(text.as_ptr()), None, &mut item, 0, None);
            let selected = parsed.and_then(|()| SHOpenFolderAndSelectItems(item, None, 0));
            if !item.is_null() {
                CoTaskMemFree(Some(item.cast()));
            }
            if owns_com {
                CoUninitialize();
            }
            selected
        };
        result.map_err(std::io::Error::other)
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn()?;
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        // FileManager1 selection still needs a verified desktop integration;
        // opening the parent is the current explicit Linux adaptation.
        open_directory(
            path.parent()
                .ok_or_else(|| std::io::Error::other("Screenshot has no parent folder"))?,
        )
    }
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
