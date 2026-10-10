//! Restore shared input after owner menu selection or cancellation.
use crate::PlayerWindow;
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::time::Duration;
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{IsWindow, WM_EXITMENULOOP},
    },
};

pub(super) struct MenuFocus {
    hwnd: HWND,
    app: Option<Box<slint::Weak<PlayerWindow>>>,
}
impl MenuFocus {
    pub(super) fn install(handle: isize, app: slint::Weak<PlayerWindow>) -> Result<Self, String> {
        let hwnd = HWND(handle as *mut _);
        let app = Box::new(app);
        let pointer = (&*app as *const slint::Weak<PlayerWindow>) as usize;
        // SAFETY: the owning UI thread installs/removes this stable box.
        if !unsafe { SetWindowSubclass(hwnd, Some(menu_proc), pointer, pointer) }.as_bool() {
            return Err("Native menu focus handler could not be installed".into());
        }
        Ok(Self {
            hwnd,
            app: Some(app),
        })
    }
}
impl Drop for MenuFocus {
    fn drop(&mut self) {
        if let Some(app) = self.app.take() {
            let pointer = (&*app as *const slint::Weak<PlayerWindow>) as usize;
            // SAFETY: the callback context remains owned until removal.
            let removed =
                unsafe { RemoveWindowSubclass(self.hwnd, Some(menu_proc), pointer) }.as_bool();
            // SAFETY: this only queries whether Windows still owns the handle.
            if !removed && unsafe { IsWindow(Some(self.hwnd)) }.as_bool() {
                // Failed removal cannot leave Windows with freed refdata.
                Box::leak(app);
            }
        }
    }
}
unsafe extern "system" fn menu_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    refdata: usize,
) -> LRESULT {
    if message == WM_EXITMENULOOP {
        // SAFETY: MenuFocus retains the stable box until subclass removal.
        let app = unsafe { &*(refdata as *const slint::Weak<PlayerWindow>) }.clone();
        // Restore focus after Windows unwinds its menu callbacks, including
        // Escape paths with no activated item.
        slint::Timer::single_shot(Duration::ZERO, move || {
            if let Some(app) = app.upgrade() {
                app.window()
                    .with_winit_window(|window| window.focus_window());
            }
        });
    }
    // SAFETY: preserve the original subclass chain for every message.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}
