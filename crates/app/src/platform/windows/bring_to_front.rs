//! Bring every visible Riina player window forward across independent sessions.

use slint::winit_030::{WinitWindowAccessor, winit};
use std::path::Path;
use windows::{
    Win32::{
        Foundation::{CloseHandle, HWND, LPARAM},
        System::Threading::{
            OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
            QueryFullProcessImageNameW,
        },
        UI::WindowsAndMessaging::{
            EnumWindows, GetWindowThreadProcessId, HWND_TOP, IsIconic, IsWindowVisible, SW_RESTORE,
            SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetForegroundWindow,
            SetWindowPos, ShowWindowAsync,
        },
    },
    core::{BOOL, PWSTR},
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// Restore and raise every visible window from this exact Riina binary,
/// then focus the window whose menu received the command.
pub(crate) fn bring_all_riina_windows_to_front(window: &slint::Window) -> Result<(), String> {
    window
        .with_winit_window(bring_all_native_windows_to_front)
        .flatten()
        .ok_or_else(|| "Could not access the active Riina window".to_owned())?
}

fn bring_all_native_windows_to_front(active: &winit::window::Window) -> Option<Result<(), String>> {
    let active = hwnd(active)?;
    let executable = std::env::current_exe().map_err(|error| error.to_string());
    let executable = match executable {
        Ok(path) => path,
        Err(error) => return Some(Err(error)),
    };
    let mut candidates = Vec::new();
    // SAFETY: EnumWindows invokes the callback synchronously; candidates remains
    // uniquely borrowed until enumeration returns.
    if let Err(error) = unsafe {
        EnumWindows(
            Some(collect_window),
            LPARAM((&mut candidates as *mut Vec<(HWND, u32)>) as isize),
        )
    } {
        return Some(Err(format!("Could not enumerate Riina windows: {error}")));
    }

    let mut matching = candidates
        .into_iter()
        .filter(|(candidate, _)| unsafe { IsWindowVisible(*candidate) }.as_bool())
        .filter(|(_, process_id)| {
            process_image_path(*process_id)
                .as_deref()
                .is_some_and(|path| same_executable(path, &executable))
        })
        .map(|(window, _)| window)
        .collect::<Vec<_>>();

    if let Some(index) = matching.iter().position(|window| *window == active) {
        let active_window = matching.remove(index);
        matching.push(active_window);
    }
    for candidate in matching {
        if unsafe { IsIconic(candidate) }.as_bool() {
            // SAFETY: candidate is a visible top-level Riina HWND from EnumWindows.
            let _ = unsafe { ShowWindowAsync(candidate, SW_RESTORE) };
        }
        // SAFETY: candidate remains a live HWND from EnumWindows. Async posting
        // avoids blocking on a sibling player; flags preserve geometry and defer
        // activation until the invoking player is raised.
        let raised = unsafe {
            SetWindowPos(
                candidate,
                Some(HWND_TOP),
                0,
                0,
                0,
                0,
                SWP_ASYNCWINDOWPOS | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
            )
        };
        if let Err(error) = raised {
            return Some(Err(format!("Could not raise a Riina window: {error}")));
        }
    }
    // The menu action is user initiated. Windows still enforces foreground
    // activation rules, and owned dialogs retain their native z-order relation.
    if !unsafe { SetForegroundWindow(active) }.as_bool() {
        return Some(Err(
            "Windows did not allow the active Riina window to focus".into(),
        ));
    }
    Some(Ok(()))
}

unsafe extern "system" fn collect_window(window: HWND, parameter: LPARAM) -> BOOL {
    let windows = unsafe { &mut *(parameter.0 as *mut Vec<(HWND, u32)>) };
    let mut process_id = 0;
    // SAFETY: process_id is writable and window is supplied by EnumWindows.
    if unsafe { GetWindowThreadProcessId(window, Some(&mut process_id)) } != 0 {
        windows.push((window, process_id));
    }
    BOOL(1)
}

fn process_image_path(process_id: u32) -> Option<String> {
    // SAFETY: this requests only the documented query right for a process that
    // owns a currently enumerated top-level window.
    let process =
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) }.ok()?;
    let mut path = vec![0u16; 32_768];
    let mut length = u32::try_from(path.len()).ok()?;
    // SAFETY: path has writable capacity and length describes that capacity.
    let result = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(path.as_mut_ptr()),
            &mut length,
        )
    };
    // SAFETY: process is the owned handle returned by OpenProcess.
    let _ = unsafe { CloseHandle(process) };
    result.ok()?;
    path.truncate(usize::try_from(length).ok()?);
    Some(String::from_utf16_lossy(&path))
}

fn same_executable(candidate: &str, expected: &Path) -> bool {
    candidate.eq_ignore_ascii_case(&expected.to_string_lossy())
}

fn hwnd(window: &winit::window::Window) -> Option<HWND> {
    let handle = window.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => Some(HWND(handle.hwnd.get() as *mut _)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::same_executable;
    use std::path::Path;

    #[test]
    fn riina_process_matching_uses_a_case_insensitive_full_path() {
        assert!(same_executable(
            r"C:\CODEX\WIINA\TARGET\DEBUG\RIINA.EXE",
            Path::new(r"c:\codex\wiina\target\debug\riina.exe")
        ));
        assert!(!same_executable(
            r"C:\other\riina.exe",
            Path::new(r"c:\codex\wiina\target\debug\riina.exe")
        ));
    }
}
