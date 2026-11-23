//! Platform presentation for a playlist menu whose tree is supplied at runtime.
use slint::{LogicalPosition, Window};

pub(crate) const SUPPORTED: bool = cfg!(any(target_os = "windows", target_os = "macos"));

#[cfg_attr(not(any(target_os = "windows", target_os = "macos")), allow(dead_code))]
pub(crate) enum Entry<A> {
    Separator,
    Item {
        title: String,
        enabled: bool,
        checked: bool,
        target: Target<A>,
    },
}

#[cfg_attr(not(any(target_os = "windows", target_os = "macos")), allow(dead_code))]
pub(crate) enum Target<A> {
    Action(A),
    Submenu(Vec<Entry<A>>),
}

pub(crate) fn popup<A>(
    window: &Window,
    position: LogicalPosition,
    entries: Vec<Entry<A>>,
) -> Result<Option<A>, String> {
    #[cfg(target_os = "windows")]
    return super::windows::playlist_menu::popup(window, position, entries);

    #[cfg(target_os = "macos")]
    return super::macos::playlist_menu::popup(window, position, entries);

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = (window, position, entries);
        Ok(None)
    }
}
