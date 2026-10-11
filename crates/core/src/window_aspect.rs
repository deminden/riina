//! Portable window sizing policy for aspect-locked playback.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelSize {
    pub width: u32,
    pub height: u32,
}

impl PixelSize {
    pub(crate) fn valid(self) -> bool {
        self.width > 0 && self.height > 0
    }
}

/// Correct the dimension the user did not drag, preserving the video ratio.
pub fn constrain_resize(
    previous: PixelSize,
    requested: PixelSize,
    aspect: f64,
    maximum: Option<PixelSize>,
) -> Option<PixelSize> {
    if !previous.valid() || !requested.valid() || !valid_aspect(aspect) {
        return None;
    }
    let width_delta = requested.width.abs_diff(previous.width);
    let height_delta = requested.height.abs_diff(previous.height);
    let width_is_authoritative = if width_delta == 0 && height_delta == 0 {
        let keep_width = (f64::from(requested.width) / aspect - f64::from(requested.height)).abs();
        let keep_height = (f64::from(requested.height) * aspect - f64::from(requested.width)).abs();
        keep_width <= keep_height
    } else {
        width_delta >= height_delta
    };
    let size = if width_is_authoritative {
        PixelSize {
            width: requested.width,
            height: pixels(f64::from(requested.width) / aspect),
        }
    } else {
        PixelSize {
            width: pixels(f64::from(requested.height) * aspect),
            height: requested.height,
        }
    };
    Some(limit(size, aspect, maximum))
}

/// Grow the client area just enough to remove bars around the displayed video.
pub fn fit_to_video(
    current: PixelSize,
    aspect: f64,
    maximum: Option<PixelSize>,
) -> Option<PixelSize> {
    if !current.valid() || !valid_aspect(aspect) {
        return None;
    }
    let current_aspect = f64::from(current.width) / f64::from(current.height);
    let size = if (current_aspect - aspect).abs() < 0.001 {
        current
    } else if current_aspect > aspect {
        PixelSize {
            width: current.width,
            height: pixels(f64::from(current.width) / aspect),
        }
    } else {
        PixelSize {
            width: pixels(f64::from(current.height) * aspect),
            height: current.height,
        }
    };
    Some(limit(size, aspect, maximum))
}

pub fn display_aspect_ratio(
    width: u32,
    height: u32,
    display_width: Option<u32>,
    display_height: Option<u32>,
    rotation: Option<i64>,
    aspect_override: Option<f64>,
) -> Option<f64> {
    let aspect = if let Some(aspect) = aspect_override.filter(|value| valid_aspect(*value)) {
        aspect
    } else {
        let width = f64::from(display_width.filter(|value| *value > 0).unwrap_or(width));
        let height = f64::from(display_height.filter(|value| *value > 0).unwrap_or(height));
        if width <= 0.0 || height <= 0.0 {
            return None;
        }
        width / height
    };
    let aspect = if rotation.is_some_and(|degrees| degrees.rem_euclid(180) == 90) {
        1.0 / aspect
    } else {
        aspect
    };
    valid_aspect(aspect).then_some(aspect)
}

fn limit(size: PixelSize, aspect: f64, maximum: Option<PixelSize>) -> PixelSize {
    let Some(maximum) = maximum.filter(|maximum| maximum.valid()) else {
        return size;
    };
    let scale = (f64::from(maximum.width) / f64::from(size.width))
        .min(f64::from(maximum.height) / f64::from(size.height))
        .min(1.0);
    if scale == 1.0 {
        size
    } else {
        let width = pixels(f64::from(size.width) * scale);
        let height = pixels(f64::from(width) / aspect);
        if height <= maximum.height {
            PixelSize { width, height }
        } else {
            let height = maximum.height;
            PixelSize {
                width: pixels(f64::from(height) * aspect),
                height,
            }
        }
    }
}

pub(crate) fn valid_aspect(aspect: f64) -> bool {
    aspect.is_finite() && aspect > 0.0
}

fn pixels(value: f64) -> u32 {
    value.round().clamp(1.0, f64::from(u32::MAX)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_follows_the_dominant_edge_and_keeps_the_video_ratio() {
        let previous = PixelSize {
            width: 960,
            height: 540,
        };
        assert_eq!(
            constrain_resize(
                previous,
                PixelSize {
                    width: 1200,
                    height: 560,
                },
                16.0 / 9.0,
                None,
            ),
            Some(PixelSize {
                width: 1200,
                height: 675,
            })
        );
        assert_eq!(
            constrain_resize(
                previous,
                PixelSize {
                    width: 980,
                    height: 700,
                },
                16.0 / 9.0,
                None,
            ),
            Some(PixelSize {
                width: 1244,
                height: 700,
            })
        );
    }

    #[test]
    fn fitting_grows_the_short_dimension_and_respects_monitor_bounds() {
        assert_eq!(
            fit_to_video(
                PixelSize {
                    width: 800,
                    height: 600,
                },
                16.0 / 9.0,
                None,
            ),
            Some(PixelSize {
                width: 1067,
                height: 600,
            })
        );
        let fitted = fit_to_video(
            PixelSize {
                width: 800,
                height: 600,
            },
            16.0 / 9.0,
            Some(PixelSize {
                width: 1000,
                height: 700,
            }),
        )
        .unwrap();
        assert!(fitted.width <= 1000 && fitted.height <= 700);
        assert!((f64::from(fitted.width) / f64::from(fitted.height) - 16.0 / 9.0).abs() < 0.002);
    }

    #[test]
    fn display_aspect_uses_pixel_shape_rotation_and_user_override() {
        assert_eq!(
            display_aspect_ratio(1440, 1080, Some(1920), Some(1080), Some(0), None),
            Some(16.0 / 9.0)
        );
        assert_eq!(
            display_aspect_ratio(1920, 1080, None, None, Some(90), None),
            Some(0.5625)
        );
        assert_eq!(
            display_aspect_ratio(1920, 1080, None, None, Some(0), Some(4.0 / 3.0)),
            Some(4.0 / 3.0)
        );
        assert_eq!(
            display_aspect_ratio(1920, 1080, None, None, Some(90), Some(4.0 / 3.0)),
            Some(0.75)
        );
        assert_eq!(display_aspect_ratio(0, 0, None, None, None, None), None);
        assert_eq!(
            display_aspect_ratio(1920, 1080, None, None, None, Some(f64::NAN)),
            Some(16.0 / 9.0)
        );
    }
}
