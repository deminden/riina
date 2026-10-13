//! Aspect constraints belong to the native user resize gesture, not WM_SIZE.

use crate::PlayerWindow;
use riina_core::{
    window_aspect::{self as policy, PixelSize},
    window_resize::{ScreenRect, clamp_position_with_margin},
};
use slint::winit_030::winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow},
    UI::{
        HiDpi::GetDpiForWindow,
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{GetClientRect, GetWindowRect, IsWindow, WM_SIZING},
    },
};

pub(crate) struct WindowSizing {
    hwnd: HWND,
    context: Option<Box<slint::Weak<PlayerWindow>>>,
}

impl WindowSizing {
    pub(crate) fn install(window: &Window, app: slint::Weak<PlayerWindow>) -> Result<Self, String> {
        let handle = window.window_handle().map_err(|error| error.to_string())?;
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return Err("Window sizing requires a Windows surface".into());
        };
        let hwnd = HWND(handle.hwnd.get() as *mut _);
        let context = Box::new(app);
        let pointer = (&*context as *const slint::Weak<PlayerWindow>) as usize;
        // SAFETY: the UI thread owns this stable box until subclass removal.
        if !unsafe { SetWindowSubclass(hwnd, Some(sizing_proc), pointer, pointer) }.as_bool() {
            return Err("Could not install native window sizing".into());
        }
        Ok(Self {
            hwnd,
            context: Some(context),
        })
    }
}

impl Drop for WindowSizing {
    fn drop(&mut self) {
        if let Some(context) = self.context.take() {
            let pointer = (&*context as *const slint::Weak<PlayerWindow>) as usize;
            // SAFETY: keep refdata alive through removal from the live HWND.
            let removed =
                unsafe { RemoveWindowSubclass(self.hwnd, Some(sizing_proc), pointer) }.as_bool();
            if !removed && unsafe { IsWindow(Some(self.hwnd)) }.as_bool() {
                Box::leak(context);
            }
        }
    }
}

unsafe extern "system" fn sizing_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    refdata: usize,
) -> LRESULT {
    // SAFETY: forward each message through the native subclass chain first,
    // retaining Winit's minimum/increment handling for the drag rectangle.
    let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    if message != WM_SIZING || lparam.0 == 0 {
        return result;
    }
    // SAFETY: WindowSizing owns this weak reference until subclass removal.
    let weak = unsafe { &*(refdata as *const slint::Weak<PlayerWindow>) };
    let Some(app) = weak.upgrade() else {
        return result;
    };
    if !app.get_loaded()
        || app.get_music_mode()
        || app.get_unlock_window_aspect_ratio()
        || !app.get_edge_to_edge_video()
        || app.get_fullscreen()
    {
        return result;
    }
    let aspect = f64::from(app.get_video_display_aspect_ratio());
    if !aspect.is_finite() || aspect <= 0.0 {
        return result;
    }
    let mut outer = RECT::default();
    let mut client = RECT::default();
    // SAFETY: both rectangles are writable and hwnd is the callback's live window.
    if unsafe { GetWindowRect(hwnd, &mut outer) }.is_err()
        || unsafe { GetClientRect(hwnd, &mut client) }.is_err()
    {
        return result;
    }
    let mut monitor = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let handle = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    if !unsafe { GetMonitorInfoW(handle, &mut monitor) }.as_bool() {
        return result;
    }
    let client_size = size(client);
    let outer_size = size(outer);
    let frame_width = outer_size.width.saturating_sub(client_size.width);
    let frame_height = outer_size.height.saturating_sub(client_size.height);
    let scale = f64::from(unsafe { GetDpiForWindow(hwnd) }.max(96)) / 96.0;
    let margin = (24.0 * scale).round() as u32;
    let work = size(monitor.rcWork);
    let maximum = PixelSize {
        width: work
            .width
            .saturating_sub(frame_width)
            .saturating_sub(2 * margin),
        height: work
            .height
            .saturating_sub(frame_height)
            .saturating_sub(2 * margin),
    };
    // SAFETY: WM_SIZING supplies a writable RECT valid for this callback.
    let proposed = unsafe { &mut *(lparam.0 as *mut RECT) };
    let proposed_size = size(*proposed);
    let requested = PixelSize {
        width: proposed_size.width.saturating_sub(frame_width),
        height: proposed_size.height.saturating_sub(frame_height),
    };
    let Some(target) = policy::constrain_resize(client_size, requested, aspect, Some(maximum))
        .and_then(|target| {
            policy::bounded_size(
                target,
                aspect,
                PixelSize {
                    width: (285.0 * scale).round() as u32,
                    height: (120.0 * scale).round() as u32,
                },
                Some(maximum),
            )
        })
    else {
        return result;
    };
    let frame = PixelSize {
        width: target.width.saturating_add(frame_width),
        height: target.height.saturating_add(frame_height),
    };
    let x = if matches!(wparam.0, 1 | 4 | 7) {
        proposed.right.saturating_sub(frame.width as i32)
    } else {
        proposed.left
    };
    let y = if matches!(wparam.0, 3..=5) {
        proposed.bottom.saturating_sub(frame.height as i32)
    } else {
        proposed.top
    };
    let (x, y) = clamp_position_with_margin(
        x,
        y,
        frame,
        ScreenRect {
            x: monitor.rcWork.left,
            y: monitor.rcWork.top,
            size: work,
        },
        margin,
    );
    *proposed = RECT {
        left: x,
        top: y,
        right: x.saturating_add(frame.width as i32),
        bottom: y.saturating_add(frame.height as i32),
    };
    LRESULT(1)
}

fn size(rect: RECT) -> PixelSize {
    PixelSize {
        width: rect.right.saturating_sub(rect.left).max(0) as u32,
        height: rect.bottom.saturating_sub(rect.top).max(0) as u32,
    }
}
