use crate::{
    PlayerWindow,
    platform::window_sizing::{self, ResizeAnchor},
    window_aspect,
};
use riina_core::window_aspect::PixelSize;
use riina_ui::PlayerMusicModeState;
use slint::{
    ComponentHandle, PhysicalSize, Window,
    winit_030::{WinitWindowAccessor, winit::dpi::LogicalSize},
};

pub(crate) const WINDOW_WIDTH: f64 = 300.0;
pub(crate) const COMPACT_CONTROL_HEIGHT: f64 = 90.0;
pub(crate) const DEFAULT_CATALOG_HEIGHT: f32 = 300.0;
const MIN_CATALOG_HEIGHT: f64 = 200.0;

#[derive(Clone, Copy)]
pub(crate) struct MainWindowFrame {
    pub(crate) size: slint::PhysicalSize,
    position: slint::PhysicalPosition,
    maximized: bool,
}

impl MainWindowFrame {
    pub(crate) fn leave(window: &slint::Window) -> Self {
        let maximized = window.is_maximized();
        if maximized {
            window.set_maximized(false);
        }
        Self {
            size: client_size(window),
            position: window.position(),
            maximized,
        }
    }

    pub(crate) fn restore_placement(self, window: &slint::Window) {
        let origin_is_visible = window
            .with_winit_window(|native| {
                native.available_monitors().any(|monitor| {
                    let origin = monitor.position();
                    let size = monitor.size();
                    let x = i64::from(self.position.x) - i64::from(origin.x);
                    let y = i64::from(self.position.y) - i64::from(origin.y);
                    (0..i64::from(size.width)).contains(&x)
                        && (0..i64::from(size.height)).contains(&y)
                })
            })
            .unwrap_or(true);
        if origin_is_visible {
            window.set_position(self.position);
        }
        if self.maximized {
            window.set_maximized(true);
        }
    }
}

pub(crate) fn client_size(window: &slint::Window) -> slint::PhysicalSize {
    // The fullscreen exit can complete before Slint consumes its resize event.
    window
        .with_winit_window(|native| {
            let size = native.inner_size();
            slint::PhysicalSize::new(size.width, size.height)
        })
        .unwrap_or_else(|| window.size())
}

pub(crate) fn compact_size(app: &PlayerWindow, scale: f64) -> PhysicalSize {
    let mode = app.global::<PlayerMusicModeState>();
    let height = base_height(app)
        + if mode.get_music_mode_show_playlist() {
            f64::from(mode.get_music_mode_catalog_height())
        } else {
            0.0
        };
    PhysicalSize::new(physical(WINDOW_WIDTH, scale), physical(height, scale))
}

pub(crate) fn base_height(app: &PlayerWindow) -> f64 {
    let mode = app.global::<PlayerMusicModeState>();
    normal_height(
        mode.get_music_mode_show_album_art(),
        f64::from(mode.get_music_mode_artwork_aspect_ratio()),
    )
}

fn normal_height(show_artwork: bool, aspect_ratio: f64) -> f64 {
    COMPACT_CONTROL_HEIGHT
        + if show_artwork {
            artwork_height(aspect_ratio)
        } else {
            0.0
        }
}

pub(crate) fn physical(points: f64, scale: f64) -> u32 {
    (points * scale).round() as u32
}

pub(crate) fn can_resize(app: &PlayerWindow) -> bool {
    app.get_loaded()
        && !app.get_fullscreen()
        && app.global::<PlayerMusicModeState>().get_music_mode()
}

pub(crate) fn preview_resize(app: &PlayerWindow, height: u32, scale: f64) {
    let additional = (f64::from(height) / scale - base_height(app)).max(0.0);
    app.global::<PlayerMusicModeState>()
        .set_music_mode_live_catalog_height(additional as f32);
}

fn settled_catalog_height(height: u32, base: f64, scale: f64) -> Option<f32> {
    // Compare the physical threshold directly so fractional display scales
    // cannot round a drag at the threshold to the hidden side.
    (height >= physical(base + MIN_CATALOG_HEIGHT, scale))
        .then(|| (f64::from(height) / scale - base) as f32)
}

pub(crate) fn finish_resize(app: &PlayerWindow, window: &Window) {
    let mode = app.global::<PlayerMusicModeState>();
    if mode.get_music_mode_live_catalog_height() < 0.0 {
        return;
    }
    mode.set_music_mode_live_catalog_height(-1.0);
    if !can_resize(app) || window.is_maximized() {
        return;
    }
    let scale = f64::from(window.scale_factor());
    let catalog = settled_catalog_height(client_size(window).height, base_height(app), scale);
    let shown = catalog.is_some();
    mode.set_music_mode_catalog_height(catalog.unwrap_or(DEFAULT_CATALOG_HEIGHT));
    mode.set_music_mode_show_playlist(shown);
    // The panel can keep its visibility while the window still needs a snap.
    app.invoke_music_mode_changed();
}

pub(crate) fn apply_compact_size(
    app: &PlayerWindow,
    window: &Window,
    overlay: Option<&Window>,
    entering: bool,
) {
    window.with_winit_window(|native| {
        native.set_min_inner_size(Some(LogicalSize::new(WINDOW_WIDTH, base_height(app))));
    });
    let mode = app.global::<PlayerMusicModeState>();
    if mode.get_music_mode_live_catalog_height() >= 0.0 {
        return;
    }
    let target = compact_size(app, f64::from(window.scale_factor()));
    let current = client_size(window);
    if current == target {
        return;
    }
    let size = PixelSize {
        width: target.width,
        height: target.height,
    };
    if entering {
        let before = PixelSize {
            width: current.width,
            height: current.height,
        };
        window_aspect::resize_centered(window, before, size, overlay);
    } else if !window_sizing::resize_native_content(window, size, ResizeAnchor::TopLeft, overlay)
        .is_some_and(|result| result.is_ok())
    {
        // Adjustments to the drawer grow down from the current top edge.
        window.set_size(target);
        // A paused Wayland surface needs a frame to commit the new geometry.
        window.request_redraw();
    }
}

pub(crate) fn restore_window_minimum(window: &Window) {
    window.with_winit_window(|native| {
        native.set_min_inner_size(Some(LogicalSize::new(285.0, 120.0)));
    });
}

fn artwork_height(aspect_ratio: f64) -> f64 {
    if aspect_ratio.is_finite() && aspect_ratio > 0.0 {
        (WINDOW_WIDTH / aspect_ratio).round().max(1.0)
    } else {
        WINDOW_WIDTH
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_art_and_playlist_fit_the_compact_window() {
        assert_eq!(
            normal_height(true, 1.0) + f64::from(DEFAULT_CATALOG_HEIGHT),
            690.0
        );
    }

    #[test]
    fn portrait_and_wide_media_keep_their_displayed_ratio() {
        assert_eq!(normal_height(true, 0.5), 690.0);
        assert_eq!(normal_height(true, 2.0), 240.0);
    }

    #[test]
    fn hidden_art_and_unknown_dimensions_do_not_create_empty_space() {
        assert_eq!(normal_height(false, 1.0), 90.0);
        assert_eq!(normal_height(true, 0.0), 390.0);
        assert_eq!(normal_height(true, f64::NAN), 390.0);
    }

    #[test]
    fn compact_drawer_uses_the_native_pixel_threshold_and_keeps_the_dragged_height() {
        for scale in [1.0, 1.25, 1.5, 3.0] {
            for base in [90.0, normal_height(true, 16.0 / 9.0)] {
                let threshold = physical(base + MIN_CATALOG_HEIGHT, scale);
                assert_eq!(settled_catalog_height(threshold - 1, base, scale), None);
                let opened = settled_catalog_height(threshold, base, scale).unwrap();
                assert_eq!(physical(base + f64::from(opened), scale), threshold);
                let dragged = threshold + 95;
                let opened = settled_catalog_height(dragged, base, scale).unwrap();
                assert_eq!(physical(base + f64::from(opened), scale), dragged);
            }
        }
    }
}
