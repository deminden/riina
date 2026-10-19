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
        System::SystemServices::{
            APPCOMMAND_MEDIA_FAST_FORWARD, APPCOMMAND_MEDIA_NEXTTRACK, APPCOMMAND_MEDIA_PAUSE,
            APPCOMMAND_MEDIA_PLAY, APPCOMMAND_MEDIA_PLAY_PAUSE, APPCOMMAND_MEDIA_PREVIOUSTRACK,
            APPCOMMAND_MEDIA_REWIND, APPCOMMAND_MEDIA_STOP, APPCOMMAND_VOLUME_DOWN,
            APPCOMMAND_VOLUME_MUTE, APPCOMMAND_VOLUME_UP,
        },
        UI::{
            Input::KeyboardAndMouse::{
                GetAsyncKeyState, SetActiveWindow, SetFocus, VK_F10, VK_MENU,
            },
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::{
                DrawMenuBar, GetForegroundWindow, GetMenu, GetMenuItemCount, GetMenuStringW,
                GetSubMenu, HMENU, IsWindow, MENUITEMINFOW, MF_BYPOSITION, MIIM_STRING,
                PostMessageW, RegisterWindowMessageW, SC_KEYMENU, SetMenu, SetMenuItemInfoW,
                WM_APPCOMMAND, WM_ENTERMENULOOP, WM_EXITMENULOOP, WM_INITMENU, WM_INITMENUPOPUP,
                WM_KEYDOWN, WM_KEYUP, WM_SETREDRAW, WM_SYSCOMMAND, WM_SYSKEYDOWN, WM_SYSKEYUP,
            },
        },
    },
    core::{PWSTR, w},
};

struct MenuContext {
    app: slint::Weak<PlayerWindow>,
    active: Rc<Cell<bool>>,
    generation: Cell<usize>,
    enter_message: u32,
    restore_message: u32,
    detached_menu: Cell<Option<HMENU>>,
}

pub(super) struct MenuFocus {
    hwnd: HWND,
    context: Option<Box<MenuContext>>,
    _controls_menu: Option<super::menu_proxy::MenuProxy>,
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
        // SAFETY: registration reads a static, null-terminated string.
        let enter_message = unsafe { RegisterWindowMessageW(w!("riina.native-menu.enter")) };
        if restore_message == 0 || enter_message == 0 {
            return Err("Native menu focus message could not be registered".into());
        }
        let context = Box::new(MenuContext {
            app,
            active,
            generation: Cell::new(0),
            enter_message,
            restore_message,
            detached_menu: Cell::new(None),
        });
        let pointer = (&*context as *const MenuContext) as usize;
        // SAFETY: the owning UI thread installs/removes this stable box.
        if !unsafe { SetWindowSubclass(hwnd, Some(menu_proc), pointer, pointer) }.as_bool() {
            return Err("Native menu focus handler could not be installed".into());
        }
        Ok(Self {
            hwnd,
            context: Some(context),
            _controls_menu: None,
        })
    }

    pub(super) fn attach_controls(&mut self, window: &slint::Window) -> Result<(), String> {
        let controls = window
            .with_winit_window(|window| {
                window
                    .window_handle()
                    .ok()
                    .and_then(|handle| match handle.as_raw() {
                        RawWindowHandle::Win32(handle) => Some(HWND(handle.hwnd.get() as *mut _)),
                        _ => None,
                    })
            })
            .flatten()
            .ok_or("Native controls menu handle is unavailable")?;
        self._controls_menu = Some(super::menu_proxy::MenuProxy::install(controls, self.hwnd)?);
        Ok(())
    }

    pub(super) fn set_visible(&self, visible: bool) -> Result<(), String> {
        let Some(context) = self.context.as_ref() else {
            return Ok(());
        };
        if visible {
            if let Some(menu) = context.detached_menu.get() {
                // Muda retains ownership of the stable root HMENU while detached.
                unsafe { SetMenu(self.hwnd, Some(menu)) }.map_err(|error| error.to_string())?;
                context.detached_menu.set(None);
            } else {
                return Ok(());
            }
        } else {
            let menu = unsafe { GetMenu(self.hwnd) };
            if menu.0.is_null() {
                return Ok(());
            }
            unsafe { SetMenu(self.hwnd, None) }.map_err(|error| error.to_string())?;
            context.detached_menu.set(Some(menu));
        }
        // Recalculate the non-client strip once, without removing/reinserting
        // every menu entry or replacing the host's underlying menu object.
        unsafe { DrawMenuBar(self.hwnd) }.map_err(|error| error.to_string())
    }
}

fn update_information_label(menu: HMENU, prefix: &str, label: &str) -> windows::core::Result<()> {
    // Informational rows refresh just before their submenu paints. Feeding
    // each continuous playback/slider update into Slint would rebuild the bar
    // and repeatedly change the native video client's height.
    let count = unsafe { GetMenuItemCount(Some(menu)) };
    for index in 0..count {
        let mut title = [0_u16; 128];
        let length = unsafe { GetMenuStringW(menu, index as u32, Some(&mut title), MF_BYPOSITION) };
        if length > 0 && String::from_utf16_lossy(&title[..length as usize]).starts_with(prefix) {
            let mut label: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
            let info = MENUITEMINFOW {
                cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
                fMask: MIIM_STRING,
                dwTypeData: PWSTR(label.as_mut_ptr()),
                ..Default::default()
            };
            // SAFETY: Windows copies this UTF-16 string during the call; the
            // live submenu and all command IDs/checked states remain unchanged.
            unsafe { SetMenuItemInfoW(menu, index as u32, true, &info) }?;
            break;
        }
    }
    Ok(())
}

fn plugin_menu_handle(hwnd: HWND) -> isize {
    // A Slint rebuild can replace submenu handles. Resolve the current handle
    // at activation instead of comparing against an earlier menu generation.
    let menu = unsafe { GetMenu(hwnd) };
    if menu.0.is_null() {
        return 0;
    }
    let count = unsafe { GetMenuItemCount(Some(menu)) };
    for index in 0..count {
        let mut title = [0u16; 64];
        let length = unsafe { GetMenuStringW(menu, index as u32, Some(&mut title), MF_BYPOSITION) };
        if length > 0 && String::from_utf16_lossy(&title[..length as usize]) == "Plugin" {
            return unsafe { GetSubMenu(menu, index) }.0 as isize;
        }
    }
    0
}
impl Drop for MenuFocus {
    fn drop(&mut self) {
        let _ = self.set_visible(true);
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
    request_with_key(HWND(handle as *mut _), LPARAM(0))
}

pub(super) fn request_with_key(owner: HWND, character: LPARAM) -> windows::core::Result<()> {
    // SAFETY: repeated registration of the same static name returns the same ID.
    let message = unsafe { RegisterWindowMessageW(w!("riina.native-menu.enter")) };
    if message == 0 {
        return Err(windows::core::Error::from_thread());
    }
    // SAFETY: the caller retains the owner on this UI thread. Queue entry after
    // the current key-up instead of nesting the menu loop in a Slint callback.
    unsafe { PostMessageW(Some(owner), message, WPARAM(0), character) }
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
    if message == WM_SETREDRAW {
        // Slint/Muda batches menu updates with WM_SETREDRAW. DefWindowProc
        // implements that by clearing/restoring WS_VISIBLE on the host, which
        // also makes mpv's independently presented child temporarily invisible.
        // Volume labels and fullscreen menu removal must not interrupt video.
        // This host has no Slint client controls to batch; Muda still repaints
        // its non-client menu with DrawMenuBar after completing the update.
        return LRESULT(0);
    }
    if message == WM_APPCOMMAND
        && let Some(app) = context.app.upgrade()
        && app.get_key_binding_use_system_media_controls()
        && let Some(key) = system_media_key(((lparam.0 as u32) >> 16) & 0x7ff)
        && app.invoke_dispatch_key_binding(key.into(), false, false, false, false)
    {
        return LRESULT(1);
    }
    if message == WM_INITMENU
        && let Some(app) = context.app.upgrade()
    {
        // Playback observations are coalesced for ordinary updates. Refresh the
        // menu's state labels synchronously before Windows paints its first row.
        app.invoke_native_menu_will_open();
    }
    if message == WM_INITMENUPOPUP
        && let Some(app) = context.app.upgrade()
    {
        let information = [
            ("Volume:", app.get_volume_menu_title().to_string()),
            ("Speed:", format!("Speed: {}x", app.get_speed_edit_value())),
            (
                "Audio Delay:",
                format!("Audio Delay: {}s", app.get_audio_delay_text()),
            ),
            (
                "Subtitle Delay:",
                format!("Subtitle Delay: {}s", app.get_subtitle_delay_text()),
            ),
        ];
        for (prefix, label) in information {
            if let Err(error) = update_information_label(HMENU(wparam.0 as *mut _), prefix, &label)
            {
                app.set_status(format!("Could not refresh playback menu: {error}").into());
            }
        }
        let plugin_menu = plugin_menu_handle(hwnd);
        if plugin_menu != 0 && wparam.0 as isize == plugin_menu {
            app.invoke_plugin_menu_will_open();
        }
    }
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
    if message == context.enter_message {
        // Coalesce queued entry requests while Windows already owns menu input.
        // A second activation must not nest another modal loop.
        if context.active.get() {
            return LRESULT(0);
        }
        // A queued keyboard request can outlive the transition into an editor
        // or modal dialog. Recheck the current input scope before activation.
        let Some(app) = context.app.upgrade() else {
            return LRESULT(0);
        };
        if app.get_crop_editing() || app.get_dialog_open() {
            return LRESULT(0);
        }
        context.active.set(true);
        context
            .generation
            .set(context.generation.get().wrapping_add(1));
        // Keyboard focus alone can leave the owned popup active. Windows needs
        // the framed menu owner active before processing its system command.
        // SAFETY: this owner belongs to the current thread and subclass chain.
        let _ = unsafe { SetActiveWindow(hwnd) };
        // SAFETY: activation retains the same live owner.
        let _ = unsafe { SetFocus(Some(hwnd)) };
        // Queue the system command after owner activation has fully unwound.
        // SAFETY: the subclass retains this owner on the UI thread.
        if unsafe {
            PostMessageW(
                Some(hwnd),
                WM_SYSCOMMAND,
                WPARAM(SC_KEYMENU as usize),
                lparam,
            )
        }
        .is_err()
        {
            context.active.set(false);
            if let Some(app) = context.app.upgrade() {
                focus_chrome(app.window());
            }
        }
        return LRESULT(0);
    }
    if message == WM_SYSCOMMAND && wparam.0 & 0xfff0 == SC_KEYMENU as usize {
        let Some(app) = context.app.upgrade() else {
            context.active.set(false);
            return LRESULT(0);
        };
        if app.get_crop_editing() || app.get_dialog_open() {
            context.active.set(false);
            // Editing may begin between activation and its queued system
            // command. Return input to the editor without disturbing a dialog.
            if !app.get_dialog_open() {
                focus_chrome(app.window());
            }
            return LRESULT(0);
        }
        context.active.set(true);
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
                // Cancelling by activating another application must not bring
                // this player back. Only recover within this owner/popup pair.
                // SAFETY: this only queries the foreground handle.
                let foreground = unsafe { GetForegroundWindow() };
                let owns_foreground = foreground == hwnd || app.window().with_winit_window(|window| {
                    window.window_handle().is_ok_and(|handle| {
                        matches!(handle.as_raw(), RawWindowHandle::Win32(handle) if HWND(handle.hwnd.get() as *mut _) == foreground)
                    })
                }).unwrap_or(false);
                if owns_foreground {
                    // Query after the modal loop unwinds. The sign bit reports
                    // current physical state; the low "pressed since" bit is not reliable.
                    // SAFETY: VK_MENU is a valid virtual key; this reads input state only.
                    let alt_held = unsafe { GetAsyncKeyState(VK_MENU.0 as i32) } < 0;
                    super::menu_modifiers::reconcile_alt(app.window(), alt_held);
                    // An action may have opened an editor/dialog. Repair its
                    // modifier state too, while retaining its native focus.
                    if !app.get_dialog_open() {
                        focus_chrome(app.window());
                    }
                }
            }
        }
        return LRESULT(0);
    }
    // SAFETY: preserve the original subclass chain for all other messages.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

fn system_media_key(command: u32) -> Option<&'static str> {
    if command == APPCOMMAND_MEDIA_PLAY.0 {
        Some("PLAY")
    } else if command == APPCOMMAND_MEDIA_PAUSE.0 {
        Some("PAUSE")
    } else if command == APPCOMMAND_MEDIA_PLAY_PAUSE.0 {
        Some("PLAYPAUSE")
    } else if command == APPCOMMAND_MEDIA_STOP.0 {
        Some("STOP")
    } else if command == APPCOMMAND_MEDIA_NEXTTRACK.0 {
        Some("NEXT")
    } else if command == APPCOMMAND_MEDIA_PREVIOUSTRACK.0 {
        Some("PREV")
    } else if command == APPCOMMAND_MEDIA_FAST_FORWARD.0 {
        Some("FORWARD")
    } else if command == APPCOMMAND_MEDIA_REWIND.0 {
        Some("REWIND")
    } else if command == APPCOMMAND_VOLUME_UP.0 {
        Some("VOLUME_UP")
    } else if command == APPCOMMAND_VOLUME_DOWN.0 {
        Some("VOLUME_DOWN")
    } else if command == APPCOMMAND_VOLUME_MUTE.0 {
        Some("MUTE")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreateMenu, CreatePopupMenu, CreateWindowExW, DestroyMenu, DestroyWindow,
        GetClientRect, IsWindowVisible, MF_DISABLED, MF_POPUP, MF_STRING, SW_HIDE, SendMessageW,
        ShowWindow, WINDOW_EX_STYLE, WS_CHILD, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
        WS_VISIBLE,
    };

    struct TestWindow(HWND);

    impl Drop for TestWindow {
        fn drop(&mut self) {
            // SAFETY: the test owns this window on the creating thread.
            let _ = unsafe { DestroyWindow(self.0) };
        }
    }

    struct TestMenu {
        hwnd: HWND,
        menu: HMENU,
    }

    impl Drop for TestMenu {
        fn drop(&mut self) {
            // SAFETY: detach before destruction so the test window never owns
            // a freed menu. DestroyMenu also releases the Audio submenu.
            unsafe {
                let _ = SetMenu(self.hwnd, None);
                let _ = DestroyMenu(self.menu);
            }
        }
    }

    fn test_host() -> TestWindow {
        // SAFETY: STATIC is a built-in class; no pointers outlive this call.
        TestWindow(unsafe {
            CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                w!("STATIC"),
                w!("Riina menu redraw regression"),
                WS_POPUP | WS_VISIBLE,
                -32000,
                -32000,
                640,
                360,
                None,
                None,
                None,
                None,
            )
            .unwrap()
        })
    }

    #[test]
    fn queued_menu_entry_is_coalesced_while_owner_has_menu_input() {
        use windows::Win32::UI::WindowsAndMessaging::{MSG, PM_REMOVE, PeekMessageW, WS_DISABLED};
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new().unwrap();
        // Hidden and disabled: native focus calls cannot activate this test
        // window. Posted system commands are removed, never dispatched.
        let host = TestWindow(
            unsafe {
                CreateWindowExW(
                    WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                    w!("STATIC"),
                    w!("Queued menu input test"),
                    WS_POPUP | WS_DISABLED,
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
        );
        let active = Rc::new(Cell::new(false));
        let lifecycle =
            MenuFocus::install(host.0.0 as isize, app.as_weak(), active.clone()).unwrap();
        let enter = lifecycle.context.as_ref().unwrap().enter_message;
        let foreground = unsafe { GetForegroundWindow() };
        unsafe {
            SendMessageW(
                host.0,
                enter,
                Some(WPARAM(0)),
                Some(LPARAM(i32::from(b' ') as isize)),
            );
            SendMessageW(host.0, enter, Some(WPARAM(0)), Some(LPARAM(0)));
        }
        assert!(active.get());
        assert_eq!(lifecycle.context.as_ref().unwrap().generation.get(), 1);
        let mut command = MSG::default();
        assert!(
            unsafe {
                PeekMessageW(
                    &mut command,
                    Some(host.0),
                    WM_SYSCOMMAND,
                    WM_SYSCOMMAND,
                    PM_REMOVE,
                )
            }
            .as_bool()
        );
        assert_eq!(command.lParam.0, i32::from(b' ') as isize);
        assert!(
            !unsafe {
                PeekMessageW(
                    &mut command,
                    Some(host.0),
                    WM_SYSCOMMAND,
                    WM_SYSCOMMAND,
                    PM_REMOVE,
                )
            }
            .as_bool()
        );
        assert_eq!(unsafe { GetForegroundWindow() }, foreground);
    }

    #[test]
    fn menu_redraw_batch_keeps_video_visible_and_does_not_show_hidden_host() {
        // Use the built-in class without a Slint event loop or playback DLL.
        // Off-screen, non-activating windows cannot steal the user's focus.
        let host = test_host();
        let child = TestWindow(unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("Video child"),
                WS_CHILD | WS_VISIBLE,
                0,
                0,
                1,
                1,
                Some(host.0),
                None,
                None,
                None,
            )
            .unwrap()
        });
        // Confirm the native default being guarded: the redraw batch hides
        // both the host and the independently owned playback child.
        unsafe {
            SendMessageW(host.0, WM_SETREDRAW, Some(WPARAM(0)), None);
            assert!(!IsWindowVisible(child.0).as_bool());
            SendMessageW(host.0, WM_SETREDRAW, Some(WPARAM(1)), None);
        }
        let _menu = MenuFocus::install(
            host.0.0 as isize,
            slint::Weak::default(),
            Rc::new(Cell::new(false)),
        )
        .unwrap();
        // SAFETY: all messages and queries target the live test-owned windows.
        unsafe {
            SendMessageW(host.0, WM_SETREDRAW, Some(WPARAM(0)), None);
            assert!(IsWindowVisible(host.0).as_bool());
            assert!(IsWindowVisible(child.0).as_bool());
            SendMessageW(host.0, WM_SETREDRAW, Some(WPARAM(1)), None);
            assert!(IsWindowVisible(child.0).as_bool());

            let _ = ShowWindow(host.0, SW_HIDE);
            SendMessageW(host.0, WM_SETREDRAW, Some(WPARAM(1)), None);
            assert!(!IsWindowVisible(host.0).as_bool());
            assert!(!IsWindowVisible(child.0).as_bool());
        }
    }

    #[test]
    fn information_labels_preserve_menu_identity_and_video_client_bounds() {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::UI::WindowsAndMessaging::GetMenuState;
        let host = test_host();
        // SAFETY: this thread owns all menus/windows and retains them below.
        let menu = TestMenu {
            hwnd: host.0,
            menu: unsafe { CreateMenu() }.unwrap(),
        };
        let audio = unsafe { CreatePopupMenu() }.unwrap();
        unsafe {
            AppendMenuW(audio, MF_STRING | MF_DISABLED, 1, w!("Volume: 100")).unwrap();
            AppendMenuW(audio, MF_STRING, 2, w!("Volume +")).unwrap();
            AppendMenuW(audio, MF_STRING | MF_DISABLED, 3, w!("Speed: 1x")).unwrap();
            AppendMenuW(audio, MF_STRING | MF_DISABLED, 4, w!("Audio Delay: 0s")).unwrap();
            AppendMenuW(audio, MF_STRING | MF_DISABLED, 5, w!("Subtitle Delay: 0s")).unwrap();
            AppendMenuW(menu.menu, MF_POPUP, audio.0 as usize, w!("Audio")).unwrap();
            SetMenu(host.0, Some(menu.menu)).unwrap();
        }
        let lifecycle = MenuFocus::install(
            host.0.0 as isize,
            slint::Weak::default(),
            Rc::new(Cell::new(false)),
        )
        .unwrap();
        let mut before = RECT::default();
        unsafe { GetClientRect(host.0, &mut before) }.unwrap();
        for (prefix, label, position) in [
            ("Volume:", "Volume: 0", 0),
            ("Volume:", "Volume: 200", 0),
            ("Speed:", "Speed: 1.1x", 2),
            ("Speed:", "Speed: 0.9x", 2),
            ("Audio Delay:", "Audio Delay: -0.5s", 3),
            ("Subtitle Delay:", "Subtitle Delay: 120s", 4),
        ] {
            update_information_label(audio, prefix, label).unwrap();
            let mut after = RECT::default();
            let mut text = [0_u16; 128];
            unsafe {
                GetClientRect(host.0, &mut after).unwrap();
                assert_eq!(GetMenu(host.0), menu.menu);
                assert_eq!(GetSubMenu(menu.menu, 0), audio);
                assert_eq!(GetMenuItemCount(Some(audio)), 5);
                let length = GetMenuStringW(audio, position, Some(&mut text), MF_BYPOSITION);
                assert_eq!(String::from_utf16_lossy(&text[..length as usize]), label);
                assert_ne!(
                    GetMenuState(audio, position, MF_BYPOSITION) & MF_DISABLED.0,
                    0
                );
                assert_eq!(GetMenuState(audio, 1, MF_BYPOSITION) & MF_DISABLED.0, 0);
            }
            assert_eq!((before.right, before.bottom), (after.right, after.bottom));
        }
        for _ in 0..3 {
            lifecycle.set_visible(false).unwrap();
            assert!(unsafe { GetMenu(host.0) }.0.is_null());
            assert!(unsafe { IsWindowVisible(host.0) }.as_bool());
            lifecycle.set_visible(true).unwrap();
            assert_eq!(unsafe { GetMenu(host.0) }, menu.menu);
            assert_eq!(unsafe { GetSubMenu(menu.menu, 0) }, audio);
        }
    }
}
