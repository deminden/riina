use crate::PlayerWindow;
use riina_ui::ApplicationMenuText;
use slint::ComponentHandle;
use windows::Win32::UI::WindowsAndMessaging::{
    EnableMenuItem, GetMenuItemCount, GetMenuStringW, HMENU, MF_BYPOSITION, MF_ENABLED, MF_GRAYED,
};

pub(super) fn refresh(app: &PlayerWindow, menu: HMENU) {
    let labels = app.global::<ApplicationMenuText>();
    let state = crate::platform::text_edit::menu_state(app);
    let rows = [
        (labels.get_undo(), state.undo),
        (labels.get_redo(), state.redo),
        (labels.get_cut(), state.cut),
        (labels.get_copy(), state.copy),
        (labels.get_paste(), state.paste),
        (labels.get_delete(), state.delete),
        (labels.get_select_all(), state.select_all),
    ];
    // Update the live native popup in place. Rebuilding its Slint menu while
    // Windows tracks it can detach the bar and disturb the video client bounds.
    // SAFETY: WM_INITMENUPOPUP supplies the live submenu on this UI thread.
    let count = unsafe { GetMenuItemCount(Some(menu)) };
    for index in 0..count {
        let mut label = [0_u16; 128];
        // SAFETY: the index is from this menu and Windows writes into the supplied buffer.
        let length = unsafe { GetMenuStringW(menu, index as u32, Some(&mut label), MF_BYPOSITION) };
        if length <= 0 {
            continue;
        }
        let label = String::from_utf16_lossy(&label[..length as usize]);
        let title = label.split('\t').next().unwrap_or_default();
        if let Some((_, enabled)) = rows.iter().find(|(label, _)| label.as_str() == title) {
            let flags = MF_BYPOSITION | if *enabled { MF_ENABLED } else { MF_GRAYED };
            // SAFETY: update only this row's availability on the owning window thread.
            let _ = unsafe { EnableMenuItem(menu, index as u32, flags) };
        }
    }
}
