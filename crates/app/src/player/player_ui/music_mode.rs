//! Integrate Music Mode commands and compact window sizing with the player.
use crate::{PlayerWindow, music_mode_layout, window_aspect};
use riina_core::window_aspect::PixelSize;
use slint::ComponentHandle;
use std::cell::Cell;

pub(super) fn install_window_resize(app: &PlayerWindow) {
    let resize_owner = app.as_weak();
    app.on_music_mode_resize_finished(move || {
        if let Some(app) = resize_owner.upgrade() {
            music_mode_layout::finish_resize(&app, app.window());
        }
    });
    let saved_frame = Cell::new(None::<music_mode_layout::MainWindowFrame>);
    let weak = app.as_weak();
    app.on_music_mode_changed(move || {
        if let Some(app) = weak.upgrade() {
            let window = app.window();
            if app
                .global::<riina_ui::PlayerMusicModeState>()
                .get_music_mode()
            {
                let entering = saved_frame.get().is_none();
                if entering {
                    saved_frame.set(Some(music_mode_layout::MainWindowFrame::leave(window)));
                }
                music_mode_layout::apply_compact_size(&app, window, None, entering);
                return;
            }
            let Some(frame) = saved_frame.take() else {
                return;
            };
            music_mode_layout::restore_window_minimum(window);
            let current = music_mode_layout::client_size(window);
            window_aspect::resize_centered(
                window,
                PixelSize {
                    width: current.width,
                    height: current.height,
                },
                PixelSize {
                    width: frame.size.width,
                    height: frame.size.height,
                },
                None,
            );
            frame.restore_placement(window);
        }
    });
}
