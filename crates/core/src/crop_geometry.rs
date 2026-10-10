//! Reversible source-pixel projection for the interactive crop surface.
use crate::{crop::Rectangle, video_controls::Rotation};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// Continuous editing coordinates. Integer conversion belongs at the label /
/// native-command boundary, not in pointer movement or viewport reprojection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl From<Rectangle> for SourceRect {
    fn from(rectangle: Rectangle) -> Self {
        Self {
            x: f64::from(rectangle.x),
            y: f64::from(rectangle.y),
            width: f64::from(rectangle.width),
            height: f64::from(rectangle.height),
        }
    }
}

impl SourceRect {
    fn validate(self, source: (u32, u32)) -> Result<(), String> {
        if ![self.x, self.y, self.width, self.height]
            .into_iter()
            .all(f64::is_finite)
            || self.x < 0.0
            || self.y < 0.0
            || self.width < 0.0
            || self.height < 0.0
            || self.x + self.width > f64::from(source.0)
            || self.y + self.height > f64::from(source.1)
        {
            return Err("Crop rectangle is outside the source".into());
        }
        Ok(())
    }

    pub fn to_native(self, source: (u32, u32)) -> Result<Rectangle, String> {
        self.validate(source)?;
        Ok(Rectangle {
            x: truncate(self.x, source.0),
            y: truncate(self.y, source.1),
            width: truncate(self.width, source.0),
            height: truncate(self.height, source.1),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisplayRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct Projection {
    source_width: u32,
    source_height: u32,
    rotation: Rotation,
    content: DisplayRect,
}

impl Projection {
    /// Interactive mode reserves space for the bottom controls and anchors the
    /// contained picture at the top inset, rather than centering it vertically.
    pub fn interactive(video: &crate::VideoParameters, size: (f64, f64)) -> Result<Self, String> {
        let mut projection = Self::for_video(
            video,
            DisplayRect {
                x: 20.0,
                y: 20.0,
                width: size.0 - 40.0,
                height: size.1 - 108.0,
            },
        )?;
        projection.content.y = 20.0;
        Ok(projection)
    }

    pub fn for_video(
        video: &crate::VideoParameters,
        viewport: DisplayRect,
    ) -> Result<Self, String> {
        let display = video
            .dw
            .zip(video.dh)
            .ok_or("Video display dimensions are unavailable")?;
        let degrees = video
            .rotate
            .ok_or("Video display rotation is unavailable")?
            .rem_euclid(360);
        let rotation = Rotation::from_degrees(degrees)
            .ok_or("This display rotation is not supported by the crop projection")?;
        Self::contain(
            (video.w, video.h),
            (f64::from(display.0), f64::from(display.1)),
            rotation,
            viewport,
        )
    }
    /// Display dimensions are unrotated and include pixel/display aspect.
    /// Viewport and returned rectangles use logical units with a top-left origin.
    pub fn contain(
        source: (u32, u32),
        display: (f64, f64),
        rotation: Rotation,
        viewport: DisplayRect,
    ) -> Result<Self, String> {
        if source.0 == 0
            || source.1 == 0
            || ![
                display.0,
                display.1,
                viewport.x,
                viewport.y,
                viewport.width,
                viewport.height,
            ]
            .into_iter()
            .all(f64::is_finite)
            || display.0 <= 0.0
            || display.1 <= 0.0
            || viewport.width <= 0.0
            || viewport.height <= 0.0
        {
            return Err("Crop projection dimensions are invalid".into());
        }
        let (dw, dh) = if matches!(rotation, Rotation::Clockwise90 | Rotation::Clockwise270) {
            (display.1, display.0)
        } else {
            display
        };
        let scale = (viewport.width / dw).min(viewport.height / dh);
        let width = dw * scale;
        let height = dh * scale;
        let content = DisplayRect {
            x: viewport.x + (viewport.width - width) / 2.0,
            y: viewport.y + (viewport.height - height) / 2.0,
            width,
            height,
        };
        if ![
            content.x,
            content.y,
            content.width,
            content.height,
            content.x + content.width,
            content.y + content.height,
        ]
        .into_iter()
        .all(f64::is_finite)
            || width <= 0.0
            || height <= 0.0
        {
            return Err("Crop projection exceeds the numeric range".into());
        }
        Ok(Self {
            source_width: source.0,
            source_height: source.1,
            rotation,
            content,
        })
    }

    pub fn content(&self) -> DisplayRect {
        self.content
    }

    pub fn project(&self, rectangle: Rectangle) -> Result<DisplayRect, String> {
        self.project_continuous(rectangle.into())
    }

    pub fn project_continuous(&self, rectangle: SourceRect) -> Result<DisplayRect, String> {
        rectangle.validate((self.source_width, self.source_height))?;
        let left = rectangle.x / f64::from(self.source_width);
        let top = rectangle.y / f64::from(self.source_height);
        let right = (rectangle.x + rectangle.width) / f64::from(self.source_width);
        let bottom = (rectangle.y + rectangle.height) / f64::from(self.source_height);
        let rotated = bounds(
            [
                Point { x: left, y: top },
                Point { x: right, y: top },
                Point { x: left, y: bottom },
                Point {
                    x: right,
                    y: bottom,
                },
            ]
            .map(|point| self.rotate(point, false)),
        );
        Ok(DisplayRect {
            x: self.content.x + rotated.x * self.content.width,
            y: self.content.y + rotated.y * self.content.height,
            width: rotated.width * self.content.width,
            height: rotated.height * self.content.height,
        })
    }

    pub fn select(&self, start: Point, end: Point) -> Result<Rectangle, String> {
        let clamp = |point: Point| Point {
            x: point
                .x
                .clamp(self.content.x, self.content.x + self.content.width),
            y: point
                .y
                .clamp(self.content.y, self.content.y + self.content.height),
        };
        let (start, end) = (clamp(start), clamp(end));
        if start.x == end.x || start.y == end.y {
            return Err("Crop selection has no area".into());
        }
        let rectangle = self
            .select_continuous(start, end)?
            .to_native((self.source_width, self.source_height))?;
        if rectangle.width == 0 || rectangle.height == 0 {
            return Err("Crop selection has no area".into());
        }
        Ok(rectangle)
    }

    pub fn select_continuous(&self, start: Point, end: Point) -> Result<SourceRect, String> {
        if ![start.x, start.y, end.x, end.y]
            .into_iter()
            .all(f64::is_finite)
        {
            return Err("Crop pointer position is invalid".into());
        }
        let source_point = |point: Point| {
            self.rotate(
                Point {
                    x: ((point.x - self.content.x) / self.content.width).clamp(0.0, 1.0),
                    y: ((point.y - self.content.y) / self.content.height).clamp(0.0, 1.0),
                },
                true,
            )
        };
        let selected = bounds([
            source_point(start),
            source_point(end),
            source_point(Point {
                x: start.x,
                y: end.y,
            }),
            source_point(Point {
                x: end.x,
                y: start.y,
            }),
        ]);
        let snap = |value: f64, size: u32, far: bool| {
            if far && f64::from(size) - value <= 4.0 {
                f64::from(size)
            } else if value <= 4.0 {
                0.0
            } else if f64::from(size) - value <= 4.0 {
                f64::from(size)
            } else {
                value
            }
        };
        let left = snap(
            selected.x * f64::from(self.source_width),
            self.source_width,
            false,
        );
        let top = snap(
            selected.y * f64::from(self.source_height),
            self.source_height,
            false,
        );
        let right = snap(
            (selected.x + selected.width) * f64::from(self.source_width),
            self.source_width,
            true,
        );
        let bottom = snap(
            (selected.y + selected.height) * f64::from(self.source_height),
            self.source_height,
            true,
        );
        let rectangle = SourceRect {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        };
        Ok(rectangle)
    }

    fn rotate(&self, point: Point, inverse: bool) -> Point {
        let degrees = if inverse {
            (360 - self.rotation.degrees()) % 360
        } else {
            self.rotation.degrees()
        };
        let (x, y) = match degrees {
            90 => (1.0 - point.y, point.x),
            180 => (1.0 - point.x, 1.0 - point.y),
            270 => (point.y, 1.0 - point.x),
            _ => (point.x, point.y),
        };
        Point { x, y }
    }
}

fn truncate(value: f64, extent: u32) -> u32 {
    // Round only machine-error distance from an integer. Ordinary fractional
    // source pixels still truncate, so resize cycles do not lose whole pixels.
    let nearest = value.round();
    let stable = if (nearest - value).abs() <= f64::EPSILON * f64::from(extent) * 64.0 {
        nearest
    } else {
        value.floor()
    };
    stable as u32
}

fn bounds(points: [Point; 4]) -> DisplayRect {
    let left = points
        .map(|point| point.x)
        .into_iter()
        .fold(f64::INFINITY, f64::min);
    let top = points
        .map(|point| point.y)
        .into_iter()
        .fold(f64::INFINITY, f64::min);
    let right = points
        .map(|point| point.x)
        .into_iter()
        .fold(f64::NEG_INFINITY, f64::max);
    let bottom = points
        .map(|point| point.y)
        .into_iter()
        .fold(f64::NEG_INFINITY, f64::max);
    DisplayRect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interactive_draft_retains_zero_extent_until_native_confirmation() {
        let projection =
            Projection::contain((320, 180), (320.0, 180.0), Rotation::Zero, viewport()).unwrap();
        let point = Point { x: 300.0, y: 300.0 };
        let empty = projection.select_continuous(point, point).unwrap();
        assert_eq!((empty.width, empty.height), (0.0, 0.0));
        let native = empty.to_native((320, 180)).unwrap();
        assert_eq!((native.width, native.height), (0, 0));
        assert!(native.editor_filter(320, 180).is_ok());
        assert!(native.filter(320, 180).is_err());
        let projected = projection.project_continuous(empty).unwrap();
        assert_eq!((projected.width, projected.height), (0.0, 0.0));
    }

    #[test]
    fn interactive_picture_is_top_anchored_and_reserves_controls() {
        let mut video: crate::VideoParameters =
            serde_json::from_str(r#"{"w":320,"h":180,"dw":320,"dh":180,"rotate":0}"#).unwrap();
        let projection = Projection::interactive(&video, (960.0, 600.0)).unwrap();
        let content = projection.content();
        assert!((content.width - 874.6666666666666).abs() < 1e-9);
        assert!((content.x - 42.6666666666667).abs() < 1e-9);
        assert_eq!(content.y, 20.0);
        assert_eq!(content.height, 492.0);
        let draft = SourceRect {
            x: 10.25,
            y: 12.5,
            width: 100.5,
            height: 60.25,
        };
        let box_rect = projection.project_continuous(draft).unwrap();
        let selected = projection
            .select_continuous(
                Point {
                    x: box_rect.x,
                    y: box_rect.y,
                },
                Point {
                    x: box_rect.x + box_rect.width,
                    y: box_rect.y + box_rect.height,
                },
            )
            .unwrap();
        assert!((selected.x - draft.x).abs() < 1e-9);
        assert!((selected.height - draft.height).abs() < 1e-9);
        video.rotate = Some(90);
        let rotated = Projection::interactive(&video, (960.0, 600.0))
            .unwrap()
            .content();
        assert_eq!(rotated.y, 20.0);
        assert_eq!(rotated.height, 492.0);
        assert!((rotated.width - 276.75).abs() < 1e-9);
        assert!(Projection::interactive(&video, (40.0, 600.0)).is_err());
        assert!(Projection::interactive(&video, (960.0, 108.0)).is_err());
        assert!(Projection::interactive(&video, (f64::NAN, 600.0)).is_err());
    }

    #[test]
    fn fractional_pointer_motion_moves_overlay_without_changing_integer_label() {
        let projection =
            Projection::contain((320, 180), (320.0, 180.0), Rotation::Zero, viewport()).unwrap();
        let start = Point { x: 30.6, y: 66.9 };
        let first = projection
            .select_continuous(start, Point { x: 330.9, y: 246.6 })
            .unwrap();
        let second = projection
            .select_continuous(
                start,
                Point {
                    x: 331.05,
                    y: 246.6,
                },
            )
            .unwrap();
        assert!((first.x - 10.2).abs() < 1e-9);
        assert!((first.y - 12.3).abs() < 1e-9);
        assert!((first.width - 100.1).abs() < 1e-9);
        assert!((first.height - 59.9).abs() < 1e-9);
        assert!((second.width - first.width - 0.05).abs() < 1e-9);
        let first_screen = projection.project_continuous(first).unwrap();
        let second_screen = projection.project_continuous(second).unwrap();
        assert!((second_screen.width - first_screen.width - 0.15).abs() < 1e-9);
        let native = Rectangle {
            x: 10,
            y: 12,
            width: 100,
            height: 59,
        };
        assert_eq!(first.to_native((320, 180)).unwrap(), native);
        assert_eq!(second.to_native((320, 180)).unwrap(), native);
    }

    #[test]
    fn continuous_draft_survives_rotated_resize_and_native_conversion() {
        let draft = SourceRect {
            x: 23.25,
            y: 17.75,
            width: 251.125,
            height: 137.5,
        };
        for degrees in [0, 90, 180, 270] {
            for size in [(960.0, 600.0), (321.5, 777.25)] {
                let projection = Projection::contain(
                    (320, 180),
                    (180.0, 180.0),
                    Rotation::from_degrees(degrees).unwrap(),
                    DisplayRect {
                        width: size.0,
                        height: size.1,
                        ..viewport()
                    },
                )
                .unwrap();
                let screen = projection.project_continuous(draft).unwrap();
                let restored = projection
                    .select_continuous(
                        Point {
                            x: screen.x,
                            y: screen.y,
                        },
                        Point {
                            x: screen.x + screen.width,
                            y: screen.y + screen.height,
                        },
                    )
                    .unwrap();
                assert!((restored.x - draft.x).abs() < 1e-9);
                assert!((restored.y - draft.y).abs() < 1e-9);
                assert!((restored.width - draft.width).abs() < 1e-9);
                assert!((restored.height - draft.height).abs() < 1e-9);
                assert_eq!(
                    restored.to_native((320, 180)).unwrap(),
                    Rectangle {
                        x: 23,
                        y: 17,
                        width: 251,
                        height: 137
                    }
                );
            }
        }
        assert!(
            SourceRect {
                width: f64::NAN,
                ..draft
            }
            .to_native((320, 180))
            .is_err()
        );
        assert!(
            SourceRect {
                width: 320.0,
                ..draft
            }
            .to_native((320, 180))
            .is_err()
        );
    }

    #[test]
    fn small_sources_keep_whole_selection_and_empty_drags_stay_empty() {
        let projection =
            Projection::contain((3, 2), (3.0, 2.0), Rotation::Zero, viewport()).unwrap();
        let screen = projection.content();
        assert_eq!(
            projection
                .select(
                    Point {
                        x: screen.x,
                        y: screen.y
                    },
                    Point {
                        x: screen.x + screen.width,
                        y: screen.y + screen.height
                    }
                )
                .unwrap(),
            Rectangle {
                x: 0,
                y: 0,
                width: 3,
                height: 2
            }
        );
        assert!(
            projection
                .select(Point { x: 500.0, y: 300.0 }, Point { x: 500.0, y: 300.0 })
                .is_err()
        );
    }

    #[test]
    fn observed_projection_rejects_missing_or_unsupported_rotation() {
        let mut video: crate::VideoParameters = serde_json::from_value(
            serde_json::json!({"w":320,"h":180,"dw":320,"dh":180,"rotate":45}),
        )
        .unwrap();
        assert!(Projection::for_video(&video, viewport()).is_err());
        video.rotate = None;
        assert!(Projection::for_video(&video, viewport()).is_err());
        video.rotate = Some(450);
        assert!(Projection::for_video(&video, viewport()).is_ok());
        video.dw = None;
        assert!(Projection::for_video(&video, viewport()).is_err());
    }
    fn viewport() -> DisplayRect {
        DisplayRect {
            x: 0.0,
            y: 0.0,
            width: 960.0,
            height: 600.0,
        }
    }

    #[test]
    fn letterbox_selection_uses_source_pixels_and_accepts_reverse_drag() {
        let projection =
            Projection::contain((320, 180), (320.0, 180.0), Rotation::Zero, viewport()).unwrap();
        assert_eq!(
            projection.content(),
            DisplayRect {
                x: 0.0,
                y: 30.0,
                width: 960.0,
                height: 540.0
            }
        );
        let start = Point { x: 30.0, y: 66.0 };
        let end = Point { x: 930.0, y: 546.0 };
        assert_eq!(
            projection.select(start, end).unwrap(),
            Rectangle {
                x: 10,
                y: 12,
                width: 300,
                height: 160
            }
        );
        assert_eq!(
            projection.select(end, start).unwrap(),
            projection.select(start, end).unwrap()
        );
        assert!(
            projection
                .select(Point { x: 0.0, y: 0.0 }, Point { x: 960.0, y: 20.0 })
                .is_err()
        );
    }

    #[test]
    fn rotation_aspect_and_resize_preserve_source_rectangle() {
        let rectangle = Rectangle {
            x: 23,
            y: 17,
            width: 251,
            height: 137,
        };
        for degrees in [0, 90, 180, 270] {
            for display in [(320.0, 180.0), (180.0, 180.0), (640.0, 180.0)] {
                for size in [(960.0, 600.0), (321.5, 777.25)] {
                    let projection = Projection::contain(
                        (320, 180),
                        display,
                        Rotation::from_degrees(degrees).unwrap(),
                        DisplayRect {
                            width: size.0,
                            height: size.1,
                            ..viewport()
                        },
                    )
                    .unwrap();
                    let screen = projection.project(rectangle).unwrap();
                    assert_eq!(
                        projection
                            .select(
                                Point {
                                    x: screen.x,
                                    y: screen.y
                                },
                                Point {
                                    x: screen.x + screen.width,
                                    y: screen.y + screen.height
                                }
                            )
                            .unwrap(),
                        rectangle
                    );
                }
            }
        }
    }

    #[test]
    fn quarter_turn_projects_top_strip_to_right_edge() {
        let projection = Projection::contain(
            (320, 180),
            (320.0, 180.0),
            Rotation::Clockwise90,
            viewport(),
        )
        .unwrap();
        assert_eq!(
            projection.content(),
            DisplayRect {
                x: 311.25,
                y: 0.0,
                width: 337.5,
                height: 600.0
            }
        );
        let strip = projection
            .project(Rectangle {
                x: 0,
                y: 0,
                width: 320,
                height: 18,
            })
            .unwrap();
        assert!((strip.x - 615.0).abs() < 1e-9);
        assert!((strip.width - 33.75).abs() < 1e-9);
        assert_eq!(strip.height, 600.0);
    }

    #[test]
    fn edge_snap_clamps_letterbox_and_rejects_invalid_geometry() {
        let projection =
            Projection::contain((320, 180), (320.0, 180.0), Rotation::Zero, viewport()).unwrap();
        assert_eq!(
            projection
                .select(Point { x: 9.0, y: 39.0 }, Point { x: 951.0, y: 561.0 })
                .unwrap(),
            Rectangle {
                x: 0,
                y: 0,
                width: 320,
                height: 180
            }
        );
        assert!(
            projection
                .select(
                    Point {
                        x: f64::NAN,
                        y: 0.0
                    },
                    Point { x: 960.0, y: 600.0 }
                )
                .is_err()
        );
        assert!(Projection::contain((320, 0), (320.0, 180.0), Rotation::Zero, viewport()).is_err());
        assert!(
            Projection::contain(
                (320, 180),
                (f64::INFINITY, 180.0),
                Rotation::Zero,
                viewport()
            )
            .is_err()
        );
    }
}
