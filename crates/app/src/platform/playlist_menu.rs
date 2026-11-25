//! Platform presentation for a playlist menu whose tree is supplied at runtime.
use crate::PlayerWindow;
use slint::ComponentHandle;
use slint::LogicalPosition;

mod client;
#[cfg(any(target_os = "linux", test))]
mod player_bar;
#[cfg(target_os = "linux")]
pub(super) use player_bar::new_player;
#[cfg(target_os = "linux")]
mod x11;

// AppKit's playlist file panel accepts folders in the same selection.
pub(crate) const MIXED_PLAYLIST_PICKER: bool = cfg!(target_os = "macos");

#[derive(Clone)]
pub(crate) enum Entry<A> {
    Separator,
    Item {
        title: String,
        shortcut: String,
        enabled: bool,
        checkable: bool,
        checked: bool,
        target: Target<A>,
    },
}

#[derive(Clone)]
pub(crate) enum Target<A> {
    Action(A),
    Submenu(Vec<Entry<A>>),
}

pub(crate) fn dismiss(app: &PlayerWindow) {
    client::dismiss(app);
    #[cfg(target_os = "linux")]
    x11::dismiss(app.window());
}

pub(crate) fn popup<A: Clone + 'static>(
    app: &PlayerWindow,
    position: LogicalPosition,
    entries: Vec<Entry<A>>,
    selected: impl FnOnce(A) + 'static,
) {
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    let window = app.window();
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        use slint::winit_030::WinitWindowAccessor;

        if window.has_winit_window() {
            #[cfg(target_os = "windows")]
            let native = super::windows::playlist_menu::popup;
            #[cfg(target_os = "macos")]
            let native = super::macos::playlist_menu::popup;
            match native(window, position, entries.clone()) {
                Ok(action) => {
                    if let Some(action) = action {
                        selected(action);
                    }
                    return;
                }
                Err(error) => {
                    crate::diagnostics::warn!("Could not present native playlist menu: {error}")
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    if x11::owner(window).is_some() {
        use std::{cell::Cell, rc::Rc};

        let callback = Rc::new(Cell::new(Some(selected)));
        let native = callback.clone();
        if let Err(error) = x11::popup(window, position, entries.clone(), move |action| {
            if let Some(selected) = native.take() {
                selected(action);
            }
        }) {
            crate::diagnostics::warn!("Could not present X11 playlist menu: {error}");
            if let Some(selected) = callback.take() {
                client::show(app, position, entries, selected);
            }
        }
        return;
    }
    client::show(app, position, entries, selected);
}
