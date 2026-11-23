//! Winit's file-drop event carries a path only. Sample the native cursor before batching it.
use slint::{
    LogicalPosition,
    winit_030::{WinitWindowAccessor, winit},
};

pub(crate) fn file_drop_position(window: &slint::Window) -> Option<LogicalPosition> {
    let position = window
        .with_winit_window(physical_cursor_position)
        .flatten()?;
    let scale = window.scale_factor();
    Some(LogicalPosition::new(
        position.0 as f32 / scale,
        position.1 as f32 / scale,
    ))
}

#[cfg(target_os = "windows")]
fn physical_cursor_position(window: &winit::window::Window) -> Option<(f64, f64)> {
    use windows::Win32::{
        Foundation::{HWND, POINT},
        Graphics::Gdi::ScreenToClient,
        UI::WindowsAndMessaging::GetCursorPos,
    };
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let RawWindowHandle::Win32(handle) = window.window_handle().ok()?.as_raw() else {
        return None;
    };
    let hwnd = HWND(handle.hwnd.get() as *mut _);
    let mut point = POINT::default();
    // SAFETY: the HWND is borrowed from a live Winit window and the output is a local POINT.
    let found =
        unsafe { GetCursorPos(&mut point).is_ok() && ScreenToClient(hwnd, &mut point).as_bool() };
    found.then(|| (f64::from(point.x), f64::from(point.y)))
}

#[cfg(target_os = "macos")]
fn physical_cursor_position(window: &winit::window::Window) -> Option<(f64, f64)> {
    use crate::platform::macos::view::{content_view, top_left_point};
    let view = content_view(window)?;
    let location = view.window()?.mouseLocationOutsideOfEventStream();
    let (x, y) = top_left_point(view, location);
    let scale = window.scale_factor();
    Some((x * scale, y * scale))
}

#[cfg(target_os = "linux")]
fn physical_cursor_position(window: &winit::window::Window) -> Option<(f64, f64)> {
    use std::ffi::{c_int, c_uint, c_ulong, c_void};
    use winit::raw_window_handle::{
        HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle,
    };

    #[link(name = "X11")]
    unsafe extern "C" {
        fn XQueryPointer(
            display: *mut c_void,
            window: c_ulong,
            root: *mut c_ulong,
            child: *mut c_ulong,
            root_x: *mut c_int,
            root_y: *mut c_int,
            window_x: *mut c_int,
            window_y: *mut c_int,
            state: *mut c_uint,
        ) -> c_int;
    }

    let RawDisplayHandle::Xlib(display) = window.display_handle().ok()?.as_raw() else {
        return None;
    };
    let RawWindowHandle::Xlib(handle) = window.window_handle().ok()?.as_raw() else {
        return None;
    };
    let (mut root, mut child, mut root_x, mut root_y, mut x, mut y, mut state) = Default::default();
    // SAFETY: both Xlib handles belong to this live Winit window. All outputs
    // point to local values with Xlib's matching integer types.
    let found = unsafe {
        XQueryPointer(
            display.display?.as_ptr(),
            handle.window,
            &mut root,
            &mut child,
            &mut root_x,
            &mut root_y,
            &mut x,
            &mut y,
            &mut state,
        )
    };
    (found != 0).then(|| (f64::from(x), f64::from(y)))
}
