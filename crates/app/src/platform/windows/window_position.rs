//! First-show placement for auxiliary Windows surfaces.

use slint::{
    ComponentHandle,
    winit_030::{WinitWindowAccessor, winit},
};
use std::time::Duration;
use windows::Win32::{
    Foundation::{HWND, POINT, RECT},
    Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, MonitorFromWindow,
    },
    UI::WindowsAndMessaging::{
        GetWindowRect, IsWindowVisible, SWP_NOACTIVATE, SWP_NOOWNERZORDER, SWP_NOSIZE,
        SWP_NOZORDER, SetWindowPos,
    },
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// Return the current monitor's taskbar-free bounds in physical screen pixels.
pub(crate) fn work_area(
    window: &winit::window::Window,
) -> Option<riina_core::window_resize::ScreenRect> {
    let hwnd = hwnd(window)?;
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    work_area_for_monitor(monitor)
}

/// First-show placement must not infer the destination display from Winit's
/// temporary cascade position. On a multi-monitor desktop that default point
/// can belong to a secondary display even when the primary is elsewhere.
pub(crate) fn primary_work_area(
    window: &winit::window::Window,
) -> Option<riina_core::window_resize::ScreenRect> {
    let primary = window.primary_monitor()?;
    let origin = primary.position();
    let size = primary.size();
    if size.width == 0 || size.height == 0 {
        return None;
    }
    let center = POINT {
        x: origin
            .x
            .saturating_add(i32::try_from(size.width / 2).unwrap_or(i32::MAX)),
        y: origin
            .y
            .saturating_add(i32::try_from(size.height / 2).unwrap_or(i32::MAX)),
    };
    // SAFETY: the point is inside the live primary monitor returned by Winit.
    let monitor = unsafe { MonitorFromPoint(center, MONITOR_DEFAULTTONEAREST) };
    work_area_for_monitor(monitor)
}

/// Return the work area containing a point in screen coordinates.
pub(crate) fn work_area_near_point(
    x: i32,
    y: i32,
) -> Option<riina_core::window_resize::ScreenRect> {
    // SAFETY: coordinates came from a live top-level window in this desktop session.
    let monitor = unsafe { MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST) };
    work_area_for_monitor(monitor)
}

fn work_area_for_monitor(
    monitor: windows::Win32::Graphics::Gdi::HMONITOR,
) -> Option<riina_core::window_resize::ScreenRect> {
    let mut monitor_info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: monitor belongs to the live Winit window and monitor_info has the
    // required structure size.
    if !unsafe { GetMonitorInfoW(monitor, &mut monitor_info) }.as_bool() {
        return None;
    }
    screen_rect_from_work_area(monitor_info.rcWork)
}

pub(crate) fn content_work_area(
    window: &winit::window::Window,
) -> Option<riina_core::window_aspect::PixelSize> {
    let work_area = work_area(window)?.size;
    let outer = window.outer_size();
    let inner = window.inner_size();
    // An auto-sized video that uses every available pixel looks pinned to the
    // display edge even when its frame is mathematically centered. Keep a
    // modest, DPI-aware margin so the taskbar-free desktop remains visible.
    let margin = (24.0 * window.scale_factor()).round().max(0.0) as u32;
    Some(content_size_for_frame(
        work_area,
        riina_core::window_aspect::PixelSize {
            width: outer.width,
            height: outer.height,
        },
        riina_core::window_aspect::PixelSize {
            width: inner.width,
            height: inner.height,
        },
        margin,
    ))
}

fn content_size_for_frame(
    work_area: riina_core::window_aspect::PixelSize,
    outer: riina_core::window_aspect::PixelSize,
    inner: riina_core::window_aspect::PixelSize,
    margin: u32,
) -> riina_core::window_aspect::PixelSize {
    riina_core::window_aspect::PixelSize {
        width: work_area
            .width
            .saturating_sub(margin.saturating_mul(2))
            .saturating_sub(outer.width.saturating_sub(inner.width)),
        height: work_area
            .height
            .saturating_sub(margin.saturating_mul(2))
            .saturating_sub(outer.height.saturating_sub(inner.height)),
    }
}

fn screen_rect_from_work_area(work: RECT) -> Option<riina_core::window_resize::ScreenRect> {
    let width = work.right.saturating_sub(work.left).max(0) as u32;
    let height = work.bottom.saturating_sub(work.top).max(0) as u32;
    if width == 0 || height == 0 {
        return None;
    }
    Some(riina_core::window_resize::ScreenRect {
        x: work.left,
        y: work.top,
        size: riina_core::window_aspect::PixelSize { width, height },
    })
}

fn preferred_work_area(
    owner: Option<riina_core::window_resize::ScreenRect>,
    primary: Option<riina_core::window_resize::ScreenRect>,
) -> Option<riina_core::window_resize::ScreenRect> {
    // A cursor can be left on a secondary display or a remote-desktop edge.
    // Only an explicit owner should override the deterministic primary display.
    owner.or(primary)
}

/// Center a first-show child over its owner while keeping its frame on the
/// owner's work area. Retry until Slint has a realized HWND and Windows accepts
/// the placement; the component adapter can exist before either is ready.
pub(crate) fn center_auxiliary_window<W, O>(window: &W, owner: &O)
where
    W: ComponentHandle + 'static,
    O: ComponentHandle + 'static,
{
    let child = window.as_weak();
    let owner = owner.as_weak();
    if !try_center_auxiliary(&child, &owner) {
        schedule_auxiliary_center(child, owner, 24);
    }
}

/// Center a newly shown top-level player window in the work area of its display.
pub(crate) fn center_window<W>(window: &W)
where
    W: ComponentHandle + 'static,
{
    let child = window.as_weak();
    if !try_center_player(&child) {
        schedule_player_center(child, 24);
    }
}

fn try_center_auxiliary<W, O>(child: &slint::Weak<W>, owner: &slint::Weak<O>) -> bool
where
    W: ComponentHandle + 'static,
    O: ComponentHandle + 'static,
{
    let (Some(child_component), Some(owner_component)) = (child.upgrade(), owner.upgrade()) else {
        return false;
    };
    let owner_hwnd = owner_component.window().with_winit_window(hwnd).flatten();
    child_component
        .window()
        .with_winit_window(|child| place_child(child, owner_hwnd))
        .unwrap_or(false)
}

fn try_center_player<W>(child: &slint::Weak<W>) -> bool
where
    W: ComponentHandle + 'static,
{
    let Some(component) = child.upgrade() else {
        return false;
    };
    component
        .window()
        .with_winit_window(|child| place_child(child, None))
        .unwrap_or(false)
}

fn schedule_auxiliary_center<W, O>(child: slint::Weak<W>, owner: slint::Weak<O>, attempts: u8)
where
    W: ComponentHandle + 'static,
    O: ComponentHandle + 'static,
{
    slint::Timer::single_shot(Duration::from_millis(80), move || {
        if !try_center_auxiliary(&child, &owner) && attempts > 1 {
            schedule_auxiliary_center(child, owner, attempts - 1);
        }
    });
}

fn schedule_player_center<W>(child: slint::Weak<W>, attempts: u8)
where
    W: ComponentHandle + 'static,
{
    slint::Timer::single_shot(Duration::from_millis(80), move || {
        if !try_center_player(&child) && attempts > 1 {
            schedule_player_center(child, attempts - 1);
        }
    });
}

fn place_child(child: &winit::window::Window, owner: Option<HWND>) -> bool {
    let Some(child_hwnd) = hwnd(child) else {
        return false;
    };
    let mut child_rect = RECT::default();
    // SAFETY: child_hwnd comes from a live Winit window and child_rect is writable.
    if unsafe { GetWindowRect(child_hwnd, &mut child_rect) }.is_err() {
        return false;
    }
    let owner_rect = owner.and_then(|owner| {
        // Welcome windows are centered before the owner is ever shown; native
        // playback also hides its owner while the welcome surface is visible.
        // A hidden HWND can report its default (often upper-left) rectangle,
        // which is not a useful placement anchor.
        if !unsafe { IsWindowVisible(owner) }.as_bool() {
            return None;
        }
        let mut rect = RECT::default();
        // SAFETY: owner is a live window handle obtained from its Winit adapter.
        unsafe { GetWindowRect(owner, &mut rect) }
            .ok()
            .filter(|()| rect.right > rect.left && rect.bottom > rect.top)
            .map(|()| rect)
    });
    let owner_work_area = if owner_rect.is_some() {
        let owner = owner.expect("a visible owner rectangle has a live owner HWND");
        let monitor = unsafe { MonitorFromWindow(owner, MONITOR_DEFAULTTONEAREST) };
        work_area_for_monitor(monitor)
    } else {
        None
    };
    let bounds = preferred_work_area(owner_work_area, primary_work_area(child));
    let Some(bounds) = bounds else { return false };
    let width = child_rect.right.saturating_sub(child_rect.left).max(0) as u32;
    let height = child_rect.bottom.saturating_sub(child_rect.top).max(0) as u32;
    let owner_is_visible = owner_rect.is_some();
    let owner_bounds = owner_rect.map_or(bounds, |owner| riina_core::window_resize::ScreenRect {
        x: owner.left,
        y: owner.top,
        size: riina_core::window_aspect::PixelSize {
            width: owner.right.saturating_sub(owner.left).max(0) as u32,
            height: owner.bottom.saturating_sub(owner.top).max(0) as u32,
        },
    });
    let old_size = riina_core::window_aspect::PixelSize { width, height };
    let margin = (24.0 * child.scale_factor()).round().max(0.0) as u32;
    let Some(placement) =
        riina_core::window_resize::centered_window_placement(old_size, bounds, margin)
    else {
        return false;
    };
    let (x, y) = if owner_is_visible {
        riina_core::window_resize::centered_window_position(owner_bounds, placement.size, bounds)
    } else {
        (placement.x, placement.y)
    };
    let mut flags = SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOZORDER;
    if placement.size == old_size {
        flags |= SWP_NOSIZE;
    }
    // SAFETY: this positions and, only when needed, shrinks the live child,
    // preserving activation, z-order, and owner relationship.
    unsafe {
        SetWindowPos(
            child_hwnd,
            None,
            x,
            y,
            i32::try_from(placement.size.width).unwrap_or(i32::MAX),
            i32::try_from(placement.size.height).unwrap_or(i32::MAX),
            flags,
        )
    }
    .is_ok()
}

fn hwnd(window: &winit::window::Window) -> Option<HWND> {
    let handle = window.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => Some(HWND(handle.hwnd.get() as *mut _)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{content_size_for_frame, preferred_work_area, screen_rect_from_work_area};
    use riina_core::window_aspect::PixelSize;
    use windows::Win32::Foundation::RECT;

    #[test]
    fn work_area_keeps_negative_origin_and_taskbar_inset() {
        let bounds = screen_rect_from_work_area(RECT {
            left: -1920,
            top: 40,
            right: 0,
            bottom: 1040,
        })
        .unwrap();
        assert_eq!(bounds.x, -1920);
        assert_eq!(bounds.y, 40);
        assert_eq!(bounds.size.width, 1920);
        assert_eq!(bounds.size.height, 1000);
    }

    #[test]
    fn empty_or_inverted_work_area_is_rejected() {
        assert!(screen_rect_from_work_area(RECT::default()).is_none());
        assert!(
            screen_rect_from_work_area(RECT {
                left: 20,
                top: 0,
                right: 10,
                bottom: 10,
            })
            .is_none()
        );
    }

    #[test]
    fn content_limit_reserves_the_measured_native_frame() {
        assert_eq!(
            content_size_for_frame(
                PixelSize {
                    width: 1920,
                    height: 1040,
                },
                PixelSize {
                    width: 1936,
                    height: 1079,
                },
                PixelSize {
                    width: 1920,
                    height: 1040,
                },
                24,
            ),
            PixelSize {
                width: 1856,
                height: 953,
            }
        );
    }

    #[test]
    fn auto_size_margin_scales_with_high_dpi_work_area() {
        assert_eq!(
            content_size_for_frame(
                PixelSize {
                    width: 2560,
                    height: 1344,
                },
                PixelSize {
                    width: 2576,
                    height: 1383,
                },
                PixelSize {
                    width: 2560,
                    height: 1344,
                },
                48,
            ),
            PixelSize {
                width: 2448,
                height: 1209,
            }
        );
    }

    #[test]
    fn startup_prefers_owner_display_and_otherwise_uses_primary() {
        let area = |x| riina_core::window_resize::ScreenRect {
            x,
            y: 0,
            size: PixelSize {
                width: 1920,
                height: 1080,
            },
        };
        assert_eq!(
            preferred_work_area(Some(area(-1920)), Some(area(1920))),
            Some(area(-1920))
        );
        assert_eq!(
            preferred_work_area(None, Some(area(1920))),
            Some(area(1920))
        );
    }
}
