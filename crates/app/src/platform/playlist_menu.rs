//! Platform presentation for a playlist menu whose tree is supplied at runtime.
use slint::{LogicalPosition, Window};

#[cfg(target_os = "linux")]
mod x11;

#[cfg_attr(
    not(any(target_os = "windows", target_os = "macos", target_os = "linux")),
    allow(dead_code)
)]
pub(crate) enum Entry<A> {
    Separator,
    Item {
        title: String,
        enabled: bool,
        checked: bool,
        target: Target<A>,
    },
}

#[cfg_attr(
    not(any(target_os = "windows", target_os = "macos", target_os = "linux")),
    allow(dead_code)
)]
pub(crate) enum Target<A> {
    Action(A),
    Submenu(Vec<Entry<A>>),
}

pub(crate) fn supports(window: &Window) -> bool {
    #[cfg(target_os = "linux")]
    return x11::owner(window).is_some();

    #[cfg(not(target_os = "linux"))]
    {
        let _ = window;
        cfg!(any(target_os = "windows", target_os = "macos"))
    }
}

pub(crate) fn dismiss(window: &Window) {
    #[cfg(target_os = "linux")]
    x11::dismiss(window);
    #[cfg(not(target_os = "linux"))]
    let _ = window;
}

pub(crate) fn popup<A: 'static>(
    window: &Window,
    position: LogicalPosition,
    entries: Vec<Entry<A>>,
    selected: impl FnOnce(A) + 'static,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let action = super::windows::playlist_menu::popup(window, position, entries)?;

    #[cfg(target_os = "macos")]
    let action = super::macos::playlist_menu::popup(window, position, entries)?;

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        if let Some(action) = action {
            selected(action);
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    return x11::popup(window, position, entries, selected);

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = (window, position, entries, selected);
        Ok(())
    }
}
