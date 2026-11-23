//! Keep the browser's editing and focus behavior in WebKit.
use gtk::{
    gdk::{self, ffi, keys::constants},
    glib::{
        self,
        translate::{ToGlibPtr, ToGlibPtrMut},
    },
    prelude::*,
};
use slint::platform::Key;
use std::ffi::CString;

pub(super) fn focus(view: &webkit2gtk::WebView, focused: bool) {
    let Some(target) = view.window() else {
        return;
    };
    if focused {
        view.grab_focus();
    }
    let mut event = gdk::Event::new(gdk::EventType::FocusChange);
    let raw: *mut ffi::GdkEvent = event.to_glib_none_mut().0;
    let window: *mut ffi::GdkWindow = target.to_glib_none().0;
    // SAFETY: this is a focus event. GDK releases the retained window with it.
    unsafe {
        let focus = &mut (*raw).focus_change;
        focus.window = glib::gobject_ffi::g_object_ref(window.cast()).cast();
        focus.send_event = 1;
        focus.in_ = i16::from(focused);
    }
    view.event(&event);
}

pub(super) fn dispatch(
    view: &webkit2gtk::WebView,
    text: &str,
    modifiers: ffi::GdkModifierType,
    released: bool,
) {
    let Some(target) = view.window() else {
        return;
    };
    let display = target.display();
    let keyboard = display.default_seat().and_then(|seat| seat.keyboard());
    let keymap = gdk::Keymap::for_display(&display);
    let window: *mut ffi::GdkWindow = target.to_glib_none().0;
    let kind = if released {
        gdk::EventType::KeyRelease
    } else {
        gdk::EventType::KeyPress
    };
    for character in text.chars() {
        let key = webkit_key(character);
        let bindings = keymap
            .as_ref()
            .map_or_else(Vec::new, |map| map.entries_for_keyval(*key));
        let text = key
            .to_unicode()
            .and_then(|key| CString::new(key.to_string()).ok());
        let mut event = gdk::Event::new(kind);
        let raw: *mut ffi::GdkEvent = event.to_glib_none_mut().0;
        // SAFETY: both event kinds use the key variant. The event owns the
        // retained window and the GLib-allocated key text.
        unsafe {
            let key_event = &mut (*raw).key;
            key_event.window = glib::gobject_ffi::g_object_ref(window.cast()).cast();
            key_event.send_event = 1;
            key_event.keyval = *key;
            key_event.state = modifiers;
            key_event.time = (glib::monotonic_time() / 1000) as u32;
            if let Some(text) = text.as_ref() {
                key_event.length = text.as_bytes().len() as i32;
                key_event.string = glib::ffi::g_strdup(text.as_ptr());
            }
            if let Some(binding) = bindings.first() {
                key_event.hardware_keycode = binding.keycode() as u16;
                key_event.group = binding.group() as u8;
            }
        }
        if let Some(keyboard) = keyboard.as_ref() {
            event.set_device(Some(keyboard));
        }
        view.event(&event);
    }
}

fn webkit_key(character: char) -> gdk::keys::Key {
    for (slint, gtk) in [
        (Key::Backspace, constants::BackSpace),
        (Key::Tab, constants::Tab),
        (Key::Backtab, constants::ISO_Left_Tab),
        (Key::Return, constants::Return),
        (Key::Escape, constants::Escape),
        (Key::LeftArrow, constants::Left),
        (Key::UpArrow, constants::Up),
        (Key::RightArrow, constants::Right),
        (Key::DownArrow, constants::Down),
        (Key::Insert, constants::Insert),
        (Key::Delete, constants::Delete),
        (Key::Home, constants::Home),
        (Key::End, constants::End),
        (Key::PageUp, constants::Page_Up),
        (Key::PageDown, constants::Page_Down),
        (Key::Control, constants::Control_L),
        (Key::ControlR, constants::Control_R),
        (Key::Alt, constants::Alt_L),
        (Key::AltGr, constants::ISO_Level3_Shift),
        (Key::Shift, constants::Shift_L),
        (Key::ShiftR, constants::Shift_R),
        (Key::Meta, constants::Super_L),
        (Key::MetaR, constants::Super_R),
    ] {
        if character == char::from(slint) {
            return gtk;
        }
    }
    let first_function = u32::from(char::from(Key::F1));
    let last_function = u32::from(char::from(Key::F24));
    let character_code = u32::from(character);
    if (first_function..=last_function).contains(&character_code) {
        gdk::keys::Key::from(*constants::F1 + character_code - first_function)
    } else {
        gdk::keys::Key::from_unicode(character)
    }
}
