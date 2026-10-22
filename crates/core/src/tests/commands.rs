use crate::*;

#[test]
fn play_from_end_seeks_to_start_then_unpauses() {
    let ended = Playback {
        phase: Phase::Ended,
        ..Playback::default()
    };
    assert_eq!(
        Command::TogglePause.plan(&ended).unwrap().commands,
        [["seek", "0", "absolute+exact"], ["set", "pause", "no"],]
    );

    let playing = Playback {
        phase: Phase::Playing,
        ..Playback::default()
    };
    assert_eq!(
        Command::TogglePause.plan(&playing).unwrap().commands,
        [["cycle", "pause"]]
    );
}

#[test]
fn set_paused_command_is_idempotent_and_replays_at_end() {
    let playing = Playback {
        phase: Phase::Playing,
        ..Playback::default()
    };
    assert_eq!(
        Command::SetPaused(true).plan(&playing).unwrap().commands,
        [["set", "pause", "yes"]]
    );
    assert_eq!(
        Command::SetPaused(false).plan(&playing).unwrap().commands,
        [["set", "pause", "no"]]
    );

    let ended = Playback {
        phase: Phase::Ended,
        ..Playback::default()
    };
    assert_eq!(
        Command::SetPaused(false).plan(&ended).unwrap().commands,
        [["seek", "0", "absolute+exact"], ["set", "pause", "no"]]
    );
}

#[test]
fn live_text_frame_capture_requires_a_paused_source_and_packs_rgba_rows() {
    let paused = Playback {
        phase: Phase::Paused,
        ..Playback::default()
    };
    assert_eq!(
        Command::CaptureFrame.plan(&paused).unwrap().commands,
        [["screenshot-raw", "video", "rgba"]]
    );
    assert!(Command::CaptureFrame.plan(&Playback::default()).is_err());

    let frame = CapturedFrame::new(
        2,
        2,
        12,
        &[
            1, 2, 3, 4, 5, 6, 7, 8, 99, 99, 99, 99, 9, 10, 11, 12, 13, 14, 15, 16, 99, 99, 99, 99,
        ],
    )
    .unwrap();
    assert_eq!(frame.width, 2);
    assert_eq!(frame.height, 2);
    assert_eq!(
        &*frame.rgba,
        &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
    );
    assert!(CapturedFrame::new(2, 2, 4, &[0; 16]).is_err());
    assert!(CapturedFrame::new(2, 2, 8, &[0; 15]).is_err());
}

#[test]
fn transition_frame_capture_requires_an_active_video() {
    let state = Playback {
        phase: Phase::Playing,
        video: Some(VideoParameters {
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
        }),
        ..Default::default()
    };
    assert_eq!(
        Command::CaptureTransitionFrame
            .plan(&state)
            .unwrap()
            .commands,
        [["screenshot-raw", "video", "rgba"]]
    );
    assert!(
        Command::CaptureTransitionFrame
            .plan(&Playback::default())
            .is_err()
    );
    assert_eq!(
        Command::CaptureMaterialFrame.plan(&state).unwrap().commands,
        [["screenshot-raw", "video", "rgba"]]
    );
    assert!(
        Command::CaptureMaterialFrame
            .plan(&Playback::default())
            .is_err()
    );
}

#[test]
fn tone_mapping_command_plan_sets_the_live_renderer_options() {
    let selected = hdr_preferences::Settings {
        target_peak_override_enabled: true,
        target_peak_nits: 1000,
        algorithm: hdr_preferences::Algorithm::Bt2390,
        param_override_enabled: true,
        param: -0.25,
        ..Default::default()
    };
    assert_eq!(
        Command::SetToneMapping(selected)
            .plan(&Playback::default())
            .unwrap()
            .commands,
        [
            ["set", "target-peak", "1000"],
            ["set", "tone-mapping", "bt.2390"],
            ["set", "tone-mapping-param", "-0.25"],
        ]
    );
}

#[test]
fn resume_policy_updates_native_save_and_restore_together() {
    assert_eq!(
        Command::SetResumeLastPosition(false)
            .plan(&Playback::default())
            .unwrap()
            .commands,
        [
            vec!["set", "save-position-on-quit", "no"],
            vec!["set", "resume-playback", "no"],
        ]
    );
    assert_eq!(
        Command::SetResumeLastPosition(true)
            .plan(&Playback::default())
            .unwrap()
            .commands,
        [
            vec!["set", "save-position-on-quit", "yes"],
            vec!["set", "resume-playback", "yes"],
        ]
    );
}

#[test]
fn stop_saves_active_resume_position_before_stopping_when_enabled() {
    let mut state = Playback {
        phase: Phase::Playing,
        save_position_on_quit: Some(true),
        ..Playback::default()
    };
    assert_eq!(
        Command::Stop.plan(&state).unwrap().commands,
        [vec!["write-watch-later-config"], vec!["stop"]]
    );

    state.phase = Phase::Paused;
    assert_eq!(
        Command::Stop.plan(&state).unwrap().commands,
        [vec!["write-watch-later-config"], vec!["stop"]]
    );
}

#[test]
fn stop_does_not_write_resume_position_when_disabled_or_inactive() {
    let mut state = Playback {
        phase: Phase::Playing,
        save_position_on_quit: Some(false),
        ..Playback::default()
    };
    assert_eq!(Command::Stop.plan(&state).unwrap().commands, [vec!["stop"]]);

    state.save_position_on_quit = Some(true);
    state.phase = Phase::Loading;
    assert_eq!(Command::Stop.plan(&state).unwrap().commands, [vec!["stop"]]);
}

#[test]
fn ab_loop_points_preserve_zero_and_validate_the_whole_intent() {
    let state = Playback::default();
    assert_eq!(
        Command::SetAbLoop {
            a: Some(0.0),
            b: Some(2.0)
        }
        .plan(&state)
        .unwrap()
        .commands,
        vec![
            vec!["set", "ab-loop-a", "0.000001"],
            vec!["set", "ab-loop-b", "2"]
        ]
    );
    assert!(
        Command::SetAbLoop {
            a: Some(0.0),
            b: Some(f64::NAN)
        }
        .plan(&state)
        .is_err()
    );
    assert!(
        Command::SetAbLoop {
            a: Some(-1.0),
            b: None
        }
        .plan(&state)
        .is_err()
    );
    assert_eq!(
        Command::SetAbLoop { a: None, b: None }
            .plan(&state)
            .unwrap()
            .commands,
        vec![
            vec!["set", "ab-loop-a", "no"],
            vec!["set", "ab-loop-b", "no"]
        ]
    );
    let mut loaded = Playback {
        ab_loop_a: Some(1.0),
        ab_loop_b: Some(2.0),
        ..Default::default()
    };
    loaded.begin_load();
    assert!(loaded.ab_loop_a.is_none() && loaded.ab_loop_b.is_none());
}

#[test]
fn dragging_an_ab_loop_point_updates_only_the_selected_endpoint() {
    let state = Playback {
        ab_loop_a: Some(1.0),
        ab_loop_b: Some(4.0),
        ..Default::default()
    };
    assert_eq!(
        Command::SetAbLoopPoint {
            point: AbLoopPoint::A,
            value: Some(2.5),
        }
        .plan(&state)
        .unwrap()
        .commands,
        [vec!["set", "ab-loop-a", "2.5"]]
    );
    assert_eq!(
        Command::SetAbLoopPoint {
            point: AbLoopPoint::B,
            value: None,
        }
        .plan(&state)
        .unwrap()
        .commands,
        [vec!["set", "ab-loop-b", "no"]]
    );
    assert!(
        Command::SetAbLoopPoint {
            point: AbLoopPoint::A,
            value: Some(f64::INFINITY),
        }
        .plan(&state)
        .is_err()
    );
}

#[test]
fn subtitle_encoding_change_updates_option_then_reloads_each_subtitle_track() {
    fn track(id: i64, kind: TrackKind) -> Track {
        Track {
            id,
            kind,
            title: None,
            lang: None,
            codec: None,
            selected: false,
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
    let state = Playback {
        tracks: vec![
            Track {
                external: true,
                ..track(7, TrackKind::Sub)
            },
            track(9, TrackKind::Audio),
            track(11, TrackKind::Sub),
        ],
        ..Playback::default()
    };

    assert_eq!(
        Command::SubtitleEncoding("UTF-8".into())
            .plan(&state)
            .unwrap()
            .commands,
        vec![
            vec!["set", "sub-codepage", "UTF-8"],
            vec!["sub-reload", "7"],
            vec!["sub-reload", "11"],
        ]
    );
    assert_eq!(
        Command::SubtitleEncoding("auto".into())
            .plan(&Playback::default())
            .unwrap()
            .commands,
        [["set", "sub-codepage", "auto"]]
    );
    assert!(
        Command::SubtitleEncoding("invalid".into())
            .plan(&state)
            .is_err()
    );
}

#[test]
fn subtitle_text_style_commands_use_native_option_names() {
    let cases = [
        (Command::SubtitleBold(true), vec!["set", "sub-bold", "yes"]),
        (
            Command::SubtitleItalic(false),
            vec!["set", "sub-italic", "no"],
        ),
        (Command::SubtitleBlur(2.5), vec!["set", "sub-blur", "2.5"]),
        (
            Command::SubtitleSpacing(-0.25),
            vec!["set", "sub-spacing", "-0.25"],
        ),
        (
            Command::SubtitleShadowOffset(3.0),
            vec!["set", "sub-shadow-offset", "3"],
        ),
    ];
    for (command, expected) in cases {
        assert_eq!(command.mpv_args().unwrap(), expected);
    }
    for command in [
        Command::SubtitleBlur(20.1),
        Command::SubtitleBlur(f64::NAN),
        Command::SubtitleSpacing(f64::INFINITY),
        Command::SubtitleShadowOffset(-1.0),
    ] {
        assert!(command.mpv_args().is_err(), "{command:?}");
    }
}

#[cfg(target_os = "windows")]
#[test]
fn subtitle_reload_recognizes_mpv_normalized_windows_paths() {
    for (observed, requested) in [
        (r"C:\media\captions.srt", r"\\?\C:\media\captions.srt"),
        (
            r"\\server\share\captions.srt",
            r"\\?\UNC\server\share\captions.srt",
        ),
    ] {
        let track = serde_json::from_value(serde_json::json!({
            "id": 7, "type": "sub", "external": true,
            "external-filename": observed,
        }))
        .unwrap();
        let state = Playback {
            tracks: vec![track],
            ..Default::default()
        };
        assert_eq!(
            Command::AddSubtitle(requested.into())
                .plan(&state)
                .unwrap()
                .commands,
            vec![vec!["sub-reload".to_owned(), "7".to_owned()]]
        );
        assert_eq!(
            Command::AddSubtitle(format!("{requested}.other"))
                .plan(&state)
                .unwrap()
                .commands[0][0],
            "sub-add"
        );
    }
}

#[test]
fn discovered_subtitles_use_auto_selection_and_skip_existing_files() {
    let path = "C:/media/subs/Movie.en.srt";
    let command = Command::AddSubtitlesWithoutSelection(vec![path.into()]);
    assert!(command.mpv_args().is_err());
    assert_eq!(
        command.plan(&Playback::default()).unwrap().commands,
        vec![vec!["sub-add", path, "auto"]]
    );

    let track: Track = serde_json::from_value(serde_json::json!({
        "id": 7, "type": "sub", "external": true,
        "external-filename": path,
    }))
    .unwrap();
    let state = Playback {
        tracks: vec![track],
        ..Playback::default()
    };
    assert!(command.plan(&state).unwrap().commands.is_empty());
}

#[test]
fn hdr_output_intent_requires_recognized_hdr_metadata() {
    let source = VideoParameters {
        pixelformat: None,
        w: 1920,
        h: 1080,
        dw: None,
        dh: None,
        rotate: None,
        colormatrix: None,
        colorlevels: None,
        primaries: Some("bt.2020".into()),
        gamma: Some("hlg".into()),
        max_luma: None,
        min_luma: None,
        signal_peak: None,
    };
    let state = Playback {
        phase: Phase::Playing,
        video: Some(source),
        ..Playback::default()
    };
    let plan = Command::SetHdrEnabled(true).plan(&state).unwrap();
    assert_eq!(plan.commands[2], ["set", "target-prim", "bt.2020"]);
    assert_eq!(plan.commands[3], ["set", "target-trc", "pq"]);

    let sdr_state = Playback {
        video: Some(VideoParameters {
            gamma: Some("bt.1886".into()),
            ..state.video.clone().unwrap()
        }),
        ..Playback::default()
    };
    assert!(Command::SetHdrEnabled(true).plan(&sdr_state).is_err());
    assert!(Command::SetHdrEnabled(false).plan(&sdr_state).is_ok());
}

#[test]
fn source_replacement_clears_current_hdr_target_before_loading() {
    let load = Command::Load("next.mkv".into())
        .plan(&Playback::default())
        .unwrap();
    assert_eq!(
        &load.commands[..4],
        [
            ["set", "icc-profile-auto", "yes"],
            ["set", "target-prim", "auto"],
            ["set", "target-trc", "auto"],
            ["set", "screenshot-tag-colorspace", "no"],
        ]
    );
    assert_eq!(
        load.commands.last().unwrap(),
        &["loadfile", "next.mkv", "replace"]
    );

    let batch = Command::OpenFiles(vec!["first.mkv".into(), "second.mkv".into()])
        .plan(&Playback::default())
        .unwrap();
    assert_eq!(&batch.commands[..4], &load.commands[..4]);
    assert_eq!(batch.commands[5], ["loadfile", "first.mkv", "replace"]);
    assert_eq!(batch.commands[6], ["loadfile", "second.mkv", "append"]);
}

#[test]
fn unicode_media_stays_one_argument() {
    let path = r"C:\Фильмы\a b.mkv";
    assert_eq!(Command::Load(path.into()).mpv_args().unwrap()[1], path);
    assert!(Command::Load("a\0b".into()).mpv_args().is_err());
}

#[test]
fn playlist_end_policy_matches_iina_keep_open_precedence() {
    assert_eq!(playlist_end_policy(true, true), "yes");
    assert_eq!(playlist_end_policy(false, true), "no");
    assert_eq!(playlist_end_policy(true, false), "always");
    assert_eq!(playlist_end_policy(false, false), "always");
    assert_eq!(
        Command::SetPlaybackEndPolicy {
            keep_open_on_file_end: true,
            playlist_auto_play_next: false,
        }
        .mpv_args()
        .unwrap(),
        ["set", "keep-open", "always"]
    );
}

#[test]
fn resume_load_uses_native_start_option_and_rejects_invalid_positions() {
    let command = Command::LoadAtPosition {
        path: r"C:\Фильмы\a b.mkv".into(),
        position: 42.5,
    };
    assert_eq!(
        command.mpv_args().unwrap(),
        [
            "loadfile",
            r"C:\Фильмы\a b.mkv",
            "replace",
            "-1",
            "start=42.5"
        ]
    );
    let plan = command.plan(&Playback::default()).unwrap();
    assert_eq!(plan.commands[4], ["set", "pause", "no"]);
    assert_eq!(plan.commands[5][4], "start=42.5");
    for position in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(
            Command::LoadAtPosition {
                path: "movie.mkv".into(),
                position
            }
            .mpv_args()
            .is_err()
        );
    }
    assert!(
        Command::LoadAtPosition {
            path: "bad\0path".into(),
            position: 2.0
        }
        .plan(&Playback::default())
        .is_err()
    );
}

#[test]
fn audio_device_commands_preserve_the_native_id_and_reject_invalid_names() {
    assert_eq!(
        Command::AudioDevice("wasapi/{device-guid}".into())
            .mpv_args()
            .unwrap(),
        ["set", "audio-device", "wasapi/{device-guid}"],
    );
    assert!(Command::AudioDevice(String::new()).mpv_args().is_err());
    assert!(
        Command::AudioDevice("bad\0device".into())
            .mpv_args()
            .is_err()
    );
}

#[test]
fn subtitle_menu_scale_factors_remain_distinct_from_additive_bindings() {
    for (factor, value) in [(1.1, "1.1"), (0.9, "0.9")] {
        assert_eq!(
            Command::MultiplySubtitleScale(factor).mpv_args().unwrap(),
            ["multiply", "sub-scale", value],
        );
    }
    assert_eq!(
        Command::AdjustSubtitleScale(0.1).mpv_args().unwrap(),
        ["add", "sub-scale", "0.1"]
    );
}

#[test]
fn reject_non_finite_commands() {
    assert!(
        Command::Seek {
            seconds: f64::NAN,
            absolute: true
        }
        .mpv_args()
        .is_err()
    );
    assert!(Command::Volume(201.0).mpv_args().is_err());
    assert!(Command::SubtitleFontSize(0.0).mpv_args().is_err());
    assert!(Command::SubtitleFontSize(-1.0).mpv_args().is_err());
    for factor in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(Command::MultiplySpeed(factor).mpv_args().is_err());
        assert!(Command::MultiplySubtitleScale(factor).mpv_args().is_err());
    }
    assert!(
        Command::AdjustVolume {
            delta: f64::NAN,
            boost: false
        }
        .plan(&Playback::default())
        .is_err()
    );
    assert!(Command::AdjustAudioDelay(f64::NAN).mpv_args().is_err());
    assert!(
        Command::AdjustAudioDelay(f64::NEG_INFINITY)
            .mpv_args()
            .is_err()
    );
    assert!(Command::AdjustSubtitleDelay(f64::NAN).mpv_args().is_err());
    assert!(Command::AdjustSubtitleScale(f64::NAN).mpv_args().is_err());
    assert!(
        Command::AdjustSubtitlePosition(f64::INFINITY)
            .mpv_args()
            .is_err()
    );
    assert!(
        Command::AdjustSubtitleDelay(f64::INFINITY)
            .mpv_args()
            .is_err()
    );
}

#[test]
fn scroll_seek_uses_the_selected_keyframe_or_exact_mode() {
    for (seek_type, mode) in [
        (control_preferences::SeekType::Keyframe, "relative"),
        (control_preferences::SeekType::Exact, "relative+exact"),
        (control_preferences::SeekType::Auto, "relative"),
    ] {
        let plan = Command::SeekWithType {
            seconds: 2.5,
            seek_type,
        }
        .plan(&Playback::default())
        .unwrap();
        assert_eq!(plan.commands, [["seek", "2.5", mode]]);
    }
}

#[test]
fn absolute_subtitle_position_preserves_lane_and_validates_engine_range() {
    for value in [0.0, 100.0, 150.0] {
        for (command, property) in [
            (Command::SubtitlePosition(value), "sub-pos"),
            (
                Command::SecondarySubtitlePosition(value),
                "secondary-sub-pos",
            ),
        ] {
            assert_eq!(
                command.mpv_args().unwrap(),
                ["set", property, &value.to_string()]
            );
        }
    }
    for value in [-1.0, 151.0, f64::NAN, f64::INFINITY] {
        assert!(Command::SubtitlePosition(value).mpv_args().is_err());
        assert!(
            Command::SecondarySubtitlePosition(value)
                .mpv_args()
                .is_err()
        );
    }
}

#[test]
fn invalid_batch_is_rejected_before_planning_any_append() {
    assert!(
        Command::AppendFiles(vec!["first.mkv".into(), "bad\0name".into()])
            .plan(&Playback::default())
            .is_err()
    );
    assert!(
        Command::OpenFiles(vec!["first.mkv".into(), "bad\0name".into()])
            .plan(&Playback::default())
            .is_err()
    );
    assert!(
        Command::OpenFiles(Vec::new())
            .plan(&Playback::default())
            .is_err()
    );
}

#[test]
fn chapter_selection_resumes_and_playlist_ids_resolve_after_reorder() {
    let state = Playback {
        chapters: vec![Chapter {
            time: 2.0,
            title: None,
        }],
        playlist: vec![
            PlaylistEntry {
                id: 41,
                filename: "b".into(),
                title: None,
                current: true,
                playing: true,
            },
            PlaylistEntry {
                id: 40,
                filename: "a".into(),
                title: None,
                current: false,
                playing: false,
            },
        ],
        ..Default::default()
    };
    let chapter = Command::SelectChapter(0).plan(&state).unwrap();
    assert_eq!(
        chapter.commands,
        vec![vec!["seek", "2", "absolute"], vec!["set", "pause", "no"]]
    );
    let selected = Command::SelectPlaylistId(40).plan(&state).unwrap();
    assert_eq!(selected.commands[1], vec!["playlist-play-index", "1"]);
    assert!(selected.resume_on_load);
    assert!(Command::SelectPlaylistId(99).plan(&state).is_err());
    assert_eq!(
        Command::MovePlaylistId {
            id: 41,
            direction: 1
        }
        .plan(&state)
        .unwrap()
        .commands[0],
        vec!["playlist-move", "0", "2"]
    );
}

#[test]
fn removing_selected_playlist_ids_uses_original_order_and_rejects_stale_or_duplicate_ids() {
    let state = Playback {
        playlist: [10, 20, 30, 40, 50]
            .into_iter()
            .map(|id| PlaylistEntry {
                id,
                filename: format!("{id}.mkv"),
                title: None,
                current: id == 30,
                playing: id == 30,
            })
            .collect(),
        ..Default::default()
    };

    assert_eq!(
        Command::RemovePlaylistIds(vec![50, 20])
            .plan(&state)
            .unwrap()
            .commands,
        vec![vec!["playlist-remove", "1"], vec!["playlist-remove", "3"]]
    );
    assert!(
        Command::RemovePlaylistIds(vec![20, 20])
            .plan(&state)
            .is_err()
    );
    assert!(Command::RemovePlaylistIds(vec![99]).plan(&state).is_err());
    assert!(
        Command::RemovePlaylistIds(Vec::new())
            .plan(&state)
            .unwrap()
            .commands
            .is_empty()
    );
}

#[test]
fn playlist_sort_plans_keep_native_entry_identity_and_current_item() {
    let state = Playback {
        playlist: vec![
            PlaylistEntry {
                id: 41,
                filename: "D:/media/zeta.mkv".into(),
                title: None,
                current: true,
                playing: true,
            },
            PlaylistEntry {
                id: 40,
                filename: "C:/media/alpha.mkv".into(),
                title: None,
                current: false,
                playing: false,
            },
        ],
        ..Default::default()
    };
    assert_eq!(
        Command::SortPlaylist(playlist::PlaylistSort::FilenameAscending)
            .plan(&state)
            .unwrap()
            .commands,
        vec![vec!["playlist-move", "1", "0"]]
    );
    assert!(
        Command::SortPlaylist(playlist::PlaylistSort::PathAscending)
            .plan(&Playback::default())
            .unwrap()
            .commands
            .is_empty()
    );
    assert_eq!(
        Command::TogglePlaylistShuffle
            .plan(&Playback::default())
            .unwrap()
            .commands,
        vec![vec!["playlist-shuffle"]]
    );
    assert_eq!(
        Command::TogglePlaylistShuffle
            .plan(&Playback {
                playlist_shuffled: true,
                ..Default::default()
            })
            .unwrap()
            .commands,
        vec![vec!["playlist-unshuffle"]]
    );
}

#[test]
fn plugin_mpv_property_observation_is_validated_and_has_no_input_command() {
    let command = Command::ObserveMpvProperty("cache-buffering-state".into());
    assert!(
        command
            .plan(&Playback::default())
            .unwrap()
            .commands
            .is_empty()
    );
    assert!(
        Command::ObserveMpvProperty(String::new())
            .plan(&Playback::default())
            .is_err()
    );
    assert!(
        Command::ObserveMpvProperty("property.with.dot".into())
            .plan(&Playback::default())
            .is_err()
    );
    assert!(
        Command::ObserveMpvProperty("bad\0property".into())
            .plan(&Playback::default())
            .is_err()
    );
}
