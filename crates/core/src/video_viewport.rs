//! Native presentation margins, independent of physical pixels and DPI.
use crate::crop_geometry::DisplayRect;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport([f64; 4]);

impl Viewport {
    pub fn new(size: (f64, f64), picture: DisplayRect) -> Result<Self, String> {
        if ![
            size.0,
            size.1,
            picture.x,
            picture.y,
            picture.width,
            picture.height,
        ]
        .into_iter()
        .all(f64::is_finite)
            || size.0 <= 0.0
            || size.1 <= 0.0
            || picture.x < 0.0
            || picture.y < 0.0
            || picture.width <= 0.0
            || picture.height <= 0.0
            || picture.x + picture.width > size.0
            || picture.y + picture.height > size.1
        {
            return Err("Video viewport is outside the window".into());
        }
        Ok(Self([
            picture.x / size.0,
            (size.0 - picture.x - picture.width) / size.0,
            picture.y / size.1,
            (size.1 - picture.y - picture.height) / size.1,
        ]))
    }

    pub fn commands(viewport: Option<Self>) -> Vec<Vec<String>> {
        // Ratios refer to the complete output window, not the remaining area.
        ["left", "right", "top", "bottom"]
            .into_iter()
            .zip(viewport.map_or([0.0; 4], |value| value.0))
            .map(|(edge, ratio)| {
                vec![
                    "set".into(),
                    format!("video-margin-ratio-{edge}"),
                    ratio.to_string(),
                ]
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn margins_use_full_window_and_reset_all_edges() {
        let picture = DisplayRect {
            x: 40.0,
            y: 20.0,
            width: 920.0,
            height: 492.0,
        };
        let viewport = Viewport::new((1000.0, 600.0), picture).unwrap();
        assert_eq!(viewport.0, [0.04, 0.04, 20.0 / 600.0, 88.0 / 600.0]);
        assert!(
            Viewport::commands(None)
                .iter()
                .all(|command| command[2] == "0")
        );
        assert!(Viewport::new((900.0, 600.0), picture).is_err());
        assert!(Viewport::new((f64::NAN, 600.0), picture).is_err());
    }
}
