pub(crate) const WINDOW_WIDTH: f64 = 300.0;
pub(crate) const COMPACT_CONTROL_HEIGHT: f64 = 90.0;
pub(crate) const PLAYLIST_HEIGHT: f64 = 300.0;

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
