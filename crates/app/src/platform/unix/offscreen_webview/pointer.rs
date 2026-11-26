//! Deliver player-local input to the WebKit document's offscreen widget.
use gtk::{
    gdk::{self, ffi},
    glib::{
        self,
        translate::{ToGlibPtr, ToGlibPtrMut},
    },
    prelude::*,
};

pub(super) fn dispatch(
    view: &webkit2gtk::WebView,
    kind: gdk::EventType,
    (x, y): (f64, f64),
    button: u32,
    held: u32,
    modifiers: u32,
) {
    let Some(target) = view.window() else {
        return;
    };
    let mut event = gdk::Event::new(kind);
    let raw: *mut ffi::GdkEvent = event.to_glib_none_mut().0;
    let window: *mut ffi::GdkWindow = target.to_glib_none().0;
    let time = (glib::monotonic_time() / 1000) as u32;
    let state = button_state(held) | modifiers;
    // SAFETY: gdk allocated the union variant matching kind. The event owns one
    // window reference and releases it when dropped. Remaining fields are initialized by gdk.
    unsafe {
        match kind {
            gdk::EventType::ButtonPress | gdk::EventType::ButtonRelease => {
                let button_event = &mut (*raw).button;
                button_event.window = glib::gobject_ffi::g_object_ref(window.cast()).cast();
                button_event.send_event = 1;
                button_event.time = time;
                button_event.button = button;
                button_event.x = x;
                button_event.y = y;
                button_event.x_root = x;
                button_event.y_root = y;
                button_event.state = state;
            }
            gdk::EventType::MotionNotify => {
                let motion_event = &mut (*raw).motion;
                motion_event.window = glib::gobject_ffi::g_object_ref(window.cast()).cast();
                motion_event.send_event = 1;
                motion_event.time = time;
                motion_event.x = x;
                motion_event.y = y;
                motion_event.x_root = x;
                motion_event.y_root = y;
                motion_event.state = state;
            }
            _ => return,
        }
    }
    if let Some(device) = target
        .display()
        .default_seat()
        .and_then(|seat| seat.pointer())
    {
        event.set_device(Some(&device));
    }
    view.event(&event);
}

pub(super) fn scroll(
    view: &webkit2gtk::WebView,
    (x, y): (f64, f64),
    (dx, dy): (f64, f64),
    held: u32,
    modifiers: u32,
) {
    let Some(target) = view.window() else {
        return;
    };
    let mut event = gdk::Event::new(gdk::EventType::Scroll);
    let raw: *mut ffi::GdkEvent = event.to_glib_none_mut().0;
    let window: *mut ffi::GdkWindow = target.to_glib_none().0;
    // SAFETY: the scroll event owns one reference to its target window.
    unsafe {
        let scroll = &mut (*raw).scroll;
        scroll.window = glib::gobject_ffi::g_object_ref(window.cast()).cast();
        scroll.send_event = 1;
        scroll.time = (glib::monotonic_time() / 1000) as u32;
        scroll.direction = ffi::GDK_SCROLL_SMOOTH;
        scroll.x = x;
        scroll.y = y;
        scroll.x_root = x;
        scroll.y_root = y;
        scroll.state = button_state(held) | modifiers;
        // Slint supplies movement in logical pixels. GDK uses signed wheel steps.
        scroll.delta_x = -dx / 40.0;
        scroll.delta_y = -dy / 40.0;
    }
    if let Some(device) = target
        .display()
        .default_seat()
        .and_then(|seat| seat.pointer())
    {
        event.set_device(Some(&device));
    }
    view.event(&event);
}

fn button_state(held: u32) -> u32 {
    [
        (1, ffi::GDK_BUTTON1_MASK),
        (2, ffi::GDK_BUTTON2_MASK),
        (4, ffi::GDK_BUTTON3_MASK),
    ]
    .into_iter()
    .filter(|(button, _)| held & button != 0)
    .fold(0, |state, (_, mask)| state | mask)
}
