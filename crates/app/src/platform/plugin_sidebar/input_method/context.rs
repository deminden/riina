//! GTK class hooks for the browser's public input method interface.
use super::InputMethod;
use gtk::glib::{self, subclass::prelude::*, translate::FromGlibPtrBorrow};
use std::ffi::c_char;
use webkit2gtk::ffi;

glib::wrapper! {
    pub struct WebKitContext(Object<ffi::WebKitInputMethodContext, ffi::WebKitInputMethodContextClass>);

    match fn {
        type_ => || ffi::webkit_input_method_context_get_type(),
    }
}

// SAFETY: WebKitInputMethodContext is a derivable GObject. The local binding
// exposes subclass support for the same public type as webkit2gtk::InputMethodContext.
unsafe impl<T: ObjectImpl> IsSubclassable<T> for WebKitContext {}

glib::wrapper! {
    pub struct Context(ObjectSubclass<InputMethod>) @extends WebKitContext;
}

#[glib::object_subclass]
impl ObjectSubclass for InputMethod {
    const NAME: &'static str = "RiinaSidebarInputMethod";
    type Type = Context;
    type ParentType = WebKitContext;

    fn class_init(class: &mut Self::Class) {
        let class = std::ptr::from_mut(class).cast::<ffi::WebKitInputMethodContextClass>();
        // SAFETY: the subclass's repr(C) class starts with its WebKit parent class.
        let class = unsafe { &mut *class };
        class.get_preedit = Some(get_preedit);
        class.notify_focus_in = Some(focus_in);
        class.notify_focus_out = Some(focus_out);
        class.notify_cursor_area = Some(cursor_area);
        class.reset = Some(reset);
    }
}

unsafe extern "C" fn get_preedit(
    object: *mut ffi::WebKitInputMethodContext,
    text: *mut *mut c_char,
    underlines: *mut *mut glib::ffi::GList,
    cursor: *mut u32,
) {
    // SAFETY: WebKit calls this vfunc on a live instance of this subclass.
    let context = unsafe { Context::from_glib_borrow(object.cast()) };
    let state = context.imp();
    let preedit = state.preedit.borrow();
    // SAFETY: WebKit provides writable out parameters or null. It owns the
    // returned GLib allocation and provides default underlining when the list is null.
    unsafe {
        if !text.is_null() {
            *text = glib::ffi::g_strndup(preedit.as_ptr().cast(), preedit.len());
        }
        if !underlines.is_null() {
            *underlines = std::ptr::null_mut();
        }
        if !cursor.is_null() {
            *cursor = state.offset.get();
        }
    }
}

unsafe extern "C" fn focus_in(object: *mut ffi::WebKitInputMethodContext) {
    // SAFETY: WebKit calls this vfunc on a live instance of this subclass.
    let context = unsafe { Context::from_glib_borrow(object.cast()) };
    context.imp().set_active(true);
}

unsafe extern "C" fn focus_out(object: *mut ffi::WebKitInputMethodContext) {
    // SAFETY: WebKit calls this vfunc on a live instance of this subclass.
    let context = unsafe { Context::from_glib_borrow(object.cast()) };
    context.imp().set_active(false);
}

unsafe extern "C" fn cursor_area(
    object: *mut ffi::WebKitInputMethodContext,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) {
    // SAFETY: WebKit calls this vfunc on a live instance of this subclass.
    let context = unsafe { Context::from_glib_borrow(object.cast()) };
    context.imp().set_cursor_area((x, y, width, height));
}

unsafe extern "C" fn reset(object: *mut ffi::WebKitInputMethodContext) {
    // SAFETY: WebKit calls this vfunc on a live instance of this subclass.
    let context = unsafe { Context::from_glib_borrow(object.cast()) };
    context.imp().clear();
}
