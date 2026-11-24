use crate::PlayerWindow;
use riina_ui::PlayerMenuActions;
use slint::{
    ComponentHandle, Weak,
    winit_030::{
        WinitWindowAccessor,
        winit::raw_window_handle::{HasWindowHandle, RawWindowHandle},
    },
};
use std::time::Duration;
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{WM_EXITMENULOOP, WM_INITMENU, WM_NCDESTROY},
    },
};

pub(super) fn track_shared_player(app: &PlayerWindow) {
    let owner = app.as_weak();
    let _ = slint::spawn_local(async move {
        let Some(app) = owner.upgrade() else { return };
        let Ok(window) = app.window().winit_window().await else {
            return;
        };
        let Ok(handle) = window.window_handle() else {
            return;
        };
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return;
        };
        let hwnd = HWND(handle.hwnd.get() as *mut _);
        let owner = Box::into_raw(Box::new(app.as_weak()));
        // SAFETY: the Winit handle belongs to this UI thread. The subclass owns
        // the boxed weak handle until the native window finishes destruction.
        if !unsafe { SetWindowSubclass(hwnd, Some(menu_proc), 0, owner as usize) }.as_bool() {
            // SAFETY: registration failed and did not take the allocation.
            drop(unsafe { Box::from_raw(owner) });
            crate::diagnostics::warn!("Could not track the shared Windows Edit menu");
        }
    });
}

unsafe extern "system" fn menu_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    owner: usize,
) -> LRESULT {
    let owner = owner as *mut Weak<PlayerWindow>;
    match message {
        WM_INITMENU => {
            // SAFETY: this callback owns the allocation until WM_NCDESTROY.
            let owner = unsafe { &*owner }.clone();
            if let Some(app) = owner.upgrade() {
                app.global::<PlayerMenuActions>()
                    .set_edit_menu_state(super::menu_state(&app));
            }
        }
        WM_EXITMENULOOP => {
            // SAFETY: the clone can outlive this native window and its subclass.
            let owner = unsafe { &*owner }.clone();
            slint::Timer::single_shot(Duration::ZERO, move || {
                if let Some(app) = owner.upgrade() {
                    app.global::<PlayerMenuActions>()
                        .set_edit_menu_state(Default::default());
                }
            });
        }
        WM_NCDESTROY => {
            // SAFETY: native destruction is the final message for this owner.
            // Remove this callback before freeing its allocation.
            unsafe {
                let _ = RemoveWindowSubclass(hwnd, Some(menu_proc), id);
                drop(Box::from_raw(owner));
            }
        }
        _ => {}
    }
    // SAFETY: forward this window's unmodified message to its subclass chain.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}
