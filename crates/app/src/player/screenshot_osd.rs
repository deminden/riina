//! Successful capture feedback survives coalescing, but never source replacement.
use riina_core::{Phase, Playback};

#[derive(Debug, PartialEq)]
pub(crate) enum Change {
    Keep,
    Hide,
    Show(String),
}

#[derive(Default)]
pub(crate) struct ScreenshotOsd {
    previous: Option<(u64, u64)>,
}

impl ScreenshotOsd {
    pub(crate) fn observe(&mut self, state: &Playback) -> Change {
        let previous = self
            .previous
            .replace((state.generation, state.completed_screenshots));
        if previous.is_none_or(|(generation, _)| generation != state.generation)
            || matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed)
        {
            return Change::Hide;
        }
        if previous.is_some_and(|(_, count)| count != state.completed_screenshots)
            && let Some(path) = &state.screenshot_file
        {
            // The counter proves capture success even if an unrelated old
            // command error remains in the general playback snapshot.
            return Change::Show(path.clone());
        }
        Change::Keep
    }
}

pub(crate) fn preview_size(width: u32, height: u32) -> (f32, f32) {
    if width == 0 || height == 0 {
        return (300.0, 200.0);
    }
    let scale = (300.0 / width as f32).min(200.0 / height as f32);
    (width as f32 * scale, height as f32 * scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_coalesce_without_replaying_failed_or_replaced_sources() {
        let mut feedback = ScreenshotOsd::default();
        let mut state = Playback {
            phase: Phase::Playing,
            generation: 1,
            ..Default::default()
        };
        assert_eq!(feedback.observe(&state), Change::Hide);
        state.completed_screenshots = 2;
        state.screenshot_file = Some("second.png".into());
        assert_eq!(feedback.observe(&state), Change::Show("second.png".into()));
        assert_eq!(feedback.observe(&state), Change::Keep);
        state.error = Some("capture failed".into());
        assert_eq!(feedback.observe(&state), Change::Keep);
        state.completed_screenshots = 3;
        state.screenshot_file = Some("retry.png".into());
        assert_eq!(feedback.observe(&state), Change::Show("retry.png".into()));
        state.error = None;
        assert_eq!(feedback.observe(&state), Change::Keep);
        state.generation = 2;
        state.completed_screenshots = 4;
        assert_eq!(feedback.observe(&state), Change::Hide);
        assert_eq!(feedback.observe(&state), Change::Keep);
        state.phase = Phase::Stopped;
        assert_eq!(feedback.observe(&state), Change::Hide);
    }

    #[test]
    fn preview_preserves_aspect_and_fills_reference_bounds() {
        assert_eq!(preview_size(96, 64), (300.0, 200.0));
        assert_eq!(preview_size(1920, 1080), (300.0, 168.75));
        assert_eq!(preview_size(100, 200), (100.0, 200.0));
        assert_eq!(preview_size(0, 10), (300.0, 200.0));
    }

    #[test]
    fn deep_rgb_png_loads_through_the_actual_preview_decoder() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/preview-rgb16.png");
        let image = slint::Image::load_from_path(&path).unwrap();
        assert_eq!((image.size().width, image.size().height), (96, 64));
        let pixels = image.to_rgba8().unwrap();
        let expected = [
            [0, 0, 0],
            [255, 255, 255],
            [255, 0, 0],
            [0, 255, 0],
            [0, 0, 255],
            [128, 128, 128],
            [0, 255, 255],
            [255, 0, 255],
        ];
        for (index, expected) in expected.into_iter().enumerate() {
            let pixel = pixels.as_slice()[32 * 96 + index * 12 + 6];
            assert_eq!([pixel.r, pixel.g, pixel.b], expected);
            assert_eq!(pixel.a, 255);
        }
    }
}
