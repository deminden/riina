//! The framed player owns native menus; its controls popup only forwards entry.
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{IsWindow, SC_KEYMENU, WM_SYSCOMMAND},
    },
};

pub(super) struct MenuProxy {
    controls: HWND,
    owner: Box<HWND>,
}

impl MenuProxy {
    pub(super) fn install(controls: HWND, owner: HWND) -> Result<Self, String> {
        let proxy = Self {
            controls,
            owner: Box::new(owner),
        };
        let pointer = (&*proxy.owner as *const HWND) as usize;
        // SAFETY: this stable box is retained until subclass removal on the UI thread.
        if !unsafe { SetWindowSubclass(controls, Some(menu_proxy_proc), pointer, pointer) }
            .as_bool()
        {
            return Err("Controls menu routing could not be installed".into());
        }
        Ok(proxy)
    }
}

impl Drop for MenuProxy {
    fn drop(&mut self) {
        let pointer = (&*self.owner as *const HWND) as usize;
        // SAFETY: the context remains alive during removal from its owning UI thread.
        let removed =
            unsafe { RemoveWindowSubclass(self.controls, Some(menu_proxy_proc), pointer) }
                .as_bool();
        if !removed && unsafe { IsWindow(Some(self.controls)) }.as_bool() {
            // A live callback must never retain freed refdata if removal fails.
            Box::leak(std::mem::replace(
                &mut self.owner,
                Box::new(HWND::default()),
            ));
        }
    }
}

unsafe extern "system" fn menu_proxy_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    refdata: usize,
) -> LRESULT {
    if message == WM_SYSCOMMAND && wparam.0 & 0xfff0 == SC_KEYMENU as usize {
        // SAFETY: MenuProxy retains this immutable owner handle until removal.
        let owner = unsafe { *(refdata as *const HWND) };
        // Queue entry after this key message, preserving Space/system menu and
        // letter mnemonics. Opening the popup's own menu bypasses owner recovery.
        if super::menu_focus::request_with_key(owner, lparam).is_ok() {
            return LRESULT(0);
        }
    }
    // SAFETY: all unrelated messages retain the original subclass chain.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::{
        Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, MSG, PM_REMOVE, PeekMessageW, RegisterWindowMessageW,
            SendMessageW, WINDOW_EX_STYLE, WS_POPUP,
        },
        core::w,
    };

    struct TestWindow(HWND);
    impl TestWindow {
        fn new() -> Self {
            // SAFETY: hidden built-in windows belong to this test thread. No menu is opened.
            Self(
                unsafe {
                    CreateWindowExW(
                        WINDOW_EX_STYLE::default(),
                        w!("STATIC"),
                        w!("Menu routing test"),
                        WS_POPUP,
                        -32000,
                        -32000,
                        1,
                        1,
                        None,
                        None,
                        None,
                        None,
                    )
                }
                .unwrap(),
            )
        }
    }
    impl Drop for TestWindow {
        fn drop(&mut self) {
            let _ = unsafe { DestroyWindow(self.0) };
        }
    }

    #[test]
    fn controls_menu_entry_reaches_owner_with_its_character_and_detaches_cleanly() {
        let owner = TestWindow::new();
        let controls = TestWindow::new();
        let proxy = MenuProxy::install(controls.0, owner.0).unwrap();
        let id = (&*proxy.owner as *const HWND) as usize;
        let message = unsafe { RegisterWindowMessageW(w!("riina.native-menu.enter")) };
        assert_ne!(message, 0);
        for character in [0, i32::from(b' '), i32::from(b'f')] {
            let mut queued = MSG::default();
            // SAFETY: SendMessage hits only our proxy; PeekMessage removes the
            // forwarded entry without dispatching it or starting a native menu loop.
            unsafe {
                SendMessageW(
                    controls.0,
                    WM_SYSCOMMAND,
                    Some(WPARAM(SC_KEYMENU as usize | 2)),
                    Some(LPARAM(character as isize)),
                );
                assert!(
                    PeekMessageW(&mut queued, Some(owner.0), message, message, PM_REMOVE).as_bool()
                );
            }
            assert_eq!(queued.hwnd, owner.0);
            assert_eq!(queued.lParam.0, character as isize);
            assert_eq!(queued.wParam.0, 0);
        }
        drop(proxy);
        // Removal by Drop must have retired the exact registered callback.
        assert!(!unsafe { RemoveWindowSubclass(controls.0, Some(menu_proxy_proc), id) }.as_bool());
    }
}
