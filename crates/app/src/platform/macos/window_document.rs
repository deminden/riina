use objc2_app_kit::NSView;
use objc2_foundation::{NSFileManager, NSString, NSURL};
use slint::winit_030::WinitWindowAccessor;
use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::path::Path;

/// Use Finder's localized display name and extension-visibility rules.
pub(crate) fn display_name_for_path(path: &Path) -> Option<String> {
    let path = path.to_str().filter(|path| Path::new(path).is_absolute())?;
    let path = NSString::from_str(path);
    Some(
        NSFileManager::defaultManager()
            .displayNameAtPath(&path)
            .to_string(),
    )
}

/// Let AppKit display the current local media as the window's represented document.
pub(crate) fn set_represented_file(window: &slint::Window, file: Option<&Path>) {
    let represented_path = file
        .filter(|path| path.is_absolute())
        .and_then(Path::to_str);
    let represented_path = represented_path.map(NSString::from_str);
    let represented_url = represented_path.as_deref().map(NSURL::fileURLWithPath);

    window.with_winit_window(|winit_window| {
        let Ok(handle) = winit_window.window_handle() else {
            return;
        };
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return;
        };
        // SAFETY: the borrowed AppKit handle points to the live Winit window's NSView.
        let view = unsafe { &*handle.ns_view.as_ptr().cast::<NSView>() };
        let Some(native_window) = view.window() else {
            return;
        };

        let unchanged = match (represented_path.as_ref(), native_window.representedURL()) {
            (None, None) => true,
            (Some(expected), Some(current)) => current
                .path()
                .is_some_and(|actual| actual.isEqualToString(expected)),
            _ => false,
        };
        if !unchanged {
            native_window.setRepresentedURL(represented_url.as_deref());
        }
    });
}
