use slint::winit_030::WinitWindowAccessor;

pub(crate) const WINDOW_WIDTH: f64 = 300.0;
pub(crate) const COMPACT_CONTROL_HEIGHT: f64 = 90.0;
pub(crate) const PLAYLIST_HEIGHT: f64 = 300.0;

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

pub(crate) fn compact_size(app: &crate::PlayerWindow, scale: f64) -> slint::PhysicalSize {
    use slint::ComponentHandle;
    let mode = app.global::<riina_ui::PlayerMusicModeState>();
    let height = window_height(
        mode.get_music_mode_show_album_art(),
        mode.get_music_mode_show_playlist(),
        f64::from(mode.get_music_mode_artwork_aspect_ratio()),
    );
    slint::PhysicalSize::new(
        (WINDOW_WIDTH * scale).round() as u32,
        (height * scale).round() as u32,
    )
}

pub(crate) fn window_height(show_artwork: bool, show_playlist: bool, aspect_ratio: f64) -> f64 {
    COMPACT_CONTROL_HEIGHT
        + if show_artwork {
            artwork_height(aspect_ratio)
        } else {
            0.0
        }
        + if show_playlist { PLAYLIST_HEIGHT } else { 0.0 }
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
        assert_eq!(window_height(true, true, 1.0), 690.0);
    }

    #[test]
    fn portrait_and_wide_media_keep_their_displayed_ratio() {
        assert_eq!(window_height(true, false, 0.5), 690.0);
        assert_eq!(window_height(true, false, 2.0), 240.0);
    }

    #[test]
    fn hidden_art_and_unknown_dimensions_do_not_create_empty_space() {
        assert_eq!(window_height(false, false, 1.0), 90.0);
        assert_eq!(window_height(true, false, 0.0), 390.0);
        assert_eq!(window_height(true, false, f64::NAN), 390.0);
    }
}
