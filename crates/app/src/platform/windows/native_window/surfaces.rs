//! Composition and verified alignment of the two native surfaces.
#[cfg(test)]
mod tests;
use super::super::window_geometry;
use crate::PlayerWindow;
use slint::{
    ComponentHandle,
    winit_030::{WinitWindowAccessor, winit},
};
use std::cell::{Cell, RefCell};
use windows::Win32::{
    Foundation::{HWND, LPARAM, RECT},
    Graphics::{
        Dwm::{
            DWM_BB_BLURREGION, DWM_BB_ENABLE, DWM_BB_TRANSITIONONMAXIMIZED, DWM_BLURBEHIND,
            DwmEnableBlurBehindWindow,
        },
        Gdi::{CreateRectRgn, DeleteObject},
    },
    UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, GetWindowRect, SWP_FRAMECHANGED, SWP_NOACTIVATE,
        SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_NOZORDER, SendMessageW, SetWindowLongPtrW,
        SetWindowPos, WS_EX_LAYERED, WS_EX_TRANSPARENT,
    },
};
use windows::core::BOOL;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

thread_local! {
    // Surface alignment retries run every 16 ms. Report each new failure once
    // so a persistent native-window mismatch stays diagnosable without flooding logs.
    static LAST_OVERLAY_ALIGNMENT_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
    #[cfg(debug_assertions)]
    static ALIGNMENT_WINDOWS_REPORTED: Cell<bool> = const { Cell::new(false) };
}

// Slint can change the popup's native styles while creating its shared menu.
// Preserve Winit's layered composition surface, but restore interactive hit
// testing after the shared menu has made the popup click-through.
pub(super) fn enable_video_transparency(window: &winit::window::Window) -> Result<(), String> {
    let handle = window.window_handle().map_err(|error| error.to_string())?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Err("Native video transparency requires a Windows window".into());
    };
    let hwnd = HWND(handle.hwnd.get() as *mut _);
    // Winit may provide WS_EX_LAYERED for per-pixel alpha composition. Preserve
    // its style and remove only the click-through flag that blocks control input.
    let extended_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
    let interactive_style = extended_style & !(WS_EX_TRANSPARENT.0 as isize);
    if interactive_style != extended_style {
        // SAFETY: hwnd is the retained, realized Slint chrome window. The style
        // change leaves its owner and taskbar attributes intact.
        unsafe {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, interactive_style);
            SetWindowPos(
                hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED
                    | SWP_NOMOVE
                    | SWP_NOSIZE
                    | SWP_NOACTIVATE
                    | SWP_NOOWNERZORDER
                    | SWP_NOZORDER,
            )
            .map_err(|error| format!("Could not enable native video control input: {error}"))?;
        }
    }
    let effective_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
    if effective_style & (WS_EX_TRANSPARENT.0 as isize) != 0 {
        return Err("The native video overlay is still mouse-transparent".into());
    }
    if (effective_style ^ extended_style) & (WS_EX_LAYERED.0 as isize) != 0 {
        return Err("The native video overlay lost its layered composition style".into());
    }
    // An empty blur region enables pixel alpha without requesting an opaque
    // whole-window backdrop. A null region instead covers the entire client,
    // including the otherwise clear fullscreen area around the controller.
    let region = unsafe { CreateRectRgn(0, 0, 0, 0) };
    if region.0.is_null() {
        return Err("Could not create the controls' transparent composition region".into());
    }
    let blur_behind = DWM_BLURBEHIND {
        dwFlags: DWM_BB_ENABLE | DWM_BB_BLURREGION | DWM_BB_TRANSITIONONMAXIMIZED,
        fEnable: BOOL(1),
        hRgnBlur: region,
        fTransitionOnMaximized: BOOL(0),
    };
    // Windows 8+ no longer blurs with this legacy API, but Microsoft documents
    // that it still enables DWM to honor this window's per-pixel alpha values.
    // The actual Windows material comes from the sampled-video Acrylic layer.
    let result = unsafe { DwmEnableBlurBehindWindow(hwnd, &blur_behind) };
    // DWM copies the region; release our GDI object even if composition failed.
    let _ = unsafe { DeleteObject(region.into()) };
    result
        .map_err(|error| format!("Could not restore video-overlay alpha composition: {error}"))?;
    Ok(())
}

pub(super) fn dismiss_unaligned_transition_cover(app: &PlayerWindow) {
    if app.get_native_transition_frame() {
        // A retained frame is opaque by design. If the controls HWND cannot be
        // verified over its video host, discard it before it can remain painted
        // at the overlay's previous desktop coordinates.
        app.set_native_transition_frame(false);
        app.set_video_frame(slint::Image::default());
        app.window().request_redraw();
    }
}

pub(super) fn set_overlay_visibility(app: &PlayerWindow, visible: bool) {
    app.window().with_winit_window(|overlay| {
        overlay.set_visible(visible);
        if visible {
            // A move/fullscreen transition may hide the popup while its
            // previous redraw is queued. Re-arm the native paint event so
            // the restored surface cannot retain that frame indefinitely.
            overlay.request_redraw();
        }
    });
}

pub(super) fn align(host: &slint::Window, chrome: &PlayerWindow) -> bool {
    let mut aligned = false;
    host.with_winit_window(|window| {
        chrome.window().with_winit_window(|overlay| {
            #[cfg(debug_assertions)]
            ALIGNMENT_WINDOWS_REPORTED.with(|reported| {
                if !reported.replace(true) {
                    let hwnd = |window: &winit::window::Window| {
                        window.window_handle().ok().and_then(|handle| match handle.as_raw() {
                            RawWindowHandle::Win32(handle) => Some(HWND(handle.hwnd.get() as *mut _)),
                            _ => None,
                        })
                    };
                    let host_hwnd = hwnd(window);
                    let overlay_hwnd = hwnd(overlay);
                    let bounds = |hwnd: HWND| {
                        let mut rect = RECT::default();
                        // SAFETY: Each handle was read from its live Winit window.
                        unsafe { GetWindowRect(hwnd, &mut rect) }
                            .ok()
                            .map(|_| (rect.left, rect.top, rect.right, rect.bottom))
                    };
                    eprintln!(
                        "Native video alignment HWNDs: host={host_hwnd:?} bounds={:?}; controls={overlay_hwnd:?} bounds={:?}",
                        host_hwnd.and_then(bounds),
                        overlay_hwnd.and_then(bounds),
                    );
                }
            });
            let layout = if chrome.get_music_mode() {
                let bottom_inset = crate::music_mode_layout::COMPACT_CONTROL_HEIGHT
                    + if chrome.get_music_mode_show_playlist() {
                        crate::music_mode_layout::PLAYLIST_HEIGHT
                    } else {
                        0.0
                    };
                window_geometry::VideoLayout::Artwork {
                    bottom_inset: (bottom_inset * window.scale_factor()).round() as u32,
                    visible: chrome.get_music_mode_show_album_art(),
                }
            } else {
                window_geometry::VideoLayout::FullClient
            };
            if let Err(error) = window_geometry::align_surfaces(window, overlay, layout) {
                dismiss_unaligned_transition_cover(chrome);
                // Winit can still report pre-move coordinates while Windows is
                // reconciling an owned popup. Writing those stale coordinates
                // back here can detach the black-backed transparent surface
                // from playback. Keep the Win32 client bounds authoritative;
                // the next alignment tick retries after the move settles.
                let should_report = LAST_OVERLAY_ALIGNMENT_ERROR.with(|previous| {
                    let mut previous = previous.borrow_mut();
                    if previous.as_deref() == Some(error.as_str()) {
                        false
                    } else {
                        *previous = Some(error.clone());
                        true
                    }
                });
                if should_report {
                    eprintln!("Could not align native video controls atomically: {error}");
                }
            } else {
                aligned = true;
            }
        });
    });
    if !aligned {
        dismiss_unaligned_transition_cover(chrome);
    }
    if aligned {
        LAST_OVERLAY_ALIGNMENT_ERROR.with(|previous| *previous.borrow_mut() = None);
    }
    aligned
}

pub(super) fn sync_native_theme(
    host: &winit::window::Window,
    dark_theme: bool,
    last_native_theme: &Cell<Option<bool>>,
) {
    // Native-video playback uses separate HWNDs for the framed host and
    // transparent controls. Keep the Windows caption/menu and Mica tone in the
    // same appearance as the controls when the user's theme changes.
    if last_native_theme.get() == Some(dark_theme) {
        return;
    }
    let Ok(handle) = host.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return;
    };
    // Slint 1.18 does not expose WindowAdapter::set_color_scheme publicly, so
    // its Muda menubar misses app-driven theme changes. Muda 0.19.3's public
    // Menu::set_theme_for_hwnd sends this private update message; mirror that
    // call for the known live HWND and keep the crate version pinned above.
    const MUDA_MENU_UPDATE_THEME: u32 = 201;
    let menu_theme = if dark_theme {
        muda::MenuTheme::Dark
    } else {
        muda::MenuTheme::Light
    };
    let hwnd = HWND(handle.hwnd.get() as *mut _);
    // SAFETY: this HWND is the live Winit host whose Slint Muda menubar is
    // attached before this synchronization timer starts.
    unsafe {
        SendMessageW(
            hwnd,
            MUDA_MENU_UPDATE_THEME,
            None,
            Some(LPARAM(menu_theme as isize)),
        );
    }
    // Winit's `theme()` reports the cached system scheme, not the explicit
    // app-selected override. Apply only on transitions, after Muda's redraw
    // request so non-client activation cannot leave the caption in the old tone.
    host.set_theme(Some(if dark_theme {
        winit::window::Theme::Dark
    } else {
        winit::window::Theme::Light
    }));
    last_native_theme.set(Some(dark_theme));
}
