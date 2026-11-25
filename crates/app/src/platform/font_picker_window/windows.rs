use slint::winit_030::winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};
use windows::Win32::{
    Foundation::{ERROR_SUCCESS, GetLastError, HWND, SetLastError},
    UI::{
        Input::KeyboardAndMouse::{EnableWindow, IsWindowEnabled},
        WindowsAndMessaging::{GWLP_HWNDPARENT, IsWindow, SetWindowLongPtrW},
    },
};

pub(crate) struct Modal {
    owner: HWND,
    picker: HWND,
    previous_owner: isize,
}

impl Drop for Modal {
    fn drop(&mut self) {
        // SAFETY: this guard is released on the window thread before the picker closes.
        // The checks also handle application shutdown closing either window first.
        unsafe {
            if IsWindow(Some(self.owner)).as_bool() {
                let _ = EnableWindow(self.owner, true);
            }
            if IsWindow(Some(self.picker)).as_bool() {
                SetWindowLongPtrW(self.picker, GWLP_HWNDPARENT, self.previous_owner);
            }
        }
    }
}

pub(crate) fn attach(owner: &Window, picker: &Window) -> Result<Modal, String> {
    let (Some(owner), Some(picker)) = (hwnd(owner), hwnd(picker)) else {
        return Err("The Settings font chooser window is unavailable".into());
    };
    // SAFETY: the handles identify the two realized top-level windows on this thread.
    // GWLP_HWNDPARENT sets top-level ownership without changing their window class.
    unsafe {
        if !IsWindowEnabled(owner).as_bool() {
            return Err("Close the current Settings dialog before choosing a font".into());
        }
        SetLastError(ERROR_SUCCESS);
        let previous_owner = SetWindowLongPtrW(picker, GWLP_HWNDPARENT, owner.0 as isize);
        if previous_owner == 0 && GetLastError() != ERROR_SUCCESS {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let _ = EnableWindow(owner, false);
        Ok(Modal {
            owner,
            picker,
            previous_owner,
        })
    }
}

fn hwnd(window: &Window) -> Option<HWND> {
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(HWND(handle.hwnd.get() as _)),
        _ => None,
    }
}
