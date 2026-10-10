//! Source-pixel crop geometry, independent of presentation aspect and rotation.

pub const FILTER_LABEL: &str = "riina-crop";
pub const LABELS: [&str; 7] = ["None", "4:3", "16:9", "16:10", "21:9", "5:4", "Custom…"];
// The reference menu and quick-settings panel intentionally offer different lists.
pub const MENU_LABELS: [&str; 10] = [
    "None", "4:3", "5:4", "16:9", "16:10", "1:1", "3:2", "2.21:1", "2.35:1", "2.39:1",
];
pub const MENU_CUSTOM_INDEX: usize = MENU_LABELS.len();

/// Catalog-derived selection; unknown native state must not look like None.
pub fn observed_index(state: &crate::Playback) -> Option<usize> {
    observed_for(state, &Preset::ALL)
}

pub fn observed_menu_index(state: &crate::Playback) -> Option<usize> {
    let index = observed_for(state, &Preset::MENU_ALL)?;
    // 21:9 belongs to the panel, not the menu. A panel preset must not falsely
    // check Custom; explicit rectangle parameters still identify custom intent.
    if index == MENU_CUSTOM_INDEX && observed_index(state) == Some(4) {
        None
    } else {
        Some(index)
    }
}

fn observed_for(state: &crate::Playback, presets: &[Preset]) -> Option<usize> {
    let filters = state.video_filters.as_ref()?;
    let Some(filter) = filters
        .iter()
        .find(|filter| filter.label.as_deref() == Some(FILTER_LABEL))
    else {
        return Some(0);
    };
    let source = state.video.as_ref()?;
    let param = |name: &str, position: &str| {
        filter
            .params
            .get(name)
            .or_else(|| filter.params.get(position))
    };
    // Explicit coordinates identify custom intent even if the dimensions happen
    // to equal a preset. Unrecognized native syntax stays Custom.
    if filter.name != "crop" || param("x", "@2").is_some() || param("y", "@3").is_some() {
        return Some(presets.len() + 1);
    }
    let dimensions = param("w", "@0")
        .and_then(|w| w.parse::<u32>().ok())
        .zip(param("h", "@1").and_then(|h| h.parse::<u32>().ok()));
    let Some(dimensions) = dimensions else {
        return Some(presets.len() + 1);
    };
    let mut matches = presets
        .iter()
        .enumerate()
        .filter(|(_, preset)| preset.dimensions(source.w, source.h).ok() == Some(dimensions));
    match (matches.next(), matches.next()) {
        (None, _) => Some(presets.len() + 1),
        (Some((index, _)), None) => Some(index + 1),
        // Tiny sources can produce identical dimensions for different ratios.
        // Native geometry alone cannot establish which named preset was chosen.
        (Some(_), Some(_)) => None,
    }
}

/// Source pixels with a top-left origin, as used by the native crop filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rectangle {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rectangle {
    pub fn filter(self, source_width: u32, source_height: u32) -> Result<String, String> {
        if self.width == 0 || self.height == 0 {
            return Err("The crop rectangle has no area".into());
        }
        self.editor_filter(source_width, source_height)
    }

    /// Interactive confirmation forwards zero dimensions to the engine, as
    /// the reference does; preset geometry still requires a nonempty rectangle.
    pub fn editor_filter(self, source_width: u32, source_height: u32) -> Result<String, String> {
        // Validate against the source at dispatch, not the UI's possibly stale
        // frame. Subtraction after checking the origin avoids coordinate overflow.
        if source_width == 0
            || source_height == 0
            || self.x > source_width
            || self.y > source_height
            || self.width > source_width - self.x
            || self.height > source_height - self.y
        {
            return Err("The crop rectangle is outside the source video".into());
        }
        Ok(format!(
            "@{FILTER_LABEL}:crop={}:{}:{}:{}",
            self.width, self.height, self.x, self.y
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    FourThree,
    SixteenNine,
    SixteenTen,
    TwentyOneNine,
    FiveFour,
    Square,
    ThreeTwo,
    Cinema221,
    Cinema235,
    Cinema239,
}

impl Preset {
    pub const MENU_ALL: [Self; 9] = [
        Self::FourThree,
        Self::FiveFour,
        Self::SixteenNine,
        Self::SixteenTen,
        Self::Square,
        Self::ThreeTwo,
        Self::Cinema221,
        Self::Cinema235,
        Self::Cinema239,
    ];
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
            Self::Square => "1:1",
            Self::ThreeTwo => "3:2",
            Self::Cinema221 => "2.21:1",
            Self::Cinema235 => "2.35:1",
            Self::Cinema239 => "2.39:1",
        }
    }

    pub fn dimensions(self, width: u32, height: u32) -> Result<(u32, u32), String> {
        let (numerator, denominator): (u64, u64) = match self {
            Self::FourThree => (4, 3),
            Self::SixteenNine => (16, 9),
            Self::SixteenTen => (16, 10),
            Self::TwentyOneNine => (21, 9),
            Self::FiveFour => (5, 4),
            Self::Square => (1, 1),
            Self::ThreeTwo => (3, 2),
            Self::Cinema221 => (221, 100),
            Self::Cinema235 => (235, 100),
            Self::Cinema239 => (239, 100),
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
    fn menu_geometry_preserves_decimal_ratios_and_distinct_panel_selection() {
        let expected = [
            (240, 180),
            (225, 180),
            (320, 180),
            (288, 180),
            (180, 180),
            (270, 180),
            (320, 144),
            (320, 136),
            (320, 133),
        ];
        let mut state = crate::Playback {
            video: Some(serde_json::from_value(serde_json::json!({"w":320,"h":180})).unwrap()),
            ..Default::default()
        };
        for (index, preset) in Preset::MENU_ALL.into_iter().enumerate() {
            let dimensions = preset.dimensions(320, 180).unwrap();
            assert_eq!(dimensions, expected[index]);
            state.video_filters = Some(vec![serde_json::from_value(serde_json::json!({
                "name":"crop","label":FILTER_LABEL,"params":{"w":dimensions.0.to_string(),"h":dimensions.1.to_string()}
            })).unwrap()]);
            assert_eq!(observed_menu_index(&state), Some(index + 1));
        }
        state.video_filters = Some(vec![serde_json::from_value(serde_json::json!({"name":"crop","label":FILTER_LABEL,"params":{"w":"320","h":"137"}})).unwrap()]);
        assert_eq!(observed_index(&state), Some(4));
        assert_eq!(observed_menu_index(&state), None);
        state.video_filters.as_mut().unwrap()[0]
            .params
            .insert("x".into(), "0".into());
        assert_eq!(observed_menu_index(&state), Some(MENU_CUSTOM_INDEX));
        state.video = Some(serde_json::from_value(serde_json::json!({"w":8,"h":8})).unwrap());
        state.video_filters.as_mut().unwrap()[0].params =
            [("w".into(), "8".into()), ("h".into(), "3".into())].into();
        assert_eq!(observed_menu_index(&state), None);
    }

    #[test]
    fn observed_selection_distinguishes_unknown_none_preset_and_explicit_rectangle() {
        let mut state = crate::Playback::default();
        assert_eq!(observed_index(&state), None);
        state.video_filters = Some(vec![]);
        assert_eq!(observed_index(&state), Some(0));
        state.video = Some(serde_json::from_value(serde_json::json!({"w":320,"h":180})).unwrap());
        state.video_filters = Some(vec![serde_json::from_value(serde_json::json!({"name":"crop","label":FILTER_LABEL,"params":{"@0":"240","@1":"180"}})).unwrap()]);
        assert_eq!(observed_index(&state), Some(1));
        state.video_filters.as_mut().unwrap()[0]
            .params
            .insert("@2".into(), "0".into());
        assert_eq!(observed_index(&state), Some(6));
        state.video_filters.as_mut().unwrap()[0].params =
            [("w".into(), "288".into()), ("h".into(), "180".into())].into();
        assert_eq!(observed_index(&state), Some(3));
    }

    #[test]
    fn custom_rectangles_validate_source_edges_without_overflow() {
        let rectangle = Rectangle {
            x: 20,
            y: 10,
            width: 300,
            height: 170,
        };
        assert_eq!(
            rectangle.filter(320, 180).unwrap(),
            "@riina-crop:crop=300:170:20:10"
        );
        assert!(rectangle.filter(319, 180).is_err());
        for invalid in [
            Rectangle {
                width: 0,
                ..rectangle
            },
            Rectangle {
                height: 0,
                ..rectangle
            },
            Rectangle {
                x: u32::MAX,
                ..rectangle
            },
            Rectangle {
                width: u32::MAX,
                ..rectangle
            },
            Rectangle {
                y: u32::MAX,
                ..rectangle
            },
        ] {
            assert!(invalid.filter(320, 180).is_err());
        }
        assert!(
            Rectangle {
                x: 0,
                y: 0,
                width: u32::MAX,
                height: u32::MAX
            }
            .filter(u32::MAX, u32::MAX)
            .is_ok()
        );
    }

    #[test]
    fn confirming_whole_source_clears_only_the_managed_crop() {
        let state = crate::Playback {
            video: Some(serde_json::from_value(serde_json::json!({"w":320,"h":180})).unwrap()),
            video_filters: Some(vec![VideoFilter {
                name: "crop".into(),
                label: Some(FILTER_LABEL.into()),
                enabled: Some(true),
                params: Default::default(),
            }]),
            ..Default::default()
        };
        let whole = Rectangle {
            x: 0,
            y: 0,
            width: 320,
            height: 180,
        };
        assert_eq!(
            crate::Command::VideoCropRectangle(whole)
                .plan(&state)
                .unwrap()
                .commands,
            crate::Command::VideoCrop(None)
                .plan(&state)
                .unwrap()
                .commands
        );
        assert!(
            crate::Command::VideoCropRectangle(Rectangle {
                width: 321,
                ..whole
            })
            .plan(&state)
            .is_err()
        );
    }

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
