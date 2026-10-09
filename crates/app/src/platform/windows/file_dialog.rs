//! The STA owns the Shell dialog; its owner token retains the actual native window.
use slint::winit_030::winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};
use std::sync::Arc;
use windows::Win32::{
    Foundation::HWND,
    System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
        CoTaskMemFree, CoUninitialize,
    },
    UI::Shell::{
        FOS_ALLOWMULTISELECT, FOS_FORCEFILESYSTEM, FileOpenDialog, IFileOpenDialog,
        SIGDN_FILESYSPATH,
    },
};
use windows::core::PCWSTR;

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

pub fn open_files(owner: &DialogOwner, multiple: bool, title: &str) -> Result<Vec<String>, String> {
    let hwnd = HWND(owner.handle as *mut _);
    // SAFETY: the caller retains the native owner for this entire modal operation.
    // This dedicated STA balances COM setup and keeps interfaces on one thread.
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|e| e.to_string())?;
        struct Apartment;
        impl Drop for Apartment {
            fn drop(&mut self) {
                unsafe {
                    CoUninitialize();
                }
            }
        }
        let _apartment = Apartment;
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| e.to_string())?;
        let mut options =
            dialog.GetOptions().map_err(|error| error.to_string())? | FOS_FORCEFILESYSTEM;
        if multiple {
            options |= FOS_ALLOWMULTISELECT;
        }
        dialog
            .SetOptions(options)
            .map_err(|error| error.to_string())?;
        let title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
        dialog
            .SetTitle(PCWSTR(title.as_ptr()))
            .map_err(|error| error.to_string())?;
        if let Err(error) = dialog.Show(Some(hwnd)) {
            if error.code().0 as u32 == 0x800704c7 {
                return Ok(Vec::new());
            }
            return Err(error.to_string());
        }
        let items = dialog.GetResults().map_err(|error| error.to_string())?;
        let mut result = Vec::new();
        for index in 0..items.GetCount().map_err(|error| error.to_string())? {
            let item = items.GetItemAt(index).map_err(|error| error.to_string())?;
            let path = item
                .GetDisplayName(SIGDN_FILESYSPATH)
                .map_err(|error| error.to_string())?;
            let text = path.to_string().map_err(|error| error.to_string());
            CoTaskMemFree(Some(path.0.cast()));
            result.push(text?);
        }
        Ok(result)
    }
}
