//! Resize the framed owner once and let native alignment move its child surfaces.
use super::{PhysicalPosition, PhysicalSize, Window};
use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOZORDER, SetWindowPos,
    },
};

pub(super) fn apply_frame(
    window: &Window,
    position: Option<PhysicalPosition<i32>>,
    size: PhysicalSize<u32>,
) -> Result<(), String> {
    let handle = window.window_handle().map_err(|error| error.to_string())?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Ok(());
    };
    let mut flags = SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOZORDER;
    if position.is_none() {
        flags |= SWP_NOMOVE;
    }
    let origin = position.unwrap_or_default();
    // SAFETY: the borrowed HWND is live and this call runs on its UI thread.
    unsafe {
        SetWindowPos(
            HWND(handle.hwnd.get() as *mut _),
            None,
            origin.x,
            origin.y,
            size.width as i32,
            size.height as i32,
            flags,
        )
    }
    .map_err(|error| error.to_string())
}
