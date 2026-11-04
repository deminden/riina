//! Initial placement remains separate from ongoing surface alignment.
use super::surfaces::align;
use crate::{NativeVideoHost, PlayerWindow};
use slint::{
    ComponentHandle,
    winit_030::{WinitWindowAccessor, winit},
};
use std::{cell::Cell, rc::Rc};
use windows::Win32::{
    Foundation::{HWND, RECT},
    UI::WindowsAndMessaging::{
        GetWindowRect, SWP_NOACTIVATE, SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos,
    },
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

pub(super) fn center_host_in_work_area(
    window: &winit::window::Window,
    owner_center: Option<(i32, i32)>,
) {
    // Startup media may enter fullscreen before the delayed placement runs.
    // Work-area clamping must never shrink a fullscreen or maximized surface.
    if window.fullscreen().is_some() || window.is_maximized() {
        return;
    }
    let Ok(raw_window) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(raw_window) = raw_window.as_raw() else {
        return;
    };
    let hwnd = HWND(raw_window.hwnd.get() as *mut _);
    let mut frame = RECT::default();
    // SAFETY: frame is a valid writable RECT and hwnd belongs to this live window.
    if unsafe { GetWindowRect(hwnd, &mut frame) }.is_err() {
        return;
    }
    // Fit oversized initial frames inside the taskbar-free area before
    // centering. A position-only center clamps an overheight window to (0, 0).
    let work_area = owner_center
        .and_then(|(x, y)| super::super::window_position::work_area_near_point(x, y))
        .or_else(|| super::super::window_position::primary_work_area(window));
    let Some(work_area) = work_area else {
        return;
    };
    let width = frame.right.saturating_sub(frame.left).max(0) as u32;
    let height = frame.bottom.saturating_sub(frame.top).max(0) as u32;
    let bounds = riina_core::window_resize::ScreenRect {
        x: work_area.x,
        y: work_area.y,
        size: riina_core::window_aspect::PixelSize {
            width: work_area.size.width,
            height: work_area.size.height,
        },
    };
    let margin = (24.0 * window.scale_factor()).round().max(0.0) as u32;
    let Some(placement) = riina_core::window_resize::centered_window_placement(
        riina_core::window_aspect::PixelSize { width, height },
        bounds,
        margin,
    ) else {
        return;
    };
    let mut flags = SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOZORDER;
    if placement.size.width == width && placement.size.height == height {
        flags |= SWP_NOSIZE;
    }
    // Position by the measured outer frame so DPI conversion cannot shift the
    // caption edge; shrink only when that frame exceeds the usable area.
    let _ = unsafe {
        SetWindowPos(
            hwnd,
            None,
            placement.x,
            placement.y,
            i32::try_from(placement.size.width).unwrap_or(i32::MAX),
            i32::try_from(placement.size.height).unwrap_or(i32::MAX),
            flags,
        )
    };
}

pub(super) fn schedule_startup_center(
    host: slint::Weak<NativeVideoHost>,
    app: slint::Weak<PlayerWindow>,
    pending: Rc<Cell<bool>>,
    generation: Rc<Cell<u64>>,
    owner_center: Option<(i32, i32)>,
) {
    if !pending.get() {
        return;
    }
    let next = generation.get().wrapping_add(1);
    generation.set(next);
    slint::Timer::single_shot(std::time::Duration::from_millis(250), move || {
        if generation.get() != next || !pending.get() {
            return;
        }
        if let (Some(host), Some(app)) = (host.upgrade(), app.upgrade()) {
            if app.get_initial_window_position_enabled() || app.get_fullscreen() {
                pending.set(false);
                return;
            }
            host.window()
                .with_winit_window(|window| center_host_in_work_area(window, owner_center));
            align(host.window(), &app);
            pending.set(false);
        }
    });
}
