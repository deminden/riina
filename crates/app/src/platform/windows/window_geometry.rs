//! Size the native client, including menu wrapping, without a resize feedback loop.

use riina_core::{
    window_aspect::PixelSize,
    window_resize::{ScreenRect, clamp_position_with_margin},
};
use slint::winit_030::winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};
use std::hash::{Hash, Hasher};
use windows::Win32::{
    Foundation::{HWND, LPARAM, POINT, RECT},
    Graphics::Gdi::ClientToScreen,
    UI::WindowsAndMessaging::{
        BeginDeferWindowPos, DeferWindowPos, EndDeferWindowPos, EnumChildWindows, GetClientRect,
        GetParent, GetWindowRect, GetWindowTextW, IsWindowVisible, SWP_NOACTIVATE,
        SWP_NOOWNERZORDER, SWP_NOZORDER, SetWindowPos,
    },
};
use windows::core::BOOL;

pub(crate) fn resize_content(
    window: &Window,
    target: PixelSize,
    center_on_screen: bool,
    overlay: Option<&Window>,
) -> Result<(), String> {
    let handle = window.window_handle().map_err(|error| error.to_string())?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Err("Window geometry requires a Windows surface".into());
    };
    let hwnd = HWND(handle.hwnd.get() as *mut _);
    let overlay = overlay
        .map(|overlay| {
            let handle = overlay.window_handle().map_err(|error| error.to_string())?;
            let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                return Err("Overlay alignment requires a Windows surface".to_owned());
            };
            Ok(HWND(handle.hwnd.get() as *mut _))
        })
        .transpose()?;
    let area =
        super::window_position::work_area(window).ok_or("Could not read the window's work area")?;
    let (outer, _) = rectangles(hwnd)?;
    let center = if center_on_screen {
        (
            i64::from(area.x) * 2 + i64::from(area.size.width),
            i64::from(area.y) * 2 + i64::from(area.size.height),
        )
    } else {
        (
            i64::from(outer.left) + i64::from(outer.right),
            i64::from(outer.top) + i64::from(outer.bottom),
        )
    };
    let margin = (24.0 * window.scale_factor()).round().max(0.0) as u32;
    // AdjustWindowRectEx does not account for menus wrapping into more rows.
    // One native resize establishes the new row count; a second bounded pass
    // uses its measured frame. Both move and size atomically around one center.
    for _ in 0..2 {
        let (outer, client) = rectangles(hwnd)?;
        let frame = target_frame(size(outer), size(client), target, area, margin);
        let x = ((center.0 - i64::from(frame.width)) / 2)
            .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
        let y = ((center.1 - i64::from(frame.height)) / 2)
            .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
        let (x, y) = clamp_position_with_margin(x, y, frame, area, margin);
        if outer.left == x && outer.top == y && size(outer) == frame {
            break;
        }
        if let Some(overlay) = overlay {
            defer_resize_surfaces(hwnd, overlay, x, y, frame, target)?;
        } else {
            // SAFETY: the live UI-owned HWND is resized synchronously. Preserve
            // activation and z-order; WM_SIZE updates Winit's observed client size.
            unsafe {
                SetWindowPos(
                    hwnd,
                    None,
                    x,
                    y,
                    frame.width as i32,
                    frame.height as i32,
                    SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOZORDER,
                )
            }
            .map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn defer_resize_surfaces(
    host: HWND,
    overlay: HWND,
    host_x: i32,
    host_y: i32,
    host_frame: PixelSize,
    overlay_client: PixelSize,
) -> Result<(), String> {
    let mut client_origin = POINT { x: 0, y: 0 };
    // SAFETY: the live host client origin is a valid in/out POINT.
    if !unsafe { ClientToScreen(host, &mut client_origin) }.as_bool() {
        return Err("Could not locate the native video client origin".into());
    }
    let host_frame_width = host_frame.width as i32;
    let host_frame_height = host_frame.height as i32;
    let (overlay_outer, overlay_inner) = rectangles(overlay)?;
    let overlay_width = overlay_client.width.saturating_add(
        size(overlay_outer)
            .width
            .saturating_sub(size(overlay_inner).width),
    );
    let overlay_height = overlay_client.height.saturating_add(
        size(overlay_outer)
            .height
            .saturating_sub(size(overlay_inner).height),
    );
    let video_windows = visible_mpv_child_windows(host);
    // mpv owns a child HWND on its render thread. Include it with the parent and
    // Slint overlay so their bounds switch aspect ratio together.
    let count = 2_i32.saturating_add(i32::try_from(video_windows.len()).unwrap_or(i32::MAX));
    let batch = unsafe { BeginDeferWindowPos(count) }.map_err(|error| error.to_string())?;
    let batch = unsafe {
        DeferWindowPos(
            batch,
            host,
            None,
            host_x,
            host_y,
            host_frame_width,
            host_frame_height,
            SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOZORDER,
        )
    }
    .map_err(|error| error.to_string())?;
    let overlay_x = host_x.saturating_add(client_origin.x.saturating_sub(rectangles(host)?.0.left));
    let overlay_y = host_y.saturating_add(client_origin.y.saturating_sub(rectangles(host)?.0.top));
    let batch = unsafe {
        DeferWindowPos(
            batch,
            overlay,
            None,
            overlay_x,
            overlay_y,
            i32::try_from(overlay_width).unwrap_or(i32::MAX),
            i32::try_from(overlay_height).unwrap_or(i32::MAX),
            SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOZORDER,
        )
    }
    .map_err(|error| error.to_string())?;
    let mut batch = batch;
    for video in video_windows {
        // SAFETY: video is the mpv child of this host. Its window thread pumps
        // Win32 messages; apply matching bounds in the parent resize batch so
        // playlist changes cannot expose the previous media's render bounds.
        batch = unsafe {
            DeferWindowPos(
                batch,
                video,
                None,
                0,
                0,
                i32::try_from(overlay_client.width).unwrap_or(i32::MAX),
                i32::try_from(overlay_client.height).unwrap_or(i32::MAX),
                SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOZORDER,
            )
        }
        .map_err(|error| error.to_string())?
    }
    unsafe { EndDeferWindowPos(batch) }.map_err(|error| error.to_string())
}

struct MpvChildSearch {
    parent: HWND,
    matches: Vec<HWND>,
}

unsafe extern "system" fn find_mpv_child(hwnd: HWND, parameter: LPARAM) -> BOOL {
    // SAFETY: EnumChildWindows invokes this callback synchronously with the
    // live stack context supplied by mpv_child_window.
    let search = unsafe { &mut *(parameter.0 as *mut MpvChildSearch) };
    if unsafe { GetParent(hwnd) }.is_ok_and(|parent| parent == search.parent) {
        let mut title = [0_u16; 128];
        let length = unsafe { GetWindowTextW(hwnd, &mut title) }.max(0) as usize;
        let title = String::from_utf16_lossy(&title[..length.min(title.len())]);
        let title = title.to_ascii_lowercase();
        if title == "mpv" || title.ends_with(" - mpv") || title.ends_with(" — mpv") {
            search.matches.push(hwnd);
        }
    }
    BOOL(1)
}

fn mpv_child_windows(parent: HWND) -> Vec<HWND> {
    let mut search = MpvChildSearch {
        parent,
        matches: Vec::new(),
    };
    // SAFETY: the callback receives a valid stack context for the duration of
    // this synchronous enumeration and only selects direct mpv-owned children.
    unsafe {
        let _ = EnumChildWindows(
            Some(parent),
            Some(find_mpv_child),
            LPARAM((&mut search as *mut MpvChildSearch) as isize),
        );
    }
    search.matches
}

fn visible_mpv_child_windows(parent: HWND) -> Vec<HWND> {
    // mpv can retain a hidden child while rebuilding its swap chain. Do not
    // resize that stale HWND on every alignment tick; it is aligned when shown.
    mpv_child_windows(parent)
        .into_iter()
        .filter(|window| unsafe { IsWindowVisible(*window) }.as_bool())
        .collect()
}

/// Check the real embedded output surface before releasing a transition cover.
/// The owner can keep a steady client size while mpv is still resizing or
/// temporarily hiding its child window during a playlist item handoff.
pub(crate) fn native_video_surface_signature(window: &Window) -> Option<u64> {
    let Ok(handle) = window.window_handle() else {
        return None;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return None;
    };
    let host = HWND(handle.hwnd.get() as *mut _);
    let mut host_client = RECT::default();
    // SAFETY: the live Winit window owns this HWND and the output RECT is writable.
    if unsafe { GetClientRect(host, &mut host_client) }.is_err() {
        return None;
    }
    let expected = size(host_client);
    let mut surfaces = Vec::new();
    for video in visible_mpv_child_windows(host) {
        let Ok((_, client)) = rectangles(video) else {
            return None;
        };
        let client_size = size(client);
        // A stale visible child must not make a newly-created surface appear
        // ready just because both happen to share one matching dimension.
        if client_size != expected {
            return None;
        }
        let mut title = [0_u16; 128];
        // SAFETY: video is a live, visible direct mpv child and title is writable.
        let length = unsafe { GetWindowTextW(video, &mut title) }.max(0) as usize;
        surfaces.push((
            video.0 as usize,
            client_size.width,
            client_size.height,
            title[..length.min(title.len())].to_vec(),
        ));
    }
    if surfaces.is_empty() {
        return None;
    }
    surfaces.sort_by_key(|(hwnd, _, _, _)| *hwnd);
    let mut signature = std::collections::hash_map::DefaultHasher::new();
    expected.width.hash(&mut signature);
    expected.height.hash(&mut signature);
    surfaces.hash(&mut signature);
    Some(signature.finish())
}

/// Keep the transparent controls over the native video client in one window
/// manager operation. Separate Slint size and position calls expose an
/// intermediate overlay frame while playlist items change aspect ratio.
pub(crate) fn align_surfaces(
    host: &Window,
    overlay: &Window,
    position: slint::PhysicalPosition,
    client_size: slint::PhysicalSize,
) -> Result<(), String> {
    let host_handle = host.window_handle().map_err(|error| error.to_string())?;
    let RawWindowHandle::Win32(host_handle) = host_handle.as_raw() else {
        return Err("Video surface alignment requires a Windows host".into());
    };
    let host = HWND(host_handle.hwnd.get() as *mut _);
    let handle = overlay.window_handle().map_err(|error| error.to_string())?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Err("Overlay alignment requires a Windows surface".into());
    };
    let hwnd = HWND(handle.hwnd.get() as *mut _);
    let (outer, client) = rectangles(hwnd)?;
    let frame_width = size(outer).width.saturating_sub(size(client).width);
    let frame_height = size(outer).height.saturating_sub(size(client).height);
    let width = client_size.width.saturating_add(frame_width);
    let height = client_size.height.saturating_add(frame_height);
    let video_windows = visible_mpv_child_windows(host);
    let video_is_aligned = video_windows.iter().all(|video| {
        rectangles(*video).is_ok_and(|(_, client)| {
            size(client).width == client_size.width && size(client).height == client_size.height
        })
    });
    if outer.left == position.x
        && outer.top == position.y
        && size(outer).width == width
        && size(outer).height == height
        && video_is_aligned
    {
        return Ok(());
    }
    let batch = unsafe {
        BeginDeferWindowPos(
            1_i32.saturating_add(i32::try_from(video_windows.len()).unwrap_or(i32::MAX)),
        )
    }
    .map_err(|error| error.to_string())?;
    // SAFETY: the overlay is a live Slint-owned HWND and dimensions are
    // measured from the native client surface. Align it with every mpv child
    // in one window-manager operation after the host's WM_SIZE notification.
    let mut batch = unsafe {
        DeferWindowPos(
            batch,
            hwnd,
            None,
            position.x,
            position.y,
            i32::try_from(width).unwrap_or(i32::MAX),
            i32::try_from(height).unwrap_or(i32::MAX),
            SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOZORDER,
        )
    }
    .map_err(|error| error.to_string())?;
    for video in video_windows {
        batch = unsafe {
            DeferWindowPos(
                batch,
                video,
                None,
                0,
                0,
                i32::try_from(client_size.width).unwrap_or(i32::MAX),
                i32::try_from(client_size.height).unwrap_or(i32::MAX),
                SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOZORDER,
            )
        }
        .map_err(|error| error.to_string())?;
    }
    unsafe { EndDeferWindowPos(batch) }.map_err(|error| error.to_string())
}

fn rectangles(hwnd: HWND) -> Result<(RECT, RECT), String> {
    let mut outer = RECT::default();
    let mut client = RECT::default();
    // SAFETY: both output rectangles are writable and hwnd belongs to this window.
    unsafe {
        GetWindowRect(hwnd, &mut outer).map_err(|error| error.to_string())?;
        GetClientRect(hwnd, &mut client).map_err(|error| error.to_string())?;
    }
    Ok((outer, client))
}

fn size(rect: RECT) -> PixelSize {
    PixelSize {
        width: rect.right.saturating_sub(rect.left).max(0) as u32,
        height: rect.bottom.saturating_sub(rect.top).max(0) as u32,
    }
}

fn target_frame(
    outer: PixelSize,
    client: PixelSize,
    target: PixelSize,
    area: ScreenRect,
    margin: u32,
) -> PixelSize {
    PixelSize {
        width: target
            .width
            .saturating_add(outer.width.saturating_sub(client.width))
            .min(area.size.width.saturating_sub(margin.saturating_mul(2))),
        height: target
            .height
            .saturating_add(outer.height.saturating_sub(client.height))
            .min(area.size.height.saturating_sub(margin.saturating_mul(2))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measured_wrapped_menu_height_is_reserved_without_growing_the_client() {
        let outer = PixelSize {
            width: 891,
            height: 1695,
        };
        let client = PixelSize {
            width: 855,
            height: 1477,
        };
        let target = PixelSize {
            width: 855,
            height: 1520,
        };
        let area = ScreenRect {
            x: 0,
            y: 0,
            size: PixelSize {
                width: 3840,
                height: 2016,
            },
        };
        assert_eq!(
            target_frame(outer, client, target, area, 72),
            PixelSize {
                width: 891,
                height: 1738
            }
        );
    }

    #[test]
    fn native_frame_cannot_extend_past_the_work_area_inset() {
        let area = ScreenRect {
            x: -3840,
            y: 0,
            size: PixelSize {
                width: 3840,
                height: 2016,
            },
        };
        assert_eq!(
            target_frame(
                PixelSize {
                    width: 891,
                    height: 1738
                },
                PixelSize {
                    width: 855,
                    height: 1520
                },
                PixelSize {
                    width: 4000,
                    height: 2000
                },
                area,
                72,
            ),
            PixelSize {
                width: 3696,
                height: 1872
            }
        );
    }
}
