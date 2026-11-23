//! Platform presentation for a playlist menu whose tree is supplied at runtime.
use crate::PlayerWindow;
use slint::ComponentHandle;
use slint::{LogicalPosition, Window};

#[cfg(any(target_os = "linux", test))]
mod client;
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
    return is_wayland(window) || x11::owner(window).is_some();

    #[cfg(not(target_os = "linux"))]
    {
        let _ = window;
        cfg!(any(target_os = "windows", target_os = "macos"))
    }
}

pub(crate) fn dismiss(app: &PlayerWindow) {
    #[cfg(target_os = "linux")]
    {
        client::dismiss(app);
        x11::dismiss(app.window());
    }
    #[cfg(not(target_os = "linux"))]
    let _ = app;
}

pub(crate) fn popup<A: 'static>(
    app: &PlayerWindow,
    position: LogicalPosition,
    entries: Vec<Entry<A>>,
    selected: impl FnOnce(A) + 'static,
) -> Result<(), String> {
    let window = app.window();
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
    {
        if is_wayland(window) {
            client::show(app, position, entries, selected);
            return Ok(());
        }
        x11::popup(window, position, entries, selected)
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = (window, position, entries, selected);
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn is_wayland(window: &Window) -> bool {
    use slint::winit_030::{
        WinitWindowAccessor,
        winit::raw_window_handle::{HasWindowHandle, RawWindowHandle},
    };

    window
        .with_winit_window(|window| {
            window
                .window_handle()
                .is_ok_and(|handle| matches!(handle.as_raw(), RawWindowHandle::Wayland(_)))
        })
        .unwrap_or_default()
}
