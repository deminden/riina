//! The STA owns the Shell dialog; its owner token retains the actual native window.
use slint::winit_030::winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};
use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf, sync::Arc};
use windows::Win32::{
    Foundation::HWND,
    System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
        CoTaskMemFree, CoUninitialize,
    },
    UI::Shell::{
        Common::COMDLG_FILTERSPEC, FOS_ALLOWMULTISELECT, FOS_FORCEFILESYSTEM, FOS_OVERWRITEPROMPT,
        FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, FileOpenDialog, FileSaveDialog, IFileDialog,
        IFileOpenDialog, IFileSaveDialog, IShellItem, SIGDN_FILESYSPATH,
    },
};
use windows::core::{PCWSTR, w};

pub struct DialogOwner {
    _window: Arc<Window>,
    handle: isize,
}

impl DialogOwner {
    pub fn new(window: Arc<Window>) -> Result<Self, String> {
        // winit exposes the borrowed handle only on its event thread. Capture
        // its value here; the retained owner and close gate keep it valid in Show.
        let handle = match window
            .window_handle()
            .map_err(|error| error.to_string())?
            .as_raw()
        {
            RawWindowHandle::Win32(handle) => handle.hwnd.get(),
            _ => return Err("File dialogs require a Windows owner".into()),
        };
        Ok(Self {
            _window: window,
            handle,
        })
    }
}

pub(super) fn window_enabled(window: &Window) -> bool {
    let Ok(handle) = window.window_handle() else {
        return false;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return false;
    };
    // SAFETY: the retained window lends a live HWND on its event thread.
    unsafe {
        windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled(HWND(
            handle.hwnd.get() as *mut _
        ))
        .as_bool()
    }
}

pub fn open_files(owner: &DialogOwner, multiple: bool, title: &str) -> Result<Vec<String>, String> {
    open_selection(owner, multiple, false, title)
}

pub fn pick_folder(owner: &DialogOwner) -> Result<Option<String>, String> {
    Ok(
        open_selection(owner, false, true, "Choose screenshot save path")?
            .into_iter()
            .next(),
    )
}

fn open_selection(
    owner: &DialogOwner,
    multiple: bool,
    folders: bool,
    title: &str,
) -> Result<Vec<String>, String> {
    let _apartment = Apartment::enter()?;
    // SAFETY: the apartment and retained owner outlive the dialog and its results.
    unsafe {
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| e.to_string())?;
        let mut options =
            dialog.GetOptions().map_err(|error| error.to_string())? | FOS_FORCEFILESYSTEM;
        if multiple {
            options |= FOS_ALLOWMULTISELECT;
        }
        if folders {
            options |= FOS_PICKFOLDERS;
        }
        dialog
            .SetOptions(options)
            .map_err(|error| error.to_string())?;
        let title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
        dialog
            .SetTitle(PCWSTR(title.as_ptr()))
            .map_err(|error| error.to_string())?;
        if !show(&dialog, owner)? {
            return Ok(Vec::new());
        }
        let items = dialog.GetResults().map_err(|error| error.to_string())?;
        let mut result = Vec::new();
        for index in 0..items.GetCount().map_err(|error| error.to_string())? {
            let item = items.GetItemAt(index).map_err(|error| error.to_string())?;
            result.push(
                file_path(&item)?
                    .into_os_string()
                    .into_string()
                    .map_err(|_| "The path contains invalid Unicode")?,
            );
        }
        Ok(result)
    }
}

pub(crate) fn playlist_destination(owner: &DialogOwner) -> Result<Option<PathBuf>, String> {
    let _apartment = Apartment::enter()?;
    // SAFETY: the owner and the thread's apartment outlive the shell dialog.
    // The filter and all strings point to static UTF-16 data.
    unsafe {
        let dialog: IFileSaveDialog = CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)
            .map_err(|error| error.to_string())?;
        let configure = || {
            let options = dialog.GetOptions()?
                | FOS_FORCEFILESYSTEM
                | FOS_PATHMUSTEXIST
                | FOS_OVERWRITEPROMPT;
            dialog.SetOptions(options)?;
            dialog.SetTitle(w!("Save to Playlist"))?;
            dialog.SetFileTypes(&[COMDLG_FILTERSPEC {
                pszName: w!("M3U8 playlist"),
                pszSpec: w!("*.m3u8"),
            }])?;
            dialog.SetDefaultExtension(w!("m3u8"))?;
            dialog.SetFileName(w!("playlist.m3u8"))
        };
        configure().map_err(|error| error.to_string())?;
        if !show(&dialog, owner)? {
            return Ok(None);
        }
        let item = dialog.GetResult().map_err(|error| error.to_string())?;
        file_path(&item).map(Some)
    }
}

fn show(dialog: &IFileDialog, owner: &DialogOwner) -> Result<bool, String> {
    // SAFETY: the token retains the dialog's native owner until Show returns.
    match unsafe { dialog.Show(Some(HWND(owner.handle as *mut _))) } {
        Ok(()) => Ok(true),
        Err(error) if error.code().0 as u32 == 0x8007_04c7 => Ok(false),
        Err(error) => Err(error.to_string()),
    }
}

fn file_path(item: &IShellItem) -> Result<PathBuf, String> {
    // SAFETY: COM gives the caller a terminated task-allocated buffer. Copy its
    // contents before freeing it, keeping the filesystem path in native encoding.
    unsafe {
        let path = item
            .GetDisplayName(SIGDN_FILESYSPATH)
            .map_err(|error| error.to_string())?;
        let result = OsString::from_wide(path.as_wide());
        CoTaskMemFree(Some(path.0.cast()));
        Ok(result.into())
    }
}

struct Apartment;

impl Apartment {
    fn enter() -> Result<Self, String> {
        // SAFETY: the guard is created only after COM initializes this thread.
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
            .ok()
            .map(|()| Self)
            .map_err(|error| error.to_string())
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: the private guard remains on the thread that initialized COM.
        unsafe { CoUninitialize() }
    }
}
