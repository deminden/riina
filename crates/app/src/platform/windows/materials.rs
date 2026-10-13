//! DWM-backed Windows surface materials with an ordinary-surface fallback.

use slint::winit_030::winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};
use windows::Win32::{
    Foundation::HWND,
    Graphics::Dwm::{
        DWM_SYSTEMBACKDROP_TYPE, DWMSBT_MAINWINDOW, DWMWA_SYSTEMBACKDROP_TYPE,
        DwmGetWindowAttribute, DwmSetWindowAttribute,
    },
};

/// Apply the Windows 11 Mica material to the persistent player host.
///
/// Builds before Windows 11 22H2 can reject the attribute; callers keep the
/// normal renderer surface in that case rather than failing player startup.
pub(crate) fn apply_main_window_mica(window: &Window) -> Result<(), String> {
    let hwnd = hwnd(window)?;
    let requested = DWMSBT_MAINWINDOW;
    // SAFETY: DWM reads this initialized enum synchronously and does not retain
    // its pointer. The HWND belongs to the live Winit window.
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            (&requested as *const DWM_SYSTEMBACKDROP_TYPE).cast(),
            std::mem::size_of::<DWM_SYSTEMBACKDROP_TYPE>() as u32,
        )
    }
    .map_err(|error| format!("Could not apply the Windows Mica backdrop: {error}"))?;

    let mut applied = DWM_SYSTEMBACKDROP_TYPE(0);
    // SAFETY: DWM writes one initialized enum-sized value for the live HWND.
    unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            (&mut applied as *mut DWM_SYSTEMBACKDROP_TYPE).cast(),
            std::mem::size_of::<DWM_SYSTEMBACKDROP_TYPE>() as u32,
        )
    }
    .map_err(|error| format!("Could not read back the Windows backdrop type: {error}"))?;
    if applied != requested {
        return Err(format!(
            "DWM reported backdrop type {}, expected Mica type {}",
            applied.0, requested.0
        ));
    }
    Ok(())
}

fn hwnd(window: &Window) -> Result<HWND, String> {
    let handle = window.window_handle().map_err(|error| error.to_string())?;
    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => Ok(HWND(handle.hwnd.get() as *mut _)),
        _ => Err("Windows Mica requires a Win32 window".into()),
    }
}
