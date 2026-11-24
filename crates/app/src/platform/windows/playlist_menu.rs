//! Win32 tracks a popup independently of the application's menu bar.
use crate::platform::playlist_menu::{Entry, Target};
use slint::{
    LogicalPosition, Window,
    winit_030::{
        WinitWindowAccessor,
        winit::raw_window_handle::{HasWindowHandle, RawWindowHandle},
    },
};
use windows::{
    Win32::{
        Foundation::{HWND, POINT},
        Graphics::Gdi::ClientToScreen,
        UI::WindowsAndMessaging::{
            AppendMenuW, CreatePopupMenu, DestroyMenu, HMENU, MF_CHECKED, MF_GRAYED, MF_POPUP,
            MF_SEPARATOR, MF_STRING, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON,
            TrackPopupMenuEx,
        },
    },
    core::PCWSTR,
};

pub(in crate::platform) fn popup<A>(
    window: &Window,
    position: LogicalPosition,
    entries: Vec<Entry<A>>,
) -> Result<Option<A>, String> {
    let position = position.to_physical(window.scale_factor());
    window
        .with_winit_window(|native| {
            let Ok(handle) = native.window_handle() else {
                return Ok(None);
            };
            let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                return Ok(None);
            };
            let hwnd = HWND(handle.hwnd.get() as *mut _);
            let mut point = POINT {
                x: position.x,
                y: position.y,
            };
            // SAFETY: the window handle is borrowed and the POINT lives on the stack.
            if !unsafe { ClientToScreen(hwnd, &mut point) }.as_bool() {
                return Ok(None);
            }
            let mut actions = Vec::new();
            let menu = Menu::new(entries, &mut actions).map_err(|error| error.to_string())?;
            // SAFETY: the menu and the owning window live through the native menu loop.
            let chosen = unsafe {
                TrackPopupMenuEx(
                    menu.0,
                    (TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON).0,
                    point.x,
                    point.y,
                    hwnd,
                    None,
                )
            };
            Ok(usize::try_from(chosen.0)
                .ok()
                .and_then(|id| id.checked_sub(1))
                .and_then(|index| actions.into_iter().nth(index)))
        })
        .unwrap_or(Ok(None))
}

struct Menu(HMENU);

impl Menu {
    fn new<A>(entries: Vec<Entry<A>>, actions: &mut Vec<A>) -> windows::core::Result<Self> {
        // SAFETY: this thread owns the new handle and Menu drops it on every exit path.
        let menu = Self(unsafe { CreatePopupMenu()? });
        for entry in entries {
            menu.append(entry, actions)?;
        }
        Ok(menu)
    }

    fn append<A>(&self, entry: Entry<A>, actions: &mut Vec<A>) -> windows::core::Result<()> {
        let Entry::Item {
            title,
            enabled,
            checked,
            target,
            ..
        } = entry
        else {
            // SAFETY: separator entries require no command identifier or string.
            return unsafe { AppendMenuW(self.0, MF_SEPARATOR, 0, PCWSTR::null()) };
        };
        let mut flags = MF_STRING;
        if !enabled {
            flags |= MF_GRAYED;
        }
        if checked {
            flags |= MF_CHECKED;
        }
        // Ampersands in plugin titles are literal text rather than Win32 mnemonics.
        let label = title
            .replace('&', "&&")
            .encode_utf16()
            .chain([0])
            .collect::<Vec<_>>();
        let label = PCWSTR(label.as_ptr());
        match target {
            Target::Action(action) => {
                actions.push(action);
                // SAFETY: the menu is live and copies the terminated label before returning.
                unsafe { AppendMenuW(self.0, flags, actions.len(), label) }
            }
            Target::Submenu(entries) => {
                let submenu = Self::new(entries, actions)?;
                // SAFETY: both handles and the label remain live during insertion.
                unsafe { AppendMenuW(self.0, flags | MF_POPUP, submenu.0.0 as usize, label)? };
                // DestroyMenu recursively frees the submenu after ownership transfers.
                std::mem::forget(submenu);
                Ok(())
            }
        }
    }
}

impl Drop for Menu {
    fn drop(&mut self) {
        // SAFETY: Menu owns the handle and its attached submenus.
        let _ = unsafe { DestroyMenu(self.0) };
    }
}
