//! Platform presentation for a playlist menu whose tree is supplied at runtime.
use slint::{LogicalPosition, Window};

pub(crate) const SUPPORTED: bool = cfg!(target_os = "windows");

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) enum Entry<A> {
    Separator,
    Item {
        title: String,
        enabled: bool,
        checked: bool,
        target: Target<A>,
    },
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
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

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (window, position, entries);
        Ok(None)
    }
}
