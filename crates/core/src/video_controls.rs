//! Portable video intent; presentation labels remain distinct from native ratios.

pub const ASPECT_PRESETS: [&str; 6] = ["Default", "4:3", "16:9", "16:10", "21:9", "5:4"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AspectRatio(Option<String>);

impl AspectRatio {
    pub fn from_text(text: &str) -> Self {
        // Invalid nonempty custom input resets the override, matching the player
        // action. Empty custom edits are ignored by the UI before reaching here.
        let valid = text.split_once(':').is_some_and(|(width, height)| {
            decimal(width) && decimal(height) && !height.contains(':')
        });
        Self(valid.then(|| text.to_owned()))
    }

    pub fn native_value(&self) -> &str {
        self.0.as_deref().unwrap_or("-1")
    }

    pub fn label(&self) -> &str {
        self.0.as_deref().unwrap_or("Default")
    }

    pub fn preset_index(&self) -> Option<usize> {
        ASPECT_PRESETS
            .iter()
            .position(|preset| *preset == self.label())
    }
}

fn decimal(value: &str) -> bool {
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
    value.split_once('.').map_or_else(
        || digits(value),
        |(integer, fraction)| digits(integer) && digits(fraction),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rotation {
    Zero,
    Clockwise90,
    Clockwise180,
    Clockwise270,
}

impl Rotation {
    pub fn from_degrees(degrees: i64) -> Option<Self> {
        match degrees {
            0 => Some(Self::Zero),
            90 => Some(Self::Clockwise90),
            180 => Some(Self::Clockwise180),
            270 => Some(Self::Clockwise270),
            _ => None,
        }
    }

    pub fn degrees(self) -> i64 {
        match self {
            Self::Zero => 0,
            Self::Clockwise90 => 90,
            Self::Clockwise180 => 180,
            Self::Clockwise270 => 270,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Command;

    #[test]
    fn aspect_input_keeps_exact_ratio_text_and_resets_invalid_syntax() {
        for ratio in ["4:3", "8:6", "2.39:1", "1:1.5", "01.00:02.00", "0:1"] {
            let aspect = AspectRatio::from_text(ratio);
            assert_eq!(aspect.label(), ratio);
            assert_eq!(
                Command::VideoAspect(aspect).mpv_args().unwrap(),
                ["set", "video-aspect-override", ratio]
            );
        }
        for invalid in [
            "", "Default", "16/9", " 16:9", "16:9 ", ".5:1", "1.:1", "1:2:3", "NaN:1", "1:1\0",
        ] {
            let aspect = AspectRatio::from_text(invalid);
            assert_eq!(aspect.label(), "Default");
            assert_eq!(aspect.preset_index(), Some(0));
            assert_eq!(aspect.native_value(), "-1");
        }
        assert_eq!(AspectRatio::from_text("8:6").preset_index(), None);
        for (index, preset) in ASPECT_PRESETS.iter().enumerate() {
            assert_eq!(AspectRatio::from_text(preset).preset_index(), Some(index));
        }
    }

    #[test]
    fn rotation_choices_accept_only_the_reference_quarter_turns() {
        for degrees in [0, 90, 180, 270] {
            let rotation = Rotation::from_degrees(degrees).unwrap();
            assert_eq!(rotation.degrees(), degrees);
            assert_eq!(
                Command::VideoRotation(rotation).mpv_args().unwrap(),
                [
                    "set".to_owned(),
                    "video-rotate".to_owned(),
                    degrees.to_string()
                ]
            );
        }
        for invalid in [-90, 45, 360, i64::MAX] {
            assert_eq!(Rotation::from_degrees(invalid), None);
        }
    }
}
