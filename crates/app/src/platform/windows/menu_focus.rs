//! Keep native menu input on the owner until Windows finishes its menu loop.
use crate::PlayerWindow;
use slint::{
    ComponentHandle,
    winit_030::{
        WinitWindowAccessor,
        winit::raw_window_handle::{HasWindowHandle, RawWindowHandle},
    },
};
use std::{cell::Cell, rc::Rc};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        UI::{
            Input::KeyboardAndMouse::{SetFocus, VK_F10},
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::{
                IsWindow, PostMessageW, RegisterWindowMessageW, SC_KEYMENU, WM_ENTERMENULOOP,
                WM_EXITMENULOOP, WM_KEYDOWN, WM_KEYUP, WM_SYSCOMMAND, WM_SYSKEYDOWN, WM_SYSKEYUP,
            },
        },
    },
    core::w,
};

struct MenuContext {
    app: slint::Weak<PlayerWindow>,
    active: Rc<Cell<bool>>,
    generation: Cell<usize>,
    restore_message: u32,
}

pub(super) struct MenuFocus {
    hwnd: HWND,
    context: Option<Box<MenuContext>>,
}
impl MenuFocus {
    pub(super) fn install(
        handle: isize,
        app: slint::Weak<PlayerWindow>,
        active: Rc<Cell<bool>>,
    ) -> Result<Self, String> {
        let hwnd = HWND(handle as *mut _);
        // A registered message cannot collide with toolkit/private window messages.
        // SAFETY: registration reads a static, null-terminated string.
        let restore_message =
            unsafe { RegisterWindowMessageW(w!("riina.native-menu.restore-focus")) };
        if restore_message == 0 {
            return Err("Native menu focus message could not be registered".into());
        }
        let context = Box::new(MenuContext {
            app,
            active,
            generation: Cell::new(0),
            restore_message,
        });
        let pointer = (&*context as *const MenuContext) as usize;
        // SAFETY: the owning UI thread installs/removes this stable box.
        if !unsafe { SetWindowSubclass(hwnd, Some(menu_proc), pointer, pointer) }.as_bool() {
            return Err("Native menu focus handler could not be installed".into());
        }
        Ok(Self {
            hwnd,
            context: Some(context),
        })
    }
}
impl Drop for MenuFocus {
    fn drop(&mut self) {
        if let Some(context) = self.context.take() {
            let pointer = (&*context as *const MenuContext) as usize;
            // SAFETY: the callback context remains owned until removal.
            let removed =
                unsafe { RemoveWindowSubclass(self.hwnd, Some(menu_proc), pointer) }.as_bool();
            // SAFETY: this only queries whether Windows still owns the handle.
            if !removed && unsafe { IsWindow(Some(self.hwnd)) }.as_bool() {
                // Failed removal cannot leave Windows with freed refdata.
                Box::leak(context);
            }
        }
    }
}

pub(super) fn request(handle: isize) -> windows::core::Result<()> {
    // SAFETY: the caller retains the owner on this UI thread. Queue entry after
    // the current key-up instead of nesting the menu loop in a Slint callback.
    unsafe {
        PostMessageW(
            Some(HWND(handle as *mut _)),
            WM_SYSCOMMAND,
            WPARAM(SC_KEYMENU as usize),
            LPARAM(0),
        )
    }
}

pub(super) fn focus_chrome(window: &slint::Window) {
    window.with_winit_window(|window| {
        if let Ok(handle) = window.window_handle()
            && let RawWindowHandle::Win32(handle) = handle.as_raw()
        {
            // These windows share the UI thread. Winit's foreground helper
            // simulates Alt; menu recovery must not generate another menu key.
            // SAFETY: the borrowed window retains this live HWND.
            let _ = unsafe { SetFocus(Some(HWND(handle.hwnd.get() as *mut _))) };
        }
    });
}

unsafe extern "system" fn menu_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    refdata: usize,
) -> LRESULT {
    // SAFETY: MenuFocus retains the stable box until subclass removal.
    let context = unsafe { &*(refdata as *const MenuContext) };
    if wparam.0 == VK_F10.0 as usize && !context.active.get() {
        if matches!(message, WM_KEYDOWN | WM_SYSKEYDOWN) {
            return LRESULT(0);
        }
        if matches!(message, WM_KEYUP | WM_SYSKEYUP) {
            // F10 can reach the framed owner directly, without visiting the
            // shared controls. Use the same queued entry for either input scope.
            let _ = request(hwnd.0 as isize);
            return LRESULT(0);
        }
    }
    if message == WM_SYSCOMMAND && wparam.0 & 0xfff0 == SC_KEYMENU as usize {
        context.active.set(true);
        // SAFETY: this owner belongs to the current thread and subclass chain.
        let _ = unsafe { SetFocus(Some(hwnd)) };
    }
    if message == WM_ENTERMENULOOP {
        context.active.set(true);
        context
            .generation
            .set(context.generation.get().wrapping_add(1));
    }
    if message == WM_EXITMENULOOP {
        // Windows still unwinds focus internally here. Restore through its own
        // queue; a later menu loop invalidates this restoration generation.
        // SAFETY: this retained owner receives its registered message on the UI thread.
        let _ = unsafe {
            PostMessageW(
                Some(hwnd),
                context.restore_message,
                WPARAM(context.generation.get()),
                LPARAM(0),
            )
        };
    }
    if message == context.restore_message {
        if wparam.0 == context.generation.get() {
            context.active.set(false);
            if let Some(app) = context.app.upgrade() {
                focus_chrome(app.window());
            }
        }
        return LRESULT(0);
    }
    // SAFETY: preserve the original subclass chain for all other messages.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}
