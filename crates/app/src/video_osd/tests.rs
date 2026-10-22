use super::*;
fn loaded() -> Playback {
    Playback {
        generation: 1,
        phase: Phase::Paused,
        speed: 1.0,
        video_equalizer: [Some(0); 5],
        hardware_decoding: Some("auto".into()),
        ..Default::default()
    }
}

#[test]
fn explicit_progress_feedback_uses_precise_clock_and_bounded_progress() {
    let message = progress_message(Some(3661.2345), Some(3662.0)).unwrap();
    assert_eq!(message.kind, Kind::Progress);
    assert_eq!(message.text, "1:01:01.235 / 1:01:02.000");
    assert_eq!(message.progress, Some((3661.2345_f64 / 3662.0) as f32));

    let beyond_end = progress_message(Some(20.0), Some(10.0)).unwrap();
    assert_eq!(beyond_end.progress, Some(1.0));
    assert_eq!(progress_message(Some(1.0), None), None);
    assert_eq!(progress_message(Some(f64::NAN), Some(2.0)), None);
    assert_eq!(progress_message(Some(0.0), Some(0.0)), None);
}

#[test]
fn accepted_action_feedback_uses_reference_stop_chapter_and_playlist_messages() {
    for (action, kind, text) in [
        (ActionFeedback::Stop, Kind::Stop, "Stop".to_owned()),
        (
            ActionFeedback::Chapter("Opening".into()),
            Kind::Chapter,
            "Chapter: Opening".to_owned(),
        ),
        (
            ActionFeedback::PlaylistAdded(3),
            Kind::PlaylistAdd,
            "Added 3 Files to Playlist".to_owned(),
        ),
        (
            ActionFeedback::PlaylistCleared,
            Kind::PlaylistClear,
            "Cleared Playlist".to_owned(),
        ),
    ] {
        let message = action_message(action);
        assert_eq!(message.kind, kind);
        assert_eq!(message.text, text);
        assert_eq!(message.progress, None);
    }
}

#[test]
fn disabling_speed_feedback_removes_only_queued_speed_messages() {
    let mut feedback = VideoOsd::default();
    let mut state = loaded();
    feedback.observe(&state);
    state.volume += 5.0;
    state.speed = 1.5;
    state.hardware_decoding = Some("no".into());
    assert_eq!(feedback.observe(&state).unwrap().kind, Kind::Volume);
    feedback.set_disable_speed(true);
    assert_eq!(
        feedback.take_pending().unwrap().kind,
        Kind::HardwareDecoding
    );
    feedback.set_disable_speed(false);
    assert_eq!(feedback.take_pending(), None);
}

#[test]
fn disabling_pause_feedback_removes_queued_pause_messages() {
    let mut feedback = VideoOsd::default();
    let mut state = loaded();
    feedback.observe(&state);
    state.aspect_requests += 1;
    state.paused = !state.paused;
    state.volume += 5.0;
    assert_eq!(feedback.observe(&state).unwrap().kind, Kind::Aspect);
    feedback.set_disable_pause_resume(true);
    assert_eq!(feedback.take_pending().unwrap().kind, Kind::Volume);
    feedback.set_disable_pause_resume(false);
    assert_eq!(feedback.take_pending(), None);
}

#[test]
fn coalesced_ab_loop_endpoints_each_receive_feedback() {
    let mut feedback = VideoOsd::default();
    let mut state = loaded();
    assert_eq!(feedback.observe(&state), None);

    state.ab_loop_a = Some(3.0);
    state.ab_loop_b = Some(9.0);
    assert_eq!(feedback.observe(&state).unwrap().text, "AB-Loop: A 0:03");
    assert_eq!(feedback.take_pending().unwrap().text, "AB-Loop: B 0:09");
    assert_eq!(feedback.take_pending(), None);
}

#[test]
fn volume_feedback_progress_tracks_the_default_and_boost_caps() {
    let mut feedback = VideoOsd::default();
    let mut state = loaded();
    state.volume = 40.0;
    assert_eq!(feedback.observe(&state), None);

    state.volume = 100.0;
    let default_cap = feedback.observe(&state).unwrap();
    assert_eq!(default_cap.kind, Kind::Volume);
    assert_eq!(default_cap.text, "Volume: 100");
    assert_eq!(default_cap.progress, Some(1.0));

    feedback.set_volume_limit(200.0);
    for (volume, progress) in [(125.0, 0.625), (200.0, 1.0), (240.0, 1.0)] {
        state.volume = volume;
        let message = feedback.observe(&state).unwrap();
        assert_eq!(message.kind, Kind::Volume);
        assert_eq!(message.text, format!("Volume: {volume:.0}"));
        assert_eq!(message.progress, Some(progress));
    }
}

#[test]
fn simultaneous_ab_loop_clear_is_reported_once() {
    let mut feedback = VideoOsd::default();
    let mut state = loaded();
    state.ab_loop_a = Some(3.0);
    state.ab_loop_b = Some(9.0);
    assert_eq!(feedback.observe(&state), None);

    state.ab_loop_a = None;
    state.ab_loop_b = None;
    assert_eq!(feedback.observe(&state).unwrap().text, "AB-Loop: Cleared");
    assert_eq!(feedback.take_pending(), None);
}

#[test]
fn feedback_requires_confirmed_changes_in_the_same_media_generation() {
    let old = loaded();
    let mut state = old.clone();
    assert_eq!(changed(None, &state), None);
    assert_eq!(changed(Some(&old), &state), None);
    state.speed = 1.234567;
    assert_eq!(
        changed(Some(&old), &state),
        Some(Message {
            kind: Kind::Speed,
            text: "Speed: 1.234567x".into(),
            progress: None
        })
    );
    state.generation += 1;
    assert_eq!(changed(Some(&old), &state), None);
    state.generation = old.generation;
    state.error = Some("Rejected command".into());
    assert_eq!(changed(Some(&old), &state), None);
}

#[test]
fn crop_feedback_reports_confirmed_presets_and_custom_rectangle_changes() {
    use riina_core::crop::{FILTER_LABEL, VideoFilter};

    let mut feedback = VideoOsd::default();
    let mut state = loaded();
    state.video = Some(serde_json::from_value(serde_json::json!({"w": 320, "h": 180})).unwrap());
    state.video_filters = Some(vec![]);
    assert_eq!(feedback.observe(&state), None);

    state.video_filters = Some(vec![VideoFilter {
        name: "crop".into(),
        label: Some(FILTER_LABEL.into()),
        enabled: Some(true),
        params: [("w".into(), "240".into()), ("h".into(), "180".into())].into(),
    }]);
    assert_eq!(feedback.observe(&state).unwrap().text, "Crop: 4:3");

    state.video_filters = Some(vec![VideoFilter {
        name: "crop".into(),
        label: Some(FILTER_LABEL.into()),
        enabled: Some(true),
        params: [
            ("w".into(), "300".into()),
            ("h".into(), "170".into()),
            ("x".into(), "20".into()),
            ("y".into(), "10".into()),
        ]
        .into(),
    }]);
    assert_eq!(
        feedback.observe(&state).unwrap().text,
        "Crop: (20, 10) (300×170)"
    );

    state.video_filters.as_mut().unwrap()[0]
        .params
        .insert("x".into(), "21".into());
    assert_eq!(
        feedback.observe(&state).unwrap().text,
        "Crop: (21, 10) (300×170)"
    );

    state.video_filters = Some(vec![]);
    assert_eq!(feedback.observe(&state).unwrap().text, "Crop: None");
    assert_eq!(feedback.observe(&state), None);
}

#[test]
fn crop_feedback_does_not_treat_unknown_filter_state_as_no_crop() {
    use riina_core::crop::{FILTER_LABEL, VideoFilter};

    let mut feedback = VideoOsd::default();
    let mut state = loaded();
    state.video = Some(serde_json::from_value(serde_json::json!({"w": 320, "h": 180})).unwrap());
    assert_eq!(feedback.observe(&state), None);

    state.video_filters = Some(vec![VideoFilter {
        name: "crop".into(),
        label: Some(FILTER_LABEL.into()),
        enabled: Some(true),
        params: [
            ("w".into(), "240".into()),
            ("h".into(), "180".into()),
            ("x".into(), "0".into()),
            ("y".into(), "0".into()),
        ]
        .into(),
    }]);
    assert_eq!(feedback.observe(&state), None);
}

#[test]
fn feedback_consumes_each_observation_independently_of_catalog_updates() {
    let mut feedback = VideoOsd::default();
    let mut state = loaded();
    assert_eq!(feedback.observe(&state), None);
    state.video_equalizer[0] = Some(-27);
    assert_eq!(feedback.observe(&state).unwrap().text, "Brightness: -27");
    assert_eq!(feedback.observe(&state), None);
    state.speed = 2.0;
    assert_eq!(feedback.observe(&state).unwrap().text, "Speed: 2x");
    state.position = Some(4.0);
    assert_eq!(feedback.observe(&state), None);
}

#[test]
fn pause_resume_messages_follow_confirmed_playback_state_and_can_be_suppressed() {
    let mut feedback = VideoOsd::new(false, false);
    let mut state = loaded();
    assert_eq!(feedback.observe(&state), None);
    state.paused = true;
    assert_eq!(feedback.observe(&state).unwrap().text, "Paused");
    state.paused = false;
    assert_eq!(feedback.observe(&state).unwrap().text, "Resumed");

    feedback.set_disable_pause_resume(true);
    state.paused = true;
    assert_eq!(feedback.observe(&state), None);
    state.video_equalizer[0] = Some(-27);
    assert_eq!(feedback.observe(&state).unwrap().kind, Kind::Equalizer);
    feedback.set_disable_pause_resume(false);
    assert_eq!(feedback.observe(&state), None);
}

#[test]
fn file_start_feedback_obeys_global_and_category_suppression() {
    assert!(file_start_message("clip.mkv", true, false).is_some());
    assert!(file_start_message("clip.mkv", false, false).is_none());
    assert!(file_start_message("clip.mkv", true, true).is_none());
    assert!(file_start_message("", true, false).is_none());
}

#[test]
fn speed_suppression_consumes_speed_but_keeps_other_feedback() {
    let mut feedback = VideoOsd::default();
    let mut state = loaded();
    assert_eq!(feedback.observe(&state), None);
    feedback.set_disable_speed(true);
    state.speed = 2.0;
    assert_eq!(feedback.observe(&state), None);
    state.video_equalizer[0] = Some(-27);
    assert_eq!(feedback.observe(&state).unwrap().kind, Kind::Equalizer);
    state.hardware_decoding = Some("no".into());
    state.speed = 3.0;
    assert_eq!(
        feedback.observe(&state).unwrap().text,
        "Hardware Decoding: Off"
    );
    feedback.set_disable_speed(false);
    assert_eq!(feedback.observe(&state), None);
    state.speed = 1.0;
    assert_eq!(feedback.observe(&state).unwrap().kind, Kind::Speed);
}
#[test]
fn equalizer_progress_and_hardware_policy_keep_distinct_semantics() {
    let old = loaded();
    let mut state = old.clone();
    for (value, progress) in [(-100, 0.0), (0, 0.5), (100, 1.0)] {
        let mut previous = old.clone();
        previous.video_equalizer[0] = Some(1);
        state.video_equalizer[0] = Some(value);
        assert_eq!(
            changed(Some(&previous), &state),
            Some(Message {
                kind: Kind::Equalizer,
                text: format!("Brightness: {value}"),
                progress: Some(progress)
            })
        );
    }
    state = old.clone();
    state.hardware_decoder = Some("no".into());
    assert_eq!(changed(Some(&old), &state), None);
    state.hardware_decoding = Some("no".into());
    assert_eq!(
        changed(Some(&old), &state).unwrap().text,
        "Hardware Decoding: Off"
    );
    state.hardware_decoding = Some("auto-copy".into());
    assert_eq!(
        changed(Some(&old), &state).unwrap().text,
        "Hardware Decoding: On"
    );
}
#[test]
fn aspect_feedback_retains_raw_text_repeats_and_native_failures() {
    use riina_core::video_controls::AspectRatio;
    let mut feedback = VideoOsd::default();
    let mut state = loaded();
    assert_eq!(feedback.observe(&state), None);
    for text in ["8:6", "8:6", "Default", "invalid", "١٦:٩"] {
        state.video_aspect = AspectRatio::from_text(text);
        state.aspect_requests += 1;
        state.error = Some("Native parse failure".into());
        assert_eq!(
            feedback.observe(&state),
            Some(Message {
                kind: Kind::Aspect,
                text: format!("Aspect Ratio: {text}"),
                progress: None,
            })
        );
        assert_eq!(feedback.observe(&state), None);
    }
    state.generation += 1;
    state.aspect_requests += 1;
    assert_eq!(feedback.observe(&state), None);
    assert_eq!(feedback.observe(&state), None);
}

#[test]
fn deinterlace_feedback_uses_confirmed_changes_not_filter_activity() {
    let mut feedback = VideoOsd::default();
    let mut state = loaded();
    state.deinterlace = Some(false);
    assert_eq!(feedback.observe(&state), None);
    for (enabled, label) in [(true, "On"), (false, "Off")] {
        state.deinterlace = Some(enabled);
        assert_eq!(
            feedback.observe(&state),
            Some(Message {
                kind: Kind::Deinterlace,
                text: format!("Deinterlace: {label}"),
                progress: None,
            })
        );
        assert_eq!(feedback.observe(&state), None);
        state.deinterlace_active = Some(!enabled);
        assert_eq!(feedback.observe(&state), None);
    }
    state.deinterlace = None;
    assert_eq!(feedback.observe(&state), None);
    state.deinterlace = Some(true);
    state.error = Some("Native command failure".into());
    assert_eq!(feedback.observe(&state), None);
    state.error = None;
    assert_eq!(feedback.observe(&state), None);
    state.generation += 1;
    state.deinterlace = Some(false);
    assert_eq!(feedback.observe(&state), None);
}

#[test]
fn delay_feedback_preserves_lane_direction_and_progress() {
    let old = loaded();
    let mut state = old.clone();
    state.audio_delay = -0.5;
    let audio = changed(Some(&old), &state).unwrap();
    assert_eq!(audio.kind, Kind::AudioDelay);
    assert_eq!(audio.text, "Audio Delay: 0.5s Earlier");
    assert_eq!(audio.progress, Some(0.475));

    let mut old = loaded();
    old.subtitle_delay = 0.0;
    let mut state = old.clone();
    state.subtitle_delay = 1.25;
    let primary = changed(Some(&old), &state).unwrap();
    assert_eq!(primary.kind, Kind::SubtitleDelay);
    assert_eq!(primary.text, "Subtitle Delay: 1.25s Later");
    assert_eq!(primary.progress, Some(0.5625));

    old.secondary_subtitle_delay = Some(0.0);
    state = old.clone();
    state.secondary_subtitle_delay = Some(-2.0);
    let secondary = changed(Some(&old), &state).unwrap();
    assert_eq!(secondary.kind, Kind::SecondarySubtitleDelay);
    assert_eq!(secondary.text, "Secondary Subtitle Delay: 2s Earlier");
    assert_eq!(secondary.progress, Some(0.4));
}

#[test]
fn subtitle_position_feedback_keeps_primary_and_secondary_lanes_distinct() {
    let mut old = loaded();
    old.subtitle_position = Some(50.0);
    let mut state = old.clone();
    state.subtitle_position = Some(75.0);
    let primary = changed(Some(&old), &state).unwrap();
    assert_eq!(primary.kind, Kind::SubtitlePosition);
    assert_eq!(primary.text, "Subtitle Position: 75");
    assert_eq!(primary.progress, Some(0.75));

    old.secondary_subtitle_position = Some(30.0);
    state = old.clone();
    state.secondary_subtitle_position = Some(65.0);
    let secondary = changed(Some(&old), &state).unwrap();
    assert_eq!(secondary.kind, Kind::SecondarySubtitlePosition);
    assert_eq!(secondary.text, "Secondary Subtitle Position: 65");
    assert_eq!(secondary.progress, Some(0.65));
}

#[test]
fn subtitle_visibility_feedback_identifies_the_changed_lane() {
    let old = loaded();
    let mut state = old.clone();
    state.subtitle_visible = true;
    let primary = changed(Some(&old), &state).unwrap();
    assert_eq!(primary.kind, Kind::SubtitleVisibility);
    assert_eq!(primary.text, "Subtitles Visible");

    let mut old = loaded();
    old.secondary_subtitle_visible = Some(false);
    let mut state = old.clone();
    state.secondary_subtitle_visible = Some(true);
    let secondary = changed(Some(&old), &state).unwrap();
    assert_eq!(secondary.kind, Kind::SecondarySubtitleVisibility);
    assert_eq!(secondary.text, "Second Subtitles Visible");
}

#[test]
fn track_feedback_uses_selected_track_identity_and_empty_choice() {
    fn audio_track(id: i64, title: &str) -> riina_core::Track {
        riina_core::Track {
            id,
            kind: riina_core::TrackKind::Audio,
            title: Some(title.into()),
            lang: None,
            codec: None,
            selected: true,
            main_selection: None,
            external: false,
            albumart: false,
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

    let mut old = loaded();
    old.tracks = vec![audio_track(1, "Main")];
    let mut state = old.clone();
    state.tracks = vec![audio_track(2, "Director Commentary")];
    let selected = changed(Some(&old), &state).unwrap();
    assert_eq!(selected.kind, Kind::Track);
    assert_eq!(selected.text, "Audio: #2 Director Commentary");

    state.tracks.clear();
    let disabled = changed(Some(&old), &state).unwrap();
    assert_eq!(disabled.kind, Kind::Track);
    assert_eq!(disabled.text, "Audio: #0 <None>");
}

#[test]
fn rotation_subtitle_scale_and_loop_feedback_use_observed_values() {
    let mut old = loaded();
    old.video_rotation = Some(0);
    let mut state = old.clone();
    state.video_rotation = Some(90);
    let rotation = changed(Some(&old), &state).unwrap();
    assert_eq!(rotation.kind, Kind::Rotation);
    assert_eq!(rotation.text, "Rotate: 90°");

    old.video_rotation = state.video_rotation;
    old.subtitle_scale = Some(1.0);
    state = old.clone();
    state.subtitle_scale = Some(0.5);
    let scale = changed(Some(&old), &state).unwrap();
    assert_eq!(scale.kind, Kind::SubtitleScale);
    assert_eq!(scale.text, "Subtitle Scale: -2x");

    old.subtitle_scale = state.subtitle_scale;
    old.loop_file = Some("no".into());
    old.loop_playlist = Some("no".into());
    state = old.clone();
    state.loop_playlist = Some("inf".into());
    let looping = changed(Some(&old), &state).unwrap();
    assert_eq!(looping.kind, Kind::LoopMode);
    assert_eq!(looping.text, "Loop: Playlist");
}
