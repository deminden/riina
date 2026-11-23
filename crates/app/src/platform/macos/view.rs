use objc2::MainThreadMarker;
use objc2_app_kit::NSView;
use objc2_foundation::NSPoint;
use slint::winit_030::winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};

pub(in crate::platform) fn content_view(window: &Window) -> Option<&NSView> {
    MainThreadMarker::new()?;
    let RawWindowHandle::AppKit(handle) = window.window_handle().ok()?.as_raw() else {
        return None;
    };
    // SAFETY: the UI-thread window handle lends a live AppKit view for the window's lifetime.
    Some(unsafe { &*handle.ns_view.as_ptr().cast::<NSView>() })
}

pub(in crate::platform) fn top_left_point(view: &NSView, window_point: NSPoint) -> (f64, f64) {
    let local = view.convertPoint_fromView(window_point, None);
    let bounds = view.bounds();
    let x = local.x - bounds.origin.x;
    let y = local.y - bounds.origin.y;
    let y = if view.isFlipped() {
        y
    } else {
        bounds.size.height - y
    };
    (x, y)
}
