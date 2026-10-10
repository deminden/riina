//! Pointer anchoring for crop selection and its four resize edges.
use crate::crop_geometry::{DisplayRect, Point};

#[derive(Debug, Clone, Copy)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy)]
pub struct Drag {
    edge: Option<Edge>,
    pointer: Point,
    rectangle: DisplayRect,
}

impl Drag {
    pub fn begin(
        edge: Option<Edge>,
        pointer: Point,
        rectangle: DisplayRect,
    ) -> Result<Self, String> {
        if ![
            pointer.x,
            pointer.y,
            rectangle.x,
            rectangle.y,
            rectangle.width,
            rectangle.height,
        ]
        .into_iter()
        .all(f64::is_finite)
            || rectangle.width <= 0.0
            || rectangle.height <= 0.0
        {
            return Err("Crop drag geometry is invalid".into());
        }
        Ok(Self {
            edge,
            pointer,
            rectangle,
        })
    }

    pub fn points(self, pointer: Point) -> Result<(Point, Point), String> {
        if ![pointer.x, pointer.y].into_iter().all(f64::is_finite) {
            return Err("Crop pointer position is invalid".into());
        }
        let dx = pointer.x - self.pointer.x;
        let dy = pointer.y - self.pointer.y;
        let left = self.rectangle.x;
        let top = self.rectangle.y;
        let right = left + self.rectangle.width;
        let bottom = top + self.rectangle.height;
        // Delta from the original grab preserves the pointer's offset inside
        // the hit strip; using absolute pointer position makes the edge jump.
        Ok(match self.edge {
            Some(Edge::Left) => (
                Point {
                    x: (left + dx).min(right),
                    y: top,
                },
                Point {
                    x: right,
                    y: bottom,
                },
            ),
            Some(Edge::Right) => (
                Point { x: left, y: top },
                Point {
                    x: (right + dx).max(left),
                    y: bottom,
                },
            ),
            Some(Edge::Top) => (
                Point {
                    x: left,
                    y: (top + dy).min(bottom),
                },
                Point {
                    x: right,
                    y: bottom,
                },
            ),
            Some(Edge::Bottom) => (
                Point { x: left, y: top },
                Point {
                    x: right,
                    y: (bottom + dy).max(top),
                },
            ),
            None => (self.pointer, pointer),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grabbing_inside_hit_strip_does_not_jump_and_keeps_opposite_edge() {
        let rectangle = DisplayRect {
            x: 30.0,
            y: 60.0,
            width: 300.0,
            height: 160.0,
        };
        for (edge, grab, move_to, expected) in [
            (
                Edge::Left,
                Point { x: 31.5, y: 100.0 },
                Point { x: 41.5, y: 101.0 },
                (Point { x: 40.0, y: 60.0 }, Point { x: 330.0, y: 220.0 }),
            ),
            (
                Edge::Right,
                Point { x: 328.5, y: 100.0 },
                Point { x: 338.5, y: 101.0 },
                (Point { x: 30.0, y: 60.0 }, Point { x: 340.0, y: 220.0 }),
            ),
            (
                Edge::Top,
                Point { x: 100.0, y: 61.5 },
                Point { x: 101.0, y: 71.5 },
                (Point { x: 30.0, y: 70.0 }, Point { x: 330.0, y: 220.0 }),
            ),
            (
                Edge::Bottom,
                Point { x: 100.0, y: 218.5 },
                Point { x: 101.0, y: 228.5 },
                (Point { x: 30.0, y: 60.0 }, Point { x: 330.0, y: 230.0 }),
            ),
        ] {
            let drag = Drag::begin(Some(edge), grab, rectangle).unwrap();
            assert_eq!(
                drag.points(grab).unwrap(),
                (Point { x: 30.0, y: 60.0 }, Point { x: 330.0, y: 220.0 })
            );
            assert_eq!(drag.points(move_to).unwrap(), expected);
        }
    }

    #[test]
    fn free_drag_retains_origin_and_rejects_invalid_pointer() {
        let origin = Point { x: 10.0, y: 20.0 };
        let drag = Drag::begin(
            None,
            origin,
            DisplayRect {
                x: 0.0,
                y: 0.0,
                width: 300.0,
                height: 160.0,
            },
        )
        .unwrap();
        assert_eq!(
            drag.points(Point { x: 5.0, y: 4.0 }).unwrap(),
            (origin, Point { x: 5.0, y: 4.0 })
        );
        assert!(
            drag.points(Point {
                x: f64::NAN,
                y: 4.0
            })
            .is_err()
        );
    }
}
