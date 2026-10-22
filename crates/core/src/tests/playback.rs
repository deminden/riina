use crate::*;
use std::sync::Arc;

#[test]
fn hdr_classification_matches_pinned_source_without_inventing_metadata() {
    for (gamma, primaries, expected) in [
        (Some("pq"), Some("bt.2020"), true),
        (Some("hlg"), Some("display-p3"), true),
        (Some("pq"), Some("bt.709"), false),
        (Some("srgb"), Some("bt.2020"), false),
        (Some("pq"), None, false),
        (None, Some("bt.2020"), false),
    ] {
        let value = serde_json::json!({"w":192,"h":108,"gamma":gamma,"primaries":primaries});
        let parameters: VideoParameters = serde_json::from_value(value).unwrap();
        assert_eq!(parameters.is_hdr(), expected);
    }
}

#[test]
fn unknown_time_is_not_zero() {
    assert_eq!(time_label(None), "--:--");
    assert_eq!(time_label(Some(3661.9)), "1:01:01");
}

#[test]
fn subtitle_style_eligibility_combines_both_selected_lanes() {
    for (tracks, text_controls, ass_warning) in [
        ("[]", true, false),
        (
            r#"[{"id":1,"type":"sub","codec":"hdmv_pgs_subtitle","selected":true,"main-selection":0}]"#,
            false,
            false,
        ),
        (
            r#"[{"id":1,"type":"sub","codec":"dvb_subtitle","selected":true,"main-selection":1}]"#,
            false,
            false,
        ),
        (
            r#"[{"id":1,"type":"sub","codec":"hdmv_pgs_subtitle","selected":true,"main-selection":0},{"id":2,"type":"sub","codec":"subrip","selected":true,"main-selection":1}]"#,
            true,
            false,
        ),
        (
            r#"[{"id":1,"type":"sub","codec":"ass","selected":true,"main-selection":1}]"#,
            true,
            true,
        ),
        (
            r#"[{"id":1,"type":"sub","codec":"ass","selected":false},{"id":2,"type":"audio","codec":"ass","selected":true,"main-selection":0}]"#,
            true,
            false,
        ),
        (
            r#"[{"id":1,"type":"sub","selected":true,"main-selection":0}]"#,
            true,
            false,
        ),
    ] {
        let state = Playback {
            tracks: serde_json::from_str(tracks).unwrap(),
            ..Playback::default()
        };
        assert_eq!(
            state.subtitle_style_availability(),
            SubtitleStyleAvailability {
                text_controls,
                ass_warning
            },
            "{tracks}"
        );
    }
}

#[test]
fn loading_clears_previous_media_clock() {
    let mut state = Playback {
        position: Some(99.0),
        duration: Some(100.0),
        ..Default::default()
    };
    state.begin_load();
    assert_eq!(state.generation, 1);
    assert_eq!(state.position, None);
    assert_eq!(state.phase, Phase::Loading);
}

#[test]
fn playlist_handoff_retains_only_the_current_file_snapshot() {
    let frame = Arc::new(CapturedFrame::new(1, 1, 4, &[1, 2, 3, 4]).unwrap());
    let mut state = Playback {
        transition_frame: Some(frame.clone()),
        transition_frame_generation: Some(0),
        ..Default::default()
    };
    state.begin_load();
    assert_eq!(state.transition_frame_generation, Some(0));
    assert_eq!(state.transition_frame, Some(frame));

    state.transition_frame_generation = Some(0);
    state.begin_load();
    assert!(state.transition_frame_generation.is_none());
    assert!(state.transition_frame.is_none());
}

#[test]
fn buffering_pause_and_seek_have_distinct_states() {
    let mut state = Playback {
        phase: Phase::Playing,
        paused: true,
        buffering: true,
        ..Default::default()
    };
    state.reconcile();
    assert_eq!(state.phase, Phase::Buffering);
    state.buffering = false;
    state.reconcile();
    assert_eq!(state.phase, Phase::Paused);
    state.seeking = true;
    state.reconcile();
    assert_eq!(state.phase, Phase::Seeking);
}
