//! Playback and clock state shared by Touch Bar controls.
use crate::{Phase, Playback, osc_preferences::ControllerOptions};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub show_remaining_time: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            show_remaining_time: true,
        }
    }
}

impl Settings {
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    #[must_use]
    pub fn clocks(self, controller: ControllerOptions, playback: &Playback) -> (String, String) {
        let separate_mode = ControllerOptions {
            show_remaining_time: self.show_remaining_time,
            ..controller
        };
        (
            separate_mode.elapsed_label(playback.position),
            separate_mode.end_label(playback),
        )
    }
}

/// Time is meaningful only for an active, seekable file with a known duration.
#[must_use]
pub fn seekable_duration(playback: &Playback) -> Option<f64> {
    if !playback.seekable
        || matches!(
            playback.phase,
            Phase::Loading | Phase::Stopped | Phase::Failed
        )
    {
        return None;
    }
    playback
        .duration
        .filter(|value| value.is_finite() && *value > 0.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScrubOrigin {
    pub media: u64,
    pub open_request: i32,
}

#[derive(Debug, Default)]
pub struct ScrubSession {
    origin: Option<(ScrubOrigin, bool)>,
}

impl ScrubSession {
    /// Duplicate begin events retain the original playback intent.
    pub fn begin(&mut self, origin: ScrubOrigin, playing: bool) -> bool {
        if self.origin.is_some() {
            return false;
        }
        self.origin = Some((origin, playing));
        playing
    }

    #[must_use]
    pub fn allows_seek(&self, origin: ScrubOrigin) -> bool {
        self.origin.is_none_or(|(started, _)| started == origin)
    }

    /// A scrub can resume only the media and open request that began it.
    pub fn end(&mut self, origin: ScrubOrigin) -> bool {
        self.origin
            .take()
            .is_some_and(|(started, playing)| started == origin && playing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrub_restores_original_intent_and_rejects_a_new_media_request() {
        let first = ScrubOrigin {
            media: 4,
            open_request: 1,
        };
        let waiting = ScrubOrigin {
            open_request: 2,
            ..first
        };
        let next = ScrubOrigin {
            media: 5,
            ..waiting
        };
        let mut scrub = ScrubSession::default();
        assert!(scrub.allows_seek(first));
        assert!(scrub.begin(first, true));
        assert!(!scrub.begin(first, false));
        assert!(scrub.allows_seek(first));
        assert!(scrub.end(first));
        assert!(!scrub.begin(first, false));
        assert!(!scrub.end(first));
        assert!(scrub.begin(first, true));
        assert!(!scrub.allows_seek(waiting));
        assert!(!scrub.allows_seek(next));
        assert!(!scrub.end(next));
        assert!(!scrub.end(first));
    }

    #[test]
    fn old_profiles_start_with_a_separate_remaining_time_display() {
        let original = crate::Preferences::parse("{}").unwrap();
        assert!(original.touch_bar.show_remaining_time);
        let playback = Playback {
            position: Some(12.0),
            duration: Some(36.0),
            remaining_time: Some(24.0),
            ..Default::default()
        };
        assert_eq!(
            original
                .touch_bar
                .clocks(original.control_bar.controller, &playback),
            ("00:12".into(), "-00:24".into())
        );
        let edited =
            crate::Preferences::parse(r#"{"touch_bar":{"show_remaining_time":false}}"#).unwrap();
        assert!(!edited.touch_bar.show_remaining_time);
        assert!(!edited.control_bar.controller.show_remaining_time);
        assert_eq!(
            edited
                .touch_bar
                .clocks(edited.control_bar.controller, &playback)
                .1,
            "00:36"
        );
    }
}
