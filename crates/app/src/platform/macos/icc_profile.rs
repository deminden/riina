use objc2::rc::Retained;
use objc2_app_kit::{NSColorSpace, NSScreen, NSView};
use objc2_core_foundation::{CFData, CFRetained};
use objc2_core_graphics::CGColorSpace;
use slint::winit_030::WinitWindowAccessor;
use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

fn active_screen(window: &slint::Window) -> Option<Retained<NSScreen>> {
    window
        .with_winit_window(|window| {
            let handle = window.window_handle().ok()?;
            let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
                return None;
            };
            // SAFETY: the borrowed AppKit handle points to the live Winit window's NSView.
            let view = unsafe { &*handle.ns_view.as_ptr().cast::<NSView>() };
            view.window()?.screen()
        })
        .flatten()
}

pub(crate) fn display_identity(window: &slint::Window) -> Option<usize> {
    active_screen(window).map(|screen| Retained::as_ptr(&screen) as usize)
}

pub(crate) fn icc_profile_for_window(window: &slint::Window) -> Option<Vec<u8>> {
    let screen = active_screen(window)?;
    let color_space: Retained<NSColorSpace> = screen.colorSpace()?;
    let color_space: Retained<CGColorSpace> = color_space.CGColorSpace()?;
    let data: CFRetained<CFData> = CGColorSpace::icc_data(Some(&color_space))?;
    let len = usize::try_from(data.length()).ok()?;
    if len == 0 || data.byte_ptr().is_null() {
        return None;
    }
    // SAFETY: CFData owns at least `len` readable bytes for the retained data lifetime.
    Some(unsafe { std::slice::from_raw_parts(data.byte_ptr(), len) }.to_vec())
}
