//! Source-pixel crop geometry, independent of presentation aspect and rotation.

pub const FILTER_LABEL: &str = "riina-crop";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    FourThree,
    SixteenNine,
    SixteenTen,
    TwentyOneNine,
    FiveFour,
}

impl Preset {
    pub const ALL: [Self; 5] = [
        Self::FourThree,
        Self::SixteenNine,
        Self::SixteenTen,
        Self::TwentyOneNine,
        Self::FiveFour,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::FourThree => "4:3",
            Self::SixteenNine => "16:9",
            Self::SixteenTen => "16:10",
            Self::TwentyOneNine => "21:9",
            Self::FiveFour => "5:4",
        }
    }

    pub fn dimensions(self, width: u32, height: u32) -> Result<(u32, u32), String> {
        let (numerator, denominator): (u64, u64) = match self {
            Self::FourThree => (4, 3),
            Self::SixteenNine => (16, 9),
            Self::SixteenTen => (16, 10),
            Self::TwentyOneNine => (21, 9),
            Self::FiveFour => (5, 4),
        };
        // Integer cross-products avoid floating-point boundary drift. Presets
        // truncate the reduced dimension; the native filter centers the crop.
        let (w, h) = if u64::from(width) * denominator > u64::from(height) * numerator {
            ((u64::from(height) * numerator / denominator) as u32, height)
        } else {
            (width, (u64::from(width) * denominator / numerator) as u32)
        };
        if w == 0 || h == 0 {
            return Err("The source is too small for this crop".into());
        }
        Ok((w, h))
    }

    pub fn filter(self, width: u32, height: u32) -> Result<String, String> {
        let (w, h) = self.dimensions(width, height)?;
        Ok(format!("@{FILTER_LABEL}:crop={w}:{h}"))
    }
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct VideoFilter {
    pub name: String,
    pub label: Option<String>,
    pub enabled: Option<bool>,
    #[serde(default)]
    pub params: std::collections::BTreeMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_preserves_foreign_filters_and_rejects_unknown_catalogs() {
        let mut state = crate::Playback::default();
        assert!(crate::Command::VideoCrop(None).plan(&state).is_err());
        let foreign = VideoFilter {
            name: "crop".into(),
            label: Some("user-crop".into()),
            enabled: Some(true),
            params: Default::default(),
        };
        state.video_filters = Some(vec![foreign.clone()]);
        assert!(
            crate::Command::VideoCrop(None)
                .plan(&state)
                .unwrap()
                .commands
                .is_empty()
        );
        state.video_filters.as_mut().unwrap().push(VideoFilter {
            label: Some(FILTER_LABEL.into()),
            ..foreign
        });
        assert_eq!(
            crate::Command::VideoCrop(None)
                .plan(&state)
                .unwrap()
                .commands,
            vec![vec!["vf", "remove", "@riina-crop"]]
        );
        assert_eq!(state.video_filters.as_ref().unwrap().len(), 2);
        assert!(
            crate::Command::VideoCrop(Some(Preset::FourThree))
                .plan(&state)
                .is_err()
        );
    }

    #[test]
    fn presets_inscribe_source_pixels_and_truncate_fractional_edges() {
        let expected = [(240, 180), (320, 180), (288, 180), (320, 137), (225, 180)];
        for (preset, dimensions) in Preset::ALL.into_iter().zip(expected) {
            assert_eq!(preset.dimensions(320, 180).unwrap(), dimensions);
        }
        assert_eq!(
            Preset::SixteenNine.dimensions(180, 320).unwrap(),
            (180, 101)
        );
        assert_eq!(Preset::FourThree.dimensions(321, 181).unwrap(), (241, 181));
        assert!(Preset::FourThree.dimensions(0, 180).is_err());
        assert!(Preset::TwentyOneNine.dimensions(1, 1).is_err());
        assert_eq!(
            Preset::SixteenNine.dimensions(u32::MAX, u32::MAX).unwrap(),
            (u32::MAX, 2_415_919_103)
        );
    }
}
