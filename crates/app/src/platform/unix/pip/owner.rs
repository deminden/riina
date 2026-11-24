//! Window operations supported by the Linux desktop's actual window protocol.

use crate::{PictureInPictureWindow, PlayerWindow};
use riina_core::pip_preferences::WindowBehavior;
use slint::{
    ComponentHandle,
    winit_030::{
        WinitWindowAccessor,
        winit::{
            raw_window_handle::{HasWindowHandle, RawWindowHandle},
            window::WindowLevel,
        },
    },
};

pub(super) struct OwnerState {
    pub(super) wayland: bool,
    pub(super) minimized: Option<bool>,
}

#[derive(Clone, Copy)]
pub(super) enum OwnerAction {
    Kept,
    Hidden,
    Minimized,
}

impl OwnerState {
    pub(super) fn is_wayland(player: &PlayerWindow) -> bool {
        player
            .window()
            .with_winit_window(|window| {
                window
                    .window_handle()
                    .is_ok_and(|handle| matches!(handle.as_raw(), RawWindowHandle::Wayland(_)))
            })
            .unwrap_or(false)
    }

    pub(super) fn read(player: &PlayerWindow) -> Self {
        let wayland = Self::is_wayland(player);
        let minimized = player
            .window()
            .with_winit_window(|window| window.is_minimized())
            .flatten();
        Self { wayland, minimized }
    }

    pub(super) fn apply(
        &self,
        player: &PlayerWindow,
        behavior: WindowBehavior,
        can_draw_offscreen: bool,
    ) -> OwnerAction {
        // xdg-shell does not let the client restore a minimized toplevel
        // or hide it while keeping the decoder's native graphics surface.
        if self.wayland || !can_draw_offscreen {
            return OwnerAction::Kept;
        }
        player
            .window()
            .with_winit_window(|window| match behavior {
                WindowBehavior::Hide => {
                    window.set_visible(false);
                    OwnerAction::Hidden
                }
                WindowBehavior::Minimize => {
                    window.set_minimized(true);
                    OwnerAction::Minimized
                }
                WindowBehavior::DoNothing => OwnerAction::Kept,
            })
            .unwrap_or(OwnerAction::Kept)
    }
}

impl OwnerAction {
    pub(super) fn restore(self, player: &PlayerWindow) {
        player.window().with_winit_window(|window| {
            match self {
                Self::Hidden => window.set_visible(true),
                Self::Minimized => window.set_minimized(false),
                Self::Kept => {}
            }
            window.request_redraw();
            window.focus_window();
        });
    }
}

pub(super) fn show_preview(preview: &PictureInPictureWindow) -> Result<(), String> {
    crate::platform::show_window(preview).map_err(|error| error.to_string())?;
    let weak = preview.as_weak();
    slint::spawn_local(async move {
        if let Some(preview) = weak.upgrade()
            && let Ok(window) = preview.window().winit_window().await
            && preview.window().is_visible()
        {
            window.set_window_level(WindowLevel::AlwaysOnTop);
            window.request_redraw();
        }
    })
    .map_err(|error| error.to_string())?;
    Ok(())
}
