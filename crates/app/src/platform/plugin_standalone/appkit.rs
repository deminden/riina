//! Bridge the standalone WebView to its AppKit window and screen coordinates.
use crate::platform::macos::view::content_view;
use objc2_app_kit::{NSWindowStyleMask, NSWindowTitleVisibility};
use riina_core::plugin_window::{Frame, Properties};
use slint::winit_030::winit::window::Window;
use wry::{Rect, dpi::LogicalPosition};

pub(super) fn content_bounds(window: &Window) -> Rect {
    Rect {
        position: LogicalPosition::new(0.0, 0.0).into(),
        size: window
            .inner_size()
            .to_logical::<f64>(window.scale_factor())
            .into(),
    }
}

pub(super) fn set_properties(window: &Window, properties: &Properties) {
    let Some(window) = content_view(window).and_then(|view| view.window()) else {
        return;
    };
    let mut mask = window.styleMask();
    mask.set(
        NSWindowStyleMask::FullSizeContentView,
        properties.full_size_content_view.unwrap_or(false),
    );
    window.setStyleMask(mask);
    let hidden = properties.hide_title_bar.unwrap_or(false);
    window.setTitlebarAppearsTransparent(hidden);
    window.setTitleVisibility(if hidden {
        NSWindowTitleVisibility::Hidden
    } else {
        NSWindowTitleVisibility::Visible
    });
    window.setMovableByWindowBackground(hidden);
}

pub(super) fn set_frame(window: &Window, edit: Frame) {
    let Some(window) = content_view(window).and_then(|view| view.window()) else {
        return;
    };
    // AppKit owns the frame in screen points. Leave omitted fields at the
    // current native values, including coordinates on secondary displays.
    let mut frame = window.frame();
    frame.origin.x = edit.x.unwrap_or(frame.origin.x);
    frame.origin.y = edit.y.unwrap_or(frame.origin.y);
    frame.size.width = edit.width.unwrap_or(frame.size.width);
    frame.size.height = edit.height.unwrap_or(frame.size.height);
    window.setFrame_display(frame, true);
}
