//! Integrate Music Mode commands and compact window sizing with the player.
use crate::{PlayerWindow, music_mode_layout, window_aspect};
use riina_core::{music_mode::State, sync::lock, window_aspect::PixelSize};
use slint::ComponentHandle;
use std::{
    cell::Cell,
    sync::{Arc, Mutex},
};

pub(super) fn install_window_resize(app: &PlayerWindow) {
    let saved_size = Cell::new(None::<slint::PhysicalSize>);
    let weak = app.as_weak();
    app.on_music_mode_changed(move || {
        if let Some(app) = weak.upgrade() {
            let window = app.window();
            if app.get_music_mode() {
                if saved_size.get().is_none() {
                    saved_size.set(Some(window.size()));
                }
                let scale = f64::from(window.scale_factor());
                let height = music_mode_layout::window_height(
                    app.get_music_mode_show_album_art(),
                    app.get_music_mode_show_playlist(),
                    f64::from(app.get_music_mode_artwork_aspect_ratio()),
                );
                let target = slint::PhysicalSize::new(
                    (music_mode_layout::WINDOW_WIDTH * scale).round() as u32,
                    (height * scale).round() as u32,
                );
                window_aspect::resize_centered(
                    window,
                    PixelSize {
                        width: window.size().width,
                        height: window.size().height,
                    },
                    PixelSize {
                        width: target.width,
                        height: target.height,
                    },
                    None,
                );
            } else if let Some(size) = saved_size.take() {
                window_aspect::resize_centered(
                    window,
                    PixelSize {
                        width: window.size().width,
                        height: window.size().height,
                    },
                    PixelSize {
                        width: size.width,
                        height: size.height,
                    },
                    None,
                );
            }
        }
    });
}

pub(super) fn install_actions(app: &PlayerWindow, state: &Arc<Mutex<State>>) {
    let weak = app.as_weak();
    let manual_music_mode = state.clone();
    app.on_toggle_music_mode(move || {
        if let Some(app) = weak.upgrade() {
            let active = lock(&manual_music_mode).toggle_manually();
            if active && app.get_fullscreen() {
                app.invoke_toggle_fullscreen();
            }
            app.set_music_mode(active);
        }
    });
    let automatic_music_mode = state.clone();
    app.on_auto_switch_music_mode_changed(move |enabled| {
        lock(&automatic_music_mode).set_auto_switch(enabled);
    });
}
