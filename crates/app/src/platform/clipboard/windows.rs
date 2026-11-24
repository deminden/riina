use clipboard_win::{
    Clipboard,
    options::NoClear,
    raw::{empty, set_file_list_with, set_string_with},
};
use slint::winit_030::{
    WinitWindowAccessor,
    winit::raw_window_handle::{HasWindowHandle, RawWindowHandle},
};

pub(super) fn copy_file_list(
    window: &slint::Window,
    paths: &[String],
    text: &str,
) -> Result<(), String> {
    window
        .with_winit_window(|native| {
            let handle = native.window_handle().map_err(|error| error.to_string())?;
            let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                return Err("The playlist window has no Windows clipboard owner".into());
            };
            // Hold the clipboard open until both formats are available to other apps.
            let _clipboard = Clipboard::new_attempts_for(handle.hwnd.get() as *mut _, 5)
                .map_err(|error| error.to_string())?;
            empty()
                .and_then(|()| set_string_with(text, NoClear))
                .and_then(|()| set_file_list_with(paths, NoClear))
                .map_err(|error| error.to_string())
        })
        .ok_or_else(|| "The playlist window is unavailable".to_owned())?
}
