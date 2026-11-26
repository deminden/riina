//! Geometry and lifetime policy for the screenshot accessory.
use riina_core::{Phase, Playback};

pub(crate) fn should_clear(state: &Playback, displayed: Option<&Playback>) -> bool {
    displayed.is_none_or(|old| old.generation != state.generation)
        || matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed)
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
        let displayed = Playback {
            phase: Phase::Playing,
            generation: 1,
            ..Default::default()
        };
        let mut state = displayed.clone();
        state
            .notices
            .push(riina_core::NoticeSeverity::Error, "capture failed".into());
        assert!(!should_clear(&state, Some(&displayed)));
        state.phase = Phase::Stopped;
        assert!(should_clear(&state, Some(&displayed)));
        state.phase = Phase::Playing;
        state.generation += 1;
        assert!(should_clear(&state, Some(&displayed)));
    }

    #[test]
    fn preview_preserves_aspect_and_fills_reference_bounds() {
        assert_eq!(preview_size(96, 64), (300.0, 200.0));
        assert_eq!(preview_size(1920, 1080), (300.0, 168.75));
        assert_eq!(preview_size(100, 200), (100.0, 200.0));
        assert_eq!(preview_size(0, 10), (300.0, 200.0));
    }
}
