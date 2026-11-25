//! Settle a Music Mode resize when the desktop stops sending configure events.
use crate::{PlayerWindow, music_mode_layout};
use riina_core::window_aspect::PixelSize;
use riina_ui::PlayerMusicModeState;
use slint::ComponentHandle;
use std::time::Duration;

pub(in crate::platform) fn observe(
    app: &PlayerWindow,
    window: &slint::Window,
    requested: PixelSize,
    timer: &slint::Timer,
) {
    // The desktop can repeat the windowed size after a fullscreen exit.
    // Compare with the compact target even if the last event had the same size.
    let scale = f64::from(window.scale_factor());
    let settled = music_mode_layout::compact_size(app, scale);
    if !music_mode_layout::can_resize(app)
        || window.is_maximized()
        || (requested.width, requested.height) == (settled.width, settled.height)
    {
        timer.stop();
        app.global::<PlayerMusicModeState>()
            .set_music_mode_live_catalog_height(-1.0);
        return;
    }
    music_mode_layout::preview_resize(app, requested.height, scale);
    let weak = app.as_weak();
    timer.start(
        slint::TimerMode::SingleShot,
        Duration::from_millis(200),
        move || {
            if let Some(app) = weak.upgrade() {
                app.invoke_music_mode_resize_finished();
            }
        },
    );
}
