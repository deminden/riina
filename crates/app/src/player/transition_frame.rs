use riina_core::{CapturedFrame, Phase, Playback, TrackKind};
use std::time::{Duration, Instant};

const INCOMING_CAPTURE_FALLBACK_SECONDS: f64 = 3.0;
const INCOMING_FRAME_STABILITY_SECONDS: f64 = 0.25;
const INCOMING_CAPTURE_STABLE_FOR: Duration = Duration::from_millis(400);
const OUTGOING_CAPTURE_LEAD_SECONDS: f64 = 1.25;
const OUTGOING_COVER_LEAD_SECONDS: f64 = 0.45;
const OUTGOING_CAPTURE_RETRY: Duration = Duration::from_millis(240);

pub fn incoming_cover_release_ready(
    capture_count: u8,
    captured_since: Instant,
    now: Instant,
    native_surface_stable: bool,
) -> bool {
    capture_count >= 3
        && now.duration_since(captured_since) >= INCOMING_CAPTURE_STABLE_FOR
        && native_surface_stable
}

pub fn outgoing_capture_due(
    playback: &Playback,
    captured_generation: Option<u64>,
    last_attempt: Option<(u64, Instant)>,
    now: Instant,
) -> bool {
    if playback.phase != Phase::Playing
        || playback.video.is_none()
        || captured_generation == Some(playback.generation)
        || !playback
            .duration
            .zip(playback.position)
            .is_some_and(|(duration, position)| {
                duration > 0.0 && duration - position <= OUTGOING_CAPTURE_LEAD_SECONDS
            })
    {
        return false;
    }
    last_attempt.is_none_or(|(generation, attempted_at)| {
        generation != playback.generation
            || now.duration_since(attempted_at) >= OUTGOING_CAPTURE_RETRY
    })
}

pub fn incoming_has_video(playback: &Playback) -> bool {
    playback.video.is_some()
        || playback
            .tracks
            .iter()
            .any(|track| track.kind == TrackKind::Video && !track.albumart)
}

pub fn incoming_capture_ready(playback: &Playback) -> bool {
    if !matches!(
        playback.phase,
        Phase::Playing | Phase::Paused | Phase::Seeking
    ) || !incoming_has_video(playback)
    {
        return false;
    }
    let Some(position) = playback.position else {
        return false;
    };
    let wait = playback
        .duration
        .filter(|duration| duration.is_finite() && *duration > 0.0)
        .map_or(INCOMING_FRAME_STABILITY_SECONDS, |duration| {
            INCOMING_FRAME_STABILITY_SECONDS.min(duration * 0.25)
        });
    position >= wait
}

pub fn incoming_frame_is_usable(playback: &Playback) -> bool {
    let Some(frame) = playback.incoming_transition_frame.as_deref() else {
        return false;
    };
    let row_stride = frame.width as usize * 4;
    let step_x = (frame.width as usize).div_ceil(64).max(1);
    let step_y = (frame.height as usize).div_ceil(36).max(1);
    let mut sampled = 0_usize;
    let mut visible = 0_usize;
    let mut left_visible = 0_usize;
    let mut right_visible = 0_usize;
    for y in (0..frame.height as usize).step_by(step_y) {
        for x in (0..frame.width as usize).step_by(step_x) {
            let index = y * row_stride + x * 4;
            if frame.rgba[index..index + 3]
                .iter()
                .copied()
                .max()
                .is_some_and(|channel| channel > 12)
            {
                visible += 1;
                if x < frame.width as usize / 2 {
                    left_visible += 1;
                } else {
                    right_visible += 1;
                }
            }
            sampled += 1;
        }
    }
    let half_sampled = sampled.div_ceil(2);
    visible >= sampled.div_ceil(20).max(1)
        && left_visible >= half_sampled.div_ceil(100).max(1)
        && right_visible >= half_sampled.div_ceil(100).max(1)
}

/// Keep displaying the complete outgoing frame while the native video child
/// settles. Incoming raw captures only gate release; they can contain a partial
/// frame while the decoder and D3D presentation surface are out of step.
pub fn cover_frame(playback: &Playback) -> Option<&CapturedFrame> {
    playback.transition_frame.as_deref()
}

pub fn incoming_capture_window_active(playback: &Playback, now: Instant) -> bool {
    incoming_capture_fallback_active(playback, now)
}

#[cfg(any(test, not(target_os = "windows")))]
pub fn should_display(playback: &Playback, playlist_auto_play_next: bool) -> bool {
    should_display_at(playback, playlist_auto_play_next, Instant::now())
}

#[cfg(any(target_os = "windows", test))]
pub fn should_display_until_incoming_stable(
    playback: &Playback,
    playlist_auto_play_next: bool,
    stable_generation: Option<u64>,
) -> bool {
    let now = Instant::now();
    if should_display_at(playback, playlist_auto_play_next, now) {
        return true;
    }
    // The Slint cover paints an opaque black backing beneath the retained
    // image. Never enable that surface unless the outgoing image exists: an
    // incoming-frame timeout alone must not leave a black rectangle over live
    // playback (or strand it at the overlay's previous screen position).
    if playback.transition_frame.is_none() {
        return false;
    }
    let Some(outgoing_generation) = playback.transition_frame_generation else {
        return false;
    };
    playback.generation > outgoing_generation
        && matches!(
            playback.phase,
            Phase::Playing | Phase::Paused | Phase::Seeking
        )
        && incoming_has_video(playback)
        && playback.incoming_transition_frame_generation == Some(playback.generation)
        && incoming_frame_is_usable(playback)
        && stable_generation != Some(playback.generation)
        && incoming_capture_fallback_active(playback, now)
}

fn incoming_capture_fallback_active(playback: &Playback, now: Instant) -> bool {
    playback.incoming_transition_started_at.map_or_else(
        || {
            playback
                .position
                .is_none_or(|position| position <= INCOMING_CAPTURE_FALLBACK_SECONDS)
        },
        |started_at| {
            now.checked_duration_since(started_at)
                .is_none_or(|elapsed| {
                    elapsed <= Duration::from_secs_f64(INCOMING_CAPTURE_FALLBACK_SECONDS)
                })
        },
    )
}

fn should_display_at(playback: &Playback, playlist_auto_play_next: bool, now: Instant) -> bool {
    let Some(outgoing_generation) = playback.transition_frame_generation else {
        return false;
    };
    if playback.transition_frame.is_none() {
        return false;
    }

    let remaining = playback
        .duration
        .zip(playback.position)
        .map(|(duration, position)| duration - position);
    let near_end = playback.phase == Phase::Playing
        && remaining.is_some_and(|remaining| (0.0..=0.12).contains(&remaining));
    let has_autoplay_successor = playlist_auto_play_next
        && playback
            .playlist
            .iter()
            .position(|item| item.current)
            .is_some_and(|current| current + 1 < playback.playlist.len());
    // Put the retained frame over the native output before mpv starts replacing
    // its D3D swap chain. The short early hold only applies when autoplay has a
    // successor, so the final frame of a last playlist item remains untouched.
    let handoff_early = playback.phase == Phase::Playing
        && has_autoplay_successor
        && remaining
            .is_some_and(|remaining| (0.0..=OUTGOING_COVER_LEAD_SECONDS).contains(&remaining));
    let incoming = if playback.generation > outgoing_generation {
        let incoming_has_video = incoming_has_video(playback);
        if playback.phase == Phase::Loading {
            true
        } else if playback.phase == Phase::Buffering {
            incoming_has_video
                || (!playback.track_list_observed
                    && incoming_capture_fallback_active(playback, now))
        } else if matches!(playback.phase, Phase::Ended | Phase::Stopped) {
            // Some playlist engines report the next START_FILE before the
            // previous END_FILE. That late end notification can temporarily
            // overwrite the incoming phase even though its decoder is still
            // opening. Keep the handoff cover through that event-order gap.
            incoming_capture_fallback_active(playback, now)
                && (incoming_has_video || !playback.track_list_observed)
        } else if matches!(
            playback.phase,
            Phase::Playing | Phase::Paused | Phase::Seeking
        ) {
            if !incoming_has_video {
                !playback.track_list_observed && incoming_capture_fallback_active(playback, now)
            } else if playback.incoming_transition_frame_generation == Some(playback.generation)
                && incoming_frame_is_usable(playback)
            {
                // The native surface has produced a complete frame. Showing
                // that snapshot over live playback creates two moving images;
                // expose the live surface as soon as it passes validation.
                false
            } else {
                // Metadata and time-pos can advance before the child surface
                // presents a decoded frame. Hold the outgoing image until an
                // mpv raw capture succeeds, with a timeout for capture errors.
                incoming_capture_fallback_active(playback, now)
            }
        } else {
            false
        }
    } else {
        false
    };

    // mpv clears time-pos before it announces the next item in some playlists.
    let between_playlist_items = playback.generation == outgoing_generation
        && playback.phase == Phase::Playing
        && playback.position.is_none();
    let current_playlist_index = playback.playlist.iter().position(|item| item.current);
    let waiting_for_playlist_advance = playback.phase == Phase::Ended
        && playlist_auto_play_next
        && current_playlist_index.is_some_and(|index| index + 1 < playback.playlist.len());

    near_end || handoff_early || incoming || between_playlist_items || waiting_for_playlist_advance
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use riina_core::{CapturedFrame, Phase, Playback, PlaylistEntry, VideoParameters};

    use super::{
        outgoing_capture_due, should_display, should_display_at,
        should_display_until_incoming_stable,
    };

    fn frame() -> Arc<CapturedFrame> {
        Arc::new(CapturedFrame::new(2, 1, 8, &[40, 80, 120, 255, 40, 80, 120, 255]).unwrap())
    }

    fn video() -> VideoParameters {
        VideoParameters {
            pixelformat: None,
            w: 640,
            h: 360,
            dw: None,
            dh: None,
            rotate: None,
            colormatrix: None,
            colorlevels: None,
            primaries: None,
            gamma: None,
            max_luma: None,
            min_luma: None,
            signal_peak: None,
        }
    }

    fn outgoing() -> Playback {
        Playback {
            generation: 4,
            phase: Phase::Playing,
            position: Some(4.9),
            duration: Some(5.0),
            video: Some(video()),
            transition_frame: Some(frame()),
            transition_frame_generation: Some(4),
            ..Playback::default()
        }
    }

    #[test]
    fn outgoing_frame_covers_final_eof_window() {
        assert!(should_display(&outgoing(), true));
    }

    #[test]
    fn outgoing_frame_covers_autoplay_handoff_before_native_output_changes() {
        let mut state = outgoing();
        state.position = Some(4.6);
        state.playlist = vec![
            PlaylistEntry {
                id: 1,
                filename: "outgoing.mkv".into(),
                title: None,
                current: true,
                playing: true,
            },
            PlaylistEntry {
                id: 2,
                filename: "incoming.mkv".into(),
                title: None,
                current: false,
                playing: false,
            },
        ];

        assert!(should_display(&state, true));
        assert!(!should_display(&state, false));

        state.position = Some(4.54);
        assert!(!should_display(&state, true));
    }

    #[test]
    fn outgoing_capture_retries_until_a_reply_confirms_the_frame() {
        let state = outgoing();
        let now = Instant::now();
        assert!(outgoing_capture_due(&state, None, None, now));
        assert!(!outgoing_capture_due(
            &state,
            None,
            Some((state.generation, now)),
            now + Duration::from_millis(239),
        ));
        assert!(outgoing_capture_due(
            &state,
            None,
            Some((state.generation, now)),
            now + Duration::from_millis(240),
        ));
        assert!(!outgoing_capture_due(
            &state,
            Some(state.generation),
            None,
            now,
        ));
    }

    #[test]
    fn outgoing_frame_waits_for_a_captured_incoming_frame() {
        let mut state = outgoing();
        state.generation = 5;
        state.position = Some(0.125);
        assert!(should_display(&state, true));
        state.position = Some(0.75);
        assert!(should_display(&state, true));
        state.position = Some(3.001);
        assert!(!should_display(&state, true));
    }

    #[test]
    fn a_black_incoming_capture_keeps_the_outgoing_transition_cover() {
        let mut state = outgoing();
        state.generation = 5;
        state.phase = Phase::Playing;
        state.position = Some(0.5);
        let now = std::time::Instant::now();
        state.incoming_transition_started_at = Some(now);
        state.incoming_transition_frame = Some(Arc::new(
            CapturedFrame::new(64, 36, 64 * 4, &vec![0; 64 * 36 * 4]).unwrap(),
        ));
        state.incoming_transition_frame_generation = Some(5);
        state.incoming_transition_frame_captured_at = Some(std::time::Instant::now());

        assert!(!super::incoming_frame_is_usable(&state));
        assert!(should_display_at(&state, true, now));
        assert!(!should_display_at(
            &state,
            true,
            now + Duration::from_secs(4)
        ));
    }

    #[test]
    fn a_visible_incoming_capture_releases_the_transition_cover() {
        let mut state = outgoing();
        state.generation = 5;
        state.phase = Phase::Playing;
        state.position = Some(0.5);
        let mut pixels = vec![0; 64 * 36 * 4];
        for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel.copy_from_slice(&[40, 80, 120, 255]);
        }
        state.incoming_transition_frame = Some(Arc::new(
            CapturedFrame::new(64, 36, 64 * 4, &pixels).unwrap(),
        ));
        state.incoming_transition_frame_generation = Some(5);
        state.incoming_transition_frame_captured_at = Some(std::time::Instant::now());

        assert!(super::incoming_frame_is_usable(&state));
        assert!(!should_display(&state, true));
    }

    #[test]
    fn partial_incoming_capture_never_replaces_the_outgoing_cover() {
        let mut state = outgoing();
        state.generation = 5;
        state.incoming_transition_frame = Some(Arc::new(
            CapturedFrame::new(64, 36, 64 * 4, &vec![0; 64 * 36 * 4]).unwrap(),
        ));
        state.incoming_transition_frame_generation = Some(5);
        assert!(super::cover_frame(&state).is_some_and(|frame| frame.width == 2));
    }

    #[test]
    fn visible_incoming_capture_keeps_cover_until_surface_stability_is_confirmed() {
        let mut state = outgoing();
        state.generation = 5;
        state.phase = Phase::Playing;
        state.position = Some(0.5);
        state.incoming_transition_frame = Some(frame());
        state.incoming_transition_frame_generation = Some(5);
        state.incoming_transition_started_at = Some(std::time::Instant::now());

        assert!(should_display_until_incoming_stable(&state, true, None));
        assert!(!should_display_until_incoming_stable(&state, true, Some(5)));
    }

    #[test]
    fn incoming_cover_still_obeys_the_capture_timeout_without_a_stable_surface() {
        let now = std::time::Instant::now();
        let mut state = outgoing();
        state.generation = 5;
        state.phase = Phase::Playing;
        state.position = Some(0.5);
        state.incoming_transition_frame = Some(frame());
        state.incoming_transition_frame_generation = Some(5);
        state.incoming_transition_started_at = Some(now - Duration::from_secs(4));

        assert!(!should_display_until_incoming_stable(&state, true, None));
    }

    #[test]
    fn a_narrow_stripe_does_not_count_as_a_presented_incoming_frame() {
        let mut state = outgoing();
        let width = 64;
        let height = 36;
        let mut pixels = vec![0; width * height * 4];
        for y in 0..height {
            for x in 56..width {
                let index = (y * width + x) * 4;
                pixels[index..index + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
        state.incoming_transition_frame = Some(Arc::new(
            CapturedFrame::new(width as u32, height as u32, width * 4, &pixels).unwrap(),
        ));

        assert!(!super::incoming_frame_is_usable(&state));
    }

    #[test]
    fn complete_incoming_frame_is_not_kept_over_live_playback() {
        let mut state = outgoing();
        state.generation = 5;
        state.phase = Phase::Playing;
        state.position = Some(0.4);
        state.incoming_transition_frame = Some(frame());
        state.incoming_transition_frame_generation = Some(5);
        state.incoming_transition_frame_position = Some(0.31);
        assert!(!should_display(&state, true));
    }

    #[test]
    fn complete_incoming_frame_releases_the_cover_at_high_playback_speed() {
        let now = std::time::Instant::now();
        let mut state = outgoing();
        state.generation = 5;
        state.phase = Phase::Playing;
        state.speed = 8.0;
        state.duration = Some(10.0);
        state.position = Some(5.0);
        state.incoming_transition_frame = Some(frame());
        state.incoming_transition_frame_generation = Some(5);
        state.incoming_transition_frame_position = Some(0.3);
        state.incoming_transition_frame_captured_at = Some(now - Duration::from_millis(90));

        assert!(!should_display_at(&state, true, now));
    }

    #[test]
    fn incoming_cover_timeout_uses_wall_clock_for_resumed_media() {
        let now = std::time::Instant::now();
        let mut state = outgoing();
        state.generation = 5;
        state.phase = Phase::Playing;
        state.position = Some(70.0);
        state.duration = Some(120.0);
        state.incoming_transition_started_at = Some(now - Duration::from_millis(2_990));

        assert!(should_display_at(&state, true, now));
        assert!(!should_display_at(
            &state,
            true,
            now + Duration::from_millis(20)
        ));
    }

    #[test]
    fn incoming_cover_waits_while_video_metadata_is_not_observed() {
        let now = std::time::Instant::now();
        let mut state = outgoing();
        state.generation = 5;
        state.phase = Phase::Playing;
        state.position = Some(0.0);
        state.video = None;
        state.incoming_transition_started_at = Some(now - Duration::from_secs(1));

        assert!(should_display_at(&state, true, now));
        state.track_list_observed = true;
        assert!(!should_display_at(&state, true, now));
    }

    #[test]
    fn late_end_file_event_does_not_drop_the_cover_for_an_opening_successor() {
        let now = Instant::now();
        let mut state = outgoing();
        state.generation = 5;
        state.phase = Phase::Ended;
        state.position = None;
        state.incoming_transition_started_at = Some(now);

        assert!(should_display_at(&state, true, now));
        assert!(!should_display_at(
            &state,
            true,
            now + Duration::from_secs(4)
        ));
    }

    #[test]
    fn transient_stopped_event_does_not_drop_the_cover_before_successor_metadata() {
        let now = Instant::now();
        let mut state = outgoing();
        state.generation = 5;
        state.phase = Phase::Stopped;
        state.position = None;
        state.incoming_transition_started_at = Some(now);

        assert!(should_display_at(&state, true, now));
        state.video = None;
        state.track_list_observed = true;
        assert!(!should_display_at(&state, true, now));
    }

    #[test]
    fn playing_frame_metadata_does_not_release_the_transition_cover_early() {
        let mut state = outgoing();
        state.generation = 5;
        state.position = Some(0.1);
        state.video_frame_info = Some(riina_core::VideoFrameInfo {
            interlaced: Some(false),
            top_field_first: None,
        });
        assert!(should_display(&state, true));
        state.paused = true;
        assert!(should_display(&state, true));
    }

    #[test]
    fn incoming_frame_capture_waits_for_the_new_video_surface_to_settle() {
        let mut state = outgoing();
        state.generation = 5;
        state.phase = Phase::Playing;
        state.position = Some(0.1);
        assert!(!super::incoming_capture_ready(&state));
        state.position = Some(0.25);
        assert!(super::incoming_capture_ready(&state));
    }

    #[test]
    fn incoming_cover_stays_until_native_surface_is_stable_too() {
        let since = Instant::now();
        let now = since + Duration::from_millis(450);

        assert!(!super::incoming_cover_release_ready(3, since, now, false));
        assert!(super::incoming_cover_release_ready(3, since, now, true));
        assert!(!super::incoming_cover_release_ready(2, since, now, true));
    }

    #[test]
    fn short_videos_use_a_proportionally_shorter_surface_stability_wait() {
        let mut state = outgoing();
        state.generation = 5;
        state.phase = Phase::Playing;
        state.duration = Some(0.4);
        state.position = Some(0.09);
        assert!(!super::incoming_capture_ready(&state));
        state.position = Some(0.1);
        assert!(super::incoming_capture_ready(&state));
    }

    #[test]
    fn frame_survives_buffering_and_end_file_until_an_autoplay_successor() {
        let mut state = outgoing();
        state.generation = 5;
        state.phase = Phase::Buffering;
        state.position = Some(0.5);
        assert!(should_display(&state, true));

        state = outgoing();
        state.phase = Phase::Ended;
        state.position = None;
        state.playlist = vec![
            PlaylistEntry {
                id: 1,
                filename: "outgoing.mkv".into(),
                title: None,
                current: true,
                playing: false,
            },
            PlaylistEntry {
                id: 2,
                filename: "incoming.mkv".into(),
                title: None,
                current: false,
                playing: false,
            },
        ];
        assert!(should_display(&state, true));
        assert!(!should_display(&state, false));
        state.playlist.pop();
        assert!(!should_display(&state, true));
    }

    #[test]
    fn an_audio_only_successor_does_not_keep_a_video_overlay_after_start() {
        let mut state = outgoing();
        state.generation = 5;
        state.phase = Phase::Playing;
        state.position = Some(0.0);
        state.video = None;
        state.track_list_observed = true;
        assert!(!should_display(&state, true));
    }
}
