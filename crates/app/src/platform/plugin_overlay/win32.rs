//! Route pointer input through the WebView2 child only for sampled DOM hits.
use super::hit_test::{HitTest, Point};
use slint::winit_030::winit::window::Window;
use std::sync::Arc;
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    Graphics::Gdi::ScreenToClient,
    UI::{
        HiDpi::GetDpiForWindow,
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{GetClientRect, GetCursorPos, HTTRANSPARENT, IsWindow, WM_NCHITTEST},
    },
};
use wry::{Rect, WebView, WebViewBuilder, WebViewExtWindows};

pub(super) struct Surface {
    // Remove the subclass before WebView2 releases the HWND.
    input: Input,
    browser: WebView,
}

impl Surface {
    pub(super) fn build(
        builder: WebViewBuilder<'_>,
        parent: &Window,
        _: Rect,
        hits: Arc<HitTest>,
    ) -> Result<Self, String> {
        let browser = builder
            .build_as_child(parent)
            .map_err(|error| error.to_string())?;
        let input = Input::install(browser.hwnd(), hits)?;
        Ok(Self { input, browser })
    }

    pub(super) fn browser(&self) -> &WebView {
        &self.browser
    }

    pub(super) fn set_bounds(&self, bounds: Rect) -> Result<(), String> {
        self.browser
            .set_bounds(bounds)
            .map_err(|error| error.to_string())
    }

    pub(super) fn pointer(&self) -> Option<Point> {
        let mut point = POINT::default();
        // SAFETY: GetCursorPos writes a POINT into a live stack allocation.
        unsafe { GetCursorPos(&mut point) }.ok()?;
        client_position(self.input.hwnd, point)
    }
}

struct Input {
    hwnd: HWND,
    hits: Option<Arc<HitTest>>,
}

impl Input {
    fn install(hwnd: HWND, hits: Arc<HitTest>) -> Result<Self, String> {
        let reference = Arc::as_ptr(&hits) as usize;
        // SAFETY: the UI thread owns the live Arc until subclass removal.
        if !unsafe { SetWindowSubclass(hwnd, Some(hit_test_proc), reference, reference) }.as_bool()
        {
            return Err("Could not install plugin overlay hit testing".into());
        }
        Ok(Self {
            hwnd,
            hits: Some(hits),
        })
    }
}

impl Drop for Input {
    fn drop(&mut self) {
        if let Some(hits) = self.hits.take() {
            let reference = Arc::as_ptr(&hits) as usize;
            // SAFETY: refdata remains valid through removal on the HWND's UI thread.
            let removed =
                unsafe { RemoveWindowSubclass(self.hwnd, Some(hit_test_proc), reference) };
            // SAFETY: IsWindow takes a handle by value and tolerates an expired HWND.
            if !removed.as_bool() && unsafe { IsWindow(Some(self.hwnd)) }.as_bool() {
                std::mem::forget(hits);
            }
        }
    }
}

unsafe extern "system" fn hit_test_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    reference: usize,
) -> LRESULT {
    if message == WM_NCHITTEST {
        let position = POINT {
            x: i32::from(lparam.0 as i16),
            y: i32::from((lparam.0 >> 16) as i16),
        };
        // SAFETY: the installed subclass stores the address of the live Arc's allocation.
        let hits = unsafe { &*(reference as *const HitTest) };
        if !client_position(hwnd, position).is_some_and(|point| hits.contains(point)) {
            return LRESULT(HTTRANSPARENT as isize);
        }
    }
    // SAFETY: leave all other processing and claimed hits to the native subclass chain.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

fn client_position(hwnd: HWND, mut point: POINT) -> Option<Point> {
    let mut client = RECT::default();
    // SAFETY: the live HWND supplies coordinates to the two writable structures.
    if !unsafe {
        ScreenToClient(hwnd, &mut point).as_bool() && GetClientRect(hwnd, &mut client).is_ok()
    } || point.x < client.left
        || point.x >= client.right
        || point.y < client.top
        || point.y >= client.bottom
    {
        return None;
    }
    // SAFETY: GetDpiForWindow takes a handle by value and returns zero on failure.
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let scale = f64::from(if dpi == 0 { 96 } else { dpi }) / 96.0;
    Some((f64::from(point.x) / scale, f64::from(point.y) / scale))
}
