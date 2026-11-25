//! Non-activating, click-through surfaces over the other monitor rectangles.
use crate::platform::display_blackout::{Bounds, secondary};
use slint::winit_030::WinitWindowAccessor;
use std::{collections::BTreeMap, sync::OnceLock};
use windows::{
    Win32::{
        Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
        Graphics::Gdi::{BLACK_BRUSH, GetStockObject, HBRUSH},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, HWND_TOPMOST, LWA_ALPHA,
            RegisterClassW, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetLayeredWindowAttributes,
            SetWindowPos, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
            WS_EX_TRANSPARENT, WS_POPUP,
        },
    },
    core::w,
};

#[derive(Default)]
pub(in crate::platform) struct Covers(BTreeMap<Bounds, Cover>);

impl Covers {
    pub(in crate::platform) fn refresh(&mut self, owner: &slint::Window, controls: &slint::Window) {
        let focused = controls.with_winit_window(|window| window.has_focus()) == Some(true);
        let bounds = owner
            .with_winit_window(|window| {
                if (focused || window.has_focus()) && window.is_minimized() != Some(true) {
                    secondary(window)
                } else {
                    Default::default()
                }
            })
            .unwrap_or_default();
        self.0.retain(|region, _| bounds.contains(region));
        for region in bounds {
            if let std::collections::btree_map::Entry::Vacant(slot) = self.0.entry(region)
                && let Some(cover) = Cover::new(region)
            {
                slot.insert(cover);
            }
        }
    }

    pub(in crate::platform) fn clear(&mut self) {
        self.0.clear();
    }
}

struct Cover(HWND);

impl Cover {
    fn new(region: Bounds) -> Option<Self> {
        if !registered() {
            return None;
        }
        let width = i32::try_from(region.width).ok()?;
        let height = i32::try_from(region.height).ok()?;
        // SAFETY: The class and its system brush have process lifetime.
        // The cover belongs to this UI thread and stays out of activation and hit testing.
        unsafe {
            let cover = Self(
                CreateWindowExW(
                    WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                    w!("RiinaFullscreenBlackout"),
                    w!(""),
                    WS_POPUP,
                    region.x,
                    region.y,
                    width,
                    height,
                    None,
                    None,
                    Some(HINSTANCE(GetModuleHandleW(None).ok()?.0)),
                    None,
                )
                .ok()?,
            );
            SetLayeredWindowAttributes(cover.0, COLORREF(0), 255, LWA_ALPHA).ok()?;
            SetWindowPos(
                cover.0,
                Some(HWND_TOPMOST),
                region.x,
                region.y,
                width,
                height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
            .ok()?;
            Some(cover)
        }
    }
}

impl Drop for Cover {
    fn drop(&mut self) {
        // SAFETY: This is the owned cover on its creating UI thread.
        unsafe {
            let _ = DestroyWindow(self.0);
        }
    }
}

fn registered() -> bool {
    static REGISTERED: OnceLock<bool> = OnceLock::new();
    *REGISTERED.get_or_init(|| {
        // SAFETY: All registered data and the stock brush live for the process.
        unsafe {
            let Ok(module) = GetModuleHandleW(None) else {
                return false;
            };
            RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(cover_proc),
                hInstance: HINSTANCE(module.0),
                hbrBackground: HBRUSH(GetStockObject(BLACK_BRUSH).0),
                lpszClassName: w!("RiinaFullscreenBlackout"),
                ..Default::default()
            }) != 0
        }
    })
}

unsafe extern "system" fn cover_proc(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    // SAFETY: Win32 supplied this window and its message parameters to the class procedure.
    unsafe { DefWindowProcW(hwnd, message, w, l) }
}
