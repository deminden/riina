//! Music mode transitions are based on observed track catalogs, not filename guesses.

use std::time::{Duration, Instant};

const AUTO_SWITCH_STABILITY: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioStatus {
    Unknown,
    Audio,
    Video,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    active: bool,
    auto_switch: bool,
    manual_override: bool,
    transition_candidate: Option<(bool, Instant)>,
}

impl State {
    pub const fn new(auto_switch: bool) -> Self {
        Self {
            active: false,
            auto_switch,
            manual_override: false,
            transition_candidate: None,
        }
    }

    pub const fn active(self) -> bool {
        self.active
    }

    pub const fn auto_switch(self) -> bool {
        self.auto_switch
    }

    pub fn set_auto_switch(&mut self, enabled: bool) {
        self.auto_switch = enabled;
        if !enabled {
            self.transition_candidate = None;
        }
    }

    /// Manual switches toggle IINA's auto-switch override until the next manual switch.
    pub fn toggle_manually(&mut self) -> bool {
        self.active = !self.active;
        self.manual_override = !self.manual_override;
        self.transition_candidate = None;
        self.active
    }

    /// Returns a new mode only when the observed catalog warrants a transition.
    pub fn observe(&mut self, status: AudioStatus, fullscreen: bool) -> Option<bool> {
        self.observe_at(status, fullscreen, Instant::now())
    }

    /// Observe with an explicit clock, including quiet, paused-catalog checks.
    pub fn observe_at(
        &mut self,
        status: AudioStatus,
        fullscreen: bool,
        now: Instant,
    ) -> Option<bool> {
        if !self.auto_switch || self.manual_override || status == AudioStatus::Unknown {
            self.transition_candidate = None;
            return None;
        }
        let enter = status == AudioStatus::Audio;
        if enter == self.active || (enter && fullscreen) {
            self.transition_candidate = None;
            return None;
        }

        // mpv can publish an audio track before the video's track-list entry.
        // Waiting for a stable catalog avoids toggling Music Mode and resizing
        // the player during every video-to-video playlist handoff.
        let Some((candidate, started)) = self.transition_candidate else {
            self.transition_candidate = Some((enter, now));
            return None;
        };
        if candidate != enter {
            self.transition_candidate = Some((enter, now));
            return None;
        }
        if now.saturating_duration_since(started) < AUTO_SWITCH_STABILITY {
            return None;
        }

        self.transition_candidate = None;
        self.active = enter;
        Some(enter)
    }
}

#[cfg(test)]
mod tests {
    use super::{AudioStatus, State};
    use crate::{Playback, PlaylistEntry, Track, TrackKind};
    use std::time::{Duration, Instant};

    fn track(kind: TrackKind, albumart: bool) -> Track {
        Track {
            id: 1,
            kind,
            title: None,
            lang: None,
            codec: None,
            selected: false,
            main_selection: None,
            external: false,
            albumart,
            external_filename: None,
            is_default: false,
            forced: false,
            channels: None,
            channel_layout: None,
            sample_rate: None,
            width: None,
            height: None,
            frame_rate: None,
        }
    }

    #[test]
    fn audio_detection_ignores_album_art_video_but_requires_audio_catalog() {
        let audio_with_art = Playback {
            tracks: vec![
                track(TrackKind::Audio, false),
                track(TrackKind::Video, true),
            ],
            ..Playback::default()
        };
        assert_eq!(audio_with_art.audio_status(), AudioStatus::Audio);

        let real_video = Playback {
            tracks: vec![
                track(TrackKind::Audio, false),
                track(TrackKind::Video, false),
            ],
            ..Playback::default()
        };
        assert_eq!(real_video.audio_status(), AudioStatus::Video);
        assert_eq!(Playback::default().audio_status(), AudioStatus::Unknown);
    }

    #[test]
    fn automatic_mode_enters_audio_and_exits_for_video_but_respects_fullscreen() {
        let mut state = State::new(true);
        let start = Instant::now();
        assert_eq!(state.observe_at(AudioStatus::Audio, true, start), None);
        assert_eq!(state.observe_at(AudioStatus::Audio, false, start), None);
        assert_eq!(
            state.observe_at(
                AudioStatus::Audio,
                false,
                start + Duration::from_millis(300)
            ),
            Some(true)
        );
        assert_eq!(
            state.observe_at(
                AudioStatus::Video,
                false,
                start + Duration::from_millis(301)
            ),
            None
        );
        assert_eq!(
            state.observe_at(
                AudioStatus::Video,
                false,
                start + Duration::from_millis(601)
            ),
            Some(false)
        );
    }

    #[test]
    fn a_video_track_arriving_after_audio_cancels_the_transient_music_mode_candidate() {
        let mut state = State::new(true);
        let start = Instant::now();
        assert_eq!(state.observe_at(AudioStatus::Audio, false, start), None);
        assert_eq!(
            state.observe_at(
                AudioStatus::Video,
                false,
                start + Duration::from_millis(120)
            ),
            None
        );
        assert_eq!(
            state.observe_at(
                AudioStatus::Video,
                false,
                start + Duration::from_millis(500)
            ),
            None
        );
        assert!(!state.active());
    }

    #[test]
    fn unknown_catalog_cancels_a_pending_automatic_mode_change() {
        let mut state = State::new(true);
        let start = Instant::now();
        assert_eq!(state.observe_at(AudioStatus::Audio, false, start), None);
        assert_eq!(
            state.observe_at(
                AudioStatus::Unknown,
                false,
                start + Duration::from_millis(200)
            ),
            None
        );
        assert_eq!(
            state.observe_at(
                AudioStatus::Audio,
                false,
                start + Duration::from_millis(501)
            ),
            None
        );
    }

    #[test]
    fn manual_toggle_overrides_automatic_switching_until_toggled_again() {
        let mut state = State::new(true);
        let start = Instant::now();
        assert!(state.toggle_manually());
        assert_eq!(state.observe_at(AudioStatus::Video, false, start), None);
        assert!(!state.toggle_manually());
        assert_eq!(state.observe_at(AudioStatus::Audio, false, start), None);
        assert_eq!(
            state.observe_at(
                AudioStatus::Audio,
                false,
                start + Duration::from_millis(300)
            ),
            Some(true)
        );
    }

    #[test]
    fn network_items_do_not_qualify_for_automatic_music_mode() {
        let state = Playback {
            tracks: vec![track(TrackKind::Audio, false)],
            playlist: vec![PlaylistEntry {
                id: 1,
                filename: "https://example.test/audio.mp3".into(),
                title: None,
                current: true,
                playing: true,
            }],
            ..Playback::default()
        };
        assert_eq!(state.audio_status(), AudioStatus::Video);
    }

    #[test]
    fn music_preferences_keep_iina_defaults_and_read_legacy_files() {
        let preferences = crate::Preferences::default();
        assert!(preferences.auto_switch_to_music_mode);
        assert!(!preferences.music_mode_show_playlist);
        assert!(preferences.music_mode_show_album_art);

        let legacy = crate::Preferences::parse("{}").unwrap();
        assert_eq!(
            legacy.auto_switch_to_music_mode,
            preferences.auto_switch_to_music_mode
        );
        assert_eq!(
            legacy.music_mode_show_playlist,
            preferences.music_mode_show_playlist
        );
        assert_eq!(
            legacy.music_mode_show_album_art,
            preferences.music_mode_show_album_art
        );
    }
}
