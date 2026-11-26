//! Apply mpv window-scale input to an application-owned video window.
use super::{ManualSizeContext, PixelSize, size_at_aspect};

#[derive(Debug, PartialEq)]
pub enum ProfileWindowScale {
    Set(f64),
    Multiply(f64),
    Cycle(Vec<f64>),
}

impl ProfileWindowScale {
    pub fn parse(action: &str) -> Option<Self> {
        let commands = crate::key_bindings::mpv_action_commands(action).ok()?;
        let [command] = commands.as_slice() else {
            return None;
        };
        let words: Vec<_> = command.iter().map(String::as_str).collect();
        match words.as_slice() {
            ["set", "window-scale", value] => valid_factor(value).map(Self::Set),
            ["multiply", "window-scale", value] => positive_number(value).map(Self::Multiply),
            ["cycle-values", "window-scale", values @ ..] => cycle(values.iter().copied()),
            ["cycle-values", "!reverse", "window-scale", values @ ..] => {
                cycle(values.iter().rev().copied())
            }
            _ => None,
        }
    }

    pub fn target(&self, context: &ManualSizeContext) -> Option<PixelSize> {
        let (width, height) = context.video_size()?;
        if !context.current.valid()
            || !context.display_scale.is_finite()
            || context.display_scale <= 0.0
        {
            return None;
        }
        let base = if context.use_physical_resolution {
            width
        } else {
            width * context.display_scale
        };
        let current = f64::from(context.current.width);
        let requested = match self {
            Self::Set(factor) => base * factor,
            Self::Multiply(factor) => {
                // mpv clamps the resulting property value to its numeric range.
                base * (current / base * factor).clamp(0.001, 100.0)
            }
            Self::Cycle(factors) => {
                // Compare with the rounded requested width that the window can report.
                let next = factors
                    .iter()
                    .position(|factor| (base * factor).round() == current)
                    .map_or(0, |index| (index + 1) % factors.len());
                base * factors.get(next)?
            }
        };
        if !requested.is_finite() || requested <= 0.0 {
            return None;
        }
        let aspect = width / height;
        let minimum_width =
            f64::from(context.minimum.width).max(f64::from(context.minimum.height) * aspect);
        Some(size_at_aspect(
            requested.max(minimum_width),
            aspect,
            context.maximum,
        ))
    }
}

fn positive_number(value: &str) -> Option<f64> {
    value
        .parse()
        .ok()
        .filter(|value: &f64| value.is_finite() && *value > 0.0)
}

fn valid_factor(value: &str) -> Option<f64> {
    positive_number(value).filter(|value| (0.001..=100.0).contains(value))
}

fn cycle<'a>(values: impl Iterator<Item = &'a str>) -> Option<ProfileWindowScale> {
    let factors = values.map(valid_factor).collect::<Option<Vec<_>>>()?;
    (!factors.is_empty()).then_some(ProfileWindowScale::Cycle(factors))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_actions_use_the_actual_frame_and_keep_the_result_inside_the_work_area() {
        let context = ManualSizeContext {
            current: PixelSize {
                width: 384,
                height: 216,
            },
            video_width: 768.0,
            video_height: 432.0,
            rotation: None,
            display_scale: 2.0,
            use_physical_resolution: true,
            maximum: Some(PixelSize {
                width: 1120,
                height: 900,
            }),
            minimum: PixelSize {
                width: 200,
                height: 120,
            },
        };
        for (action, expected) in [
            ("cycle-values window-scale 0.25 0.5 1 2", (768, 432)),
            (
                "cycle-values !reverse window-scale 0.25 0.5 1 2",
                (213, 120),
            ),
            ("cycle-values window-scale 2 1", (1120, 630)),
            ("multiply window-scale 1.1", (422, 238)),
            ("set window-scale 2", (1120, 630)),
        ] {
            let size = ProfileWindowScale::parse(action)
                .unwrap()
                .target(&context)
                .unwrap();
            assert_eq!((size.width, size.height), expected, "{action}");
        }
        assert!(ProfileWindowScale::parse("set window-scale -5").is_none());
        assert!(ProfileWindowScale::parse("cycle-values window-scale").is_none());
    }
}
