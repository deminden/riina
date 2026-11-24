//! Integrate Music Mode commands and compact window sizing with the player.
use crate::{PlayerWindow, music_mode_layout, window_aspect};
use riina_core::{music_mode::State, sync::lock, window_aspect::PixelSize};
use slint::ComponentHandle;
use std::{cell::Cell, sync::Mutex};

pub(super) fn install_window_resize(app: &PlayerWindow) {
    let saved_frame = Cell::new(None::<music_mode_layout::MainWindowFrame>);
    let weak = app.as_weak();
    app.on_music_mode_changed(move || {
        if let Some(app) = weak.upgrade() {
            let window = app.window();
            let (target, restore) = if app
                .global::<riina_ui::PlayerMusicModeState>()
                .get_music_mode()
            {
                if saved_frame.get().is_none() {
                    saved_frame.set(Some(music_mode_layout::MainWindowFrame::leave(window)));
                }
                (
                    music_mode_layout::compact_size(&app, f64::from(window.scale_factor())),
                    None,
                )
            } else if let Some(frame) = saved_frame.take() {
                (frame.size, Some(frame))
            } else {
                return;
            };
            let current = music_mode_layout::client_size(window);
            window_aspect::resize_centered(
                window,
                PixelSize {
                    width: current.width,
                    height: current.height,
                },
                PixelSize {
                    width: target.width,
                    height: target.height,
                },
                None,
            );
            if let Some(frame) = restore {
                frame.restore_placement(window);
            }
        }
    });
}

pub(super) fn toggle_manually(app: &PlayerWindow, state: &Mutex<State>) {
    let active = lock(state).toggle_manually();
    crate::player::presentation::set_music_mode(app, active);
}
