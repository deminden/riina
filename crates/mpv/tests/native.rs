//! Run explicitly: cargo test -p riina-mpv --test native --locked -- --ignored --nocapture
use riina_core::{Command, Phase, Playback, TrackKind};
use riina_mpv::Player;
use std::{
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

fn test_library(root: &std::path::Path) -> PathBuf {
    if let Some(path) = std::env::var_os("RIINA_TEST_MPV_LIBRARY") {
        let path = PathBuf::from(path);
        assert!(
            path.is_absolute(),
            "The selected test library must have an absolute path"
        );
        return path
            .canonicalize()
            .expect("The selected test library must exist");
    }
    #[cfg(target_os = "windows")]
    return root.join("tools/mpv/libmpv-2.dll");
    #[cfg(not(target_os = "windows"))]
    {
        let _ = root;
        panic!("Set RIINA_TEST_MPV_LIBRARY to the absolute path of a provisioned libmpv library");
    }
}

#[test]
#[ignore = "requires provisioned libmpv; verifies native cycling of video audio and external subtitle tracks"]
fn track_cycling_includes_disabled_and_external_tracks() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let fixture = |name: &str| {
        root.join("tests/fixtures")
            .join(name)
            .to_string_lossy()
            .into_owned()
    };
    let (sender, receiver) = mpsc::channel();
    let player = Player::start(&test_library(&root), true, move |state| {
        let _ = sender.send(state);
    })
    .unwrap();
    player.send(Command::Load(fixture("catalog.mkv"))).unwrap();
    let loaded = wait_for(&receiver, |state| {
        state.phase == Phase::Playing
            && state
                .tracks
                .iter()
                .any(|t| t.kind == TrackKind::Audio && t.selected)
    });
    let internal_audio = selected_track(&loaded, TrackKind::Audio).unwrap();
    let video = selected_track(&loaded, TrackKind::Video).unwrap();
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| state.paused);
    player
        .send(Command::AddAudio(fixture("external-tone.wav")))
        .unwrap();
    let external = wait_for(&receiver, |state| {
        state
            .tracks
            .iter()
            .any(|t| t.kind == TrackKind::Audio && t.external && t.selected)
    });
    let external_audio = selected_track(&external, TrackKind::Audio).unwrap();
    assert_ne!(internal_audio, external_audio);
    for expected in [None, Some(internal_audio), Some(external_audio)] {
        player.send(Command::CycleTrack(TrackKind::Audio)).unwrap();
        let cycled = wait_for(&receiver, |state| {
            selected_track(state, TrackKind::Audio) == expected
        });
        assert!(cycled.paused);
        assert_eq!(cycled.error, None);
        assert_eq!(selected_track(&cycled, TrackKind::Video), Some(video));
    }
    player
        .send(Command::AddSubtitle(fixture("manual-captions.srt")))
        .unwrap();
    let subtitle = wait_for(&receiver, |state| {
        state
            .tracks
            .iter()
            .any(|t| t.kind == TrackKind::Sub && t.external && t.selected)
    });
    let subtitle_id = selected_track(&subtitle, TrackKind::Sub).unwrap();
    for expected in [None, Some(subtitle_id)] {
        player.send(Command::CycleTrack(TrackKind::Sub)).unwrap();
        let cycled = wait_for(&receiver, |state| {
            selected_track(state, TrackKind::Sub) == expected
        });
        assert!(cycled.paused);
        assert_eq!(cycled.error, None);
        assert_eq!(
            selected_track(&cycled, TrackKind::Audio),
            Some(external_audio)
        );
    }
    for expected in [None, Some(video)] {
        player.send(Command::CycleTrack(TrackKind::Video)).unwrap();
        let cycled = wait_for(&receiver, |state| {
            selected_track(state, TrackKind::Video) == expected
        });
        assert!(cycled.paused);
        assert_eq!(cycled.error, None);
        assert_eq!(
            selected_track(&cycled, TrackKind::Audio),
            Some(external_audio)
        );
        assert_eq!(selected_track(&cycled, TrackKind::Sub), Some(subtitle_id));
    }
}

fn selected_track(state: &Playback, kind: TrackKind) -> Option<i64> {
    state
        .tracks
        .iter()
        .find(|t| t.kind == kind && t.selected)
        .map(|t| t.id)
}

#[test]
#[ignore = "requires provisioned libmpv; verifies file repeat playback and independent playlist repeat"]
fn file_repeat_wraps_and_preserves_playlist_repeat() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let (sender, receiver) = mpsc::channel();
    let player = Player::start(&test_library(&root), true, move |state| {
        let _ = sender.send(state);
    })
    .unwrap();
    player
        .send(Command::Load(
            root.join("tests/fixtures/catalog.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    wait_for(&receiver, |state| {
        state.phase == Phase::Playing && state.position.is_some_and(|p| p > 0.1)
    });
    player.send(Command::TogglePause).unwrap();
    let paused = wait_for(&receiver, |state| state.paused);
    player.send(Command::CycleLoop).unwrap();
    wait_for(&receiver, |state| state.loop_file.as_deref() == Some("inf"));
    player.send(Command::CycleLoop).unwrap();
    let playlist_repeat = wait_for(&receiver, |state| {
        state.loop_file.as_deref() == Some("no") && state.loop_playlist.as_deref() == Some("inf")
    });
    assert!(playlist_repeat.paused);
    player.send(Command::ToggleFileLoop).unwrap();
    let both = wait_for(&receiver, |state| {
        state.loop_file.as_deref() == Some("inf") && state.loop_playlist.as_deref() == Some("inf")
    });
    assert!(both.paused);
    assert_eq!(both.loop_mode(), riina_core::LoopMode::File);
    player
        .send(Command::Seek {
            seconds: 4.8,
            absolute: true,
        })
        .unwrap();
    let near_end = wait_for(&receiver, |state| {
        state.paused
            && !state.seeking
            && state.completed_seeks > paused.completed_seeks
            && state.position.is_some_and(|p| (4.75..4.95).contains(&p))
    });
    player.send(Command::TogglePause).unwrap();
    let wrapped = wait_for(&receiver, |state| {
        !state.paused
            && state.phase == Phase::Playing
            && state.position.is_some_and(|p| p < 0.5)
            && state.generation == near_end.generation
    });
    assert_eq!(wrapped.error, None);
    assert_eq!(wrapped.loop_file.as_deref(), Some("inf"));
    assert_eq!(wrapped.loop_playlist.as_deref(), Some("inf"));
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| state.paused);
    player.send(Command::ToggleFileLoop).unwrap();
    let file_off = wait_for(&receiver, |state| {
        state.loop_file.as_deref() == Some("no") && state.loop_playlist.as_deref() == Some("inf")
    });
    assert!(file_off.paused);
    assert_eq!(file_off.loop_mode(), riina_core::LoopMode::Playlist);
    assert_eq!(file_off.error, None);
    player.send(Command::CycleLoop).unwrap();
    wait_for(&receiver, |state| {
        state.loop_mode() == riina_core::LoopMode::Off
    });
    player.send(Command::ToggleFileLoop).unwrap();
    let file_only = wait_for(&receiver, |state| {
        state.loop_file.as_deref() == Some("inf") && state.loop_playlist.as_deref() == Some("no")
    });
    assert!(file_only.paused);
    player.send(Command::ToggleFileLoop).unwrap();
    let all_off = wait_for(&receiver, |state| {
        state.loop_file.as_deref() == Some("no") && state.loop_playlist.as_deref() == Some("no")
    });
    assert!(all_off.paused);
    assert_eq!(all_off.error, None);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies A–B point cycling and actual loop playback"]
fn ab_repeat_observes_points_and_wraps_playback() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let (sender, receiver) = mpsc::channel();
    let player = Player::start(&test_library(&root), true, move |state| {
        let _ = sender.send(state);
    })
    .unwrap();
    player
        .send(Command::Load(
            root.join("tests/fixtures/catalog.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    wait_for(&receiver, |state| {
        state.phase == Phase::Playing && state.position.is_some_and(|p| p > 0.1)
    });
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| state.paused);
    player
        .send(Command::SetAbLoop {
            a: Some(0.0),
            b: Some(0.6),
        })
        .unwrap();
    wait_for(&receiver, |state| {
        state
            .ab_loop_a
            .is_some_and(|a| (a - 0.000001).abs() < 0.0000001)
            && state.ab_loop_b.is_some_and(|b| (b - 0.6).abs() < 0.001)
    });
    player
        .send(Command::Seek {
            seconds: 0.25,
            absolute: true,
        })
        .unwrap();
    let start = wait_for(&receiver, |state| {
        state.paused && !state.seeking && state.position.is_some_and(|p| (p - 0.25).abs() < 0.01)
    });
    player.send(Command::TogglePause).unwrap();
    let wrapped = wait_for(&receiver, |state| {
        !state.paused
            && !state.seeking
            && state.completed_seeks > start.completed_seeks
            && state.position.is_some_and(|p| p < 0.2)
    });
    assert_eq!(wrapped.error, None);
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| state.paused);
    player.send(Command::CycleAbLoop).unwrap();
    wait_for(&receiver, |state| {
        state.ab_loop_a.is_none() && state.ab_loop_b.is_none()
    });
    player.send(Command::CycleAbLoop).unwrap();
    let a_set = wait_for(&receiver, |state| {
        state.ab_loop_a.is_some() && state.ab_loop_b.is_none()
    });
    player
        .send(Command::Seek {
            seconds: 0.4,
            absolute: true,
        })
        .unwrap();
    let sought = wait_for(&receiver, |state| {
        !state.seeking
            && state.completed_seeks > a_set.completed_seeks
            && state.position.is_some_and(|p| (p - 0.4).abs() < 0.05)
    });
    player.send(Command::CycleAbLoop).unwrap();
    wait_for(&receiver, |state| {
        state.ab_loop_a.is_some()
            && state
                .ab_loop_b
                .is_some_and(|b| (b - sought.position.unwrap()).abs() < 0.05)
    });
    player.send(Command::CycleAbLoop).unwrap();
    let cleared = wait_for(&receiver, |state| {
        state.ab_loop_a.is_none() && state.ab_loop_b.is_none()
    });
    assert_eq!(cleared.error, None);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies subtitle cue navigation with observed clock and text"]
fn subtitle_navigation_seeks_cues_without_changing_delay() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let (sender, receiver) = mpsc::channel();
    let player = Player::start(&test_library(&root), true, move |state| {
        let _ = sender.send(state);
    })
    .unwrap();
    player
        .send(Command::Load(
            root.join("tests/fixtures/catalog.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    wait_for(&receiver, |state| {
        state.phase == Phase::Playing && state.position.is_some_and(|p| p > 0.1)
    });
    player.send(Command::TogglePause).unwrap();
    let paused = wait_for(&receiver, |state| state.paused);
    player
        .send(Command::AddSubtitle(
            root.join("tests/fixtures/manual-captions.srt")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    wait_for(&receiver, |state| {
        state
            .tracks
            .iter()
            .any(|t| t.kind == TrackKind::Sub && t.external && t.selected)
    });
    player
        .send(Command::Seek {
            seconds: 0.5,
            absolute: true,
        })
        .unwrap();
    let first = wait_for(&receiver, |state| {
        state.paused
            && !state.seeking
            && state.completed_seeks > paused.completed_seeks
            && state.position.is_some_and(|p| (p - 0.5).abs() < 0.05)
            && state
                .subtitle_text
                .as_deref()
                .is_some_and(|text| text.contains("Привет"))
    });
    player.send(Command::SubtitleSeek(1)).unwrap();
    let second = wait_for(&receiver, |state| {
        state.paused
            && !state.seeking
            && state.completed_seeks > first.completed_seeks
            && state.position.is_some_and(|p| (1.9..2.2).contains(&p))
            && state
                .subtitle_text
                .as_deref()
                .is_some_and(|text| text.contains("Chapter two"))
    });
    assert_eq!(second.subtitle_delay, first.subtitle_delay);
    assert_eq!(second.error, None);
    player.send(Command::SubtitleSeek(-1)).unwrap();
    let returned = wait_for(&receiver, |state| {
        state.paused
            && !state.seeking
            && state.completed_seeks > second.completed_seeks
            && state.position.is_some_and(|p| p < 0.2)
            && state
                .subtitle_text
                .as_deref()
                .is_some_and(|text| text.contains("Привет"))
    });
    assert_eq!(returned.subtitle_delay, first.subtitle_delay);
    assert_eq!(returned.error, None);
    player.send(Command::SubtitleDelay(0.25)).unwrap();
    wait_for(&receiver, |state| {
        (state.subtitle_delay - 0.25).abs() < 0.001
    });
    player
        .send(Command::Seek {
            seconds: 0.5,
            absolute: true,
        })
        .unwrap();
    let delayed_first = wait_for(&receiver, |state| {
        state.paused
            && !state.seeking
            && state.completed_seeks > returned.completed_seeks
            && state.position.is_some_and(|p| (p - 0.5).abs() < 0.05)
            && state
                .subtitle_text
                .as_deref()
                .is_some_and(|text| text.contains("Привет"))
    });
    player.send(Command::SubtitleSeek(1)).unwrap();
    let delayed_second = wait_for(&receiver, |state| {
        state.paused
            && !state.seeking
            && state.completed_seeks > delayed_first.completed_seeks
            && state.position.is_some_and(|p| (2.15..2.4).contains(&p))
            && state
                .subtitle_text
                .as_deref()
                .is_some_and(|text| text.contains("Chapter two"))
    });
    assert!((delayed_second.subtitle_delay - 0.25).abs() < 0.001);
    player
        .send(Command::Seek {
            seconds: 2.8,
            absolute: true,
        })
        .unwrap();
    let inside = wait_for(&receiver, |state| {
        state.paused
            && !state.seeking
            && state.completed_seeks > delayed_second.completed_seeks
            && state.position.is_some_and(|p| (p - 2.8).abs() < 0.05)
    });
    player.send(Command::SubtitleSeek(0)).unwrap();
    let restarted = wait_for(&receiver, |state| {
        state.paused
            && !state.seeking
            && state.completed_seeks > inside.completed_seeks
            && state.position.is_some_and(|p| (2.15..2.4).contains(&p))
    });
    assert!((restarted.subtitle_delay - 0.25).abs() < 0.001);
    assert_eq!(restarted.error, None);

    // Queue the burst without reading snapshots: each adjustment must use the
    // decoder's current delay rather than the UI's last observed value.
    for _ in 0..20 {
        player.send(Command::AdjustSubtitleDelay(0.1)).unwrap();
    }
    let increased = wait_for(&receiver, |state| {
        (state.subtitle_delay - 2.25).abs() < 0.001
    });
    assert!(increased.paused);
    assert_eq!(increased.error, None);
    for _ in 0..4 {
        player.send(Command::AdjustSubtitleDelay(-0.5)).unwrap();
    }
    let restored = wait_for(&receiver, |state| {
        (state.subtitle_delay - 0.25).abs() < 0.001
    });
    assert!(restored.paused);
    player.send(Command::SubtitleDelay(0.0)).unwrap();
    let reset = wait_for(&receiver, |state| state.subtitle_delay.abs() < 0.001);
    assert!(reset.paused);
    assert_eq!(reset.error, None);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies observed forward/backward frame navigation"]
fn frame_navigation_moves_one_frame_and_retains_pause() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let (sender, receiver) = mpsc::channel();
    let player = Player::start(&test_library(&root), true, move |state| {
        let _ = sender.send(state);
    })
    .unwrap();
    player
        .send(Command::Load(
            root.join("tests/fixtures/catalog.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    wait_for(&receiver, |state| {
        state.phase == Phase::Playing && state.position.is_some_and(|p| p > 0.1)
    });
    player.send(Command::TogglePause).unwrap();
    let paused = wait_for(&receiver, |state| state.paused);
    player
        .send(Command::Seek {
            seconds: 2.0,
            absolute: true,
        })
        .unwrap();
    let mut previous = wait_for(&receiver, |state| {
        state.paused
            && !state.seeking
            && state.completed_seeks > paused.completed_seeks
            && state.position.is_some_and(|p| (p - 2.0).abs() < 0.01)
    });
    // The original catalog fixture is constant-rate 24 fps. Check observed frame
    // movement rather than treating command acceptance as a decoded frame.
    for (command, direction) in [
        (Command::FrameStep, 1.0),
        (Command::FrameStep, 1.0),
        (Command::FrameBackStep, -1.0),
        (Command::FrameBackStep, -1.0),
    ] {
        let from = previous.position.unwrap();
        player.send(command).unwrap();
        let next = wait_for(&receiver, |state| {
            state.paused
                && !state.seeking
                && state
                    .position
                    .is_some_and(|p| (p - from) * direction > 0.02)
        });
        assert!(((next.position.unwrap() - from) * direction - 1.0 / 24.0).abs() < 0.002);
        assert_eq!(next.error, None);
        previous = next;
    }
    assert!((previous.position.unwrap() - 2.0).abs() < 0.002);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies file-local A–B loop ownership"]
fn ab_repeat_resets_on_replacement_and_stop() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let (sender, receiver) = mpsc::channel();
    let player = Player::start(&test_library(&root), true, move |state| {
        let _ = sender.send(state);
    })
    .unwrap();
    let load = |name: &str| {
        Command::Load(
            root.join("tests/fixtures")
                .join(name)
                .to_string_lossy()
                .into_owned(),
        )
    };
    player.send(load("catalog.mkv")).unwrap();
    let first = wait_for(&receiver, |state| {
        state.phase == Phase::Playing && state.position.is_some_and(|p| p > 0.1)
    });
    player
        .send(Command::SetAbLoop {
            a: Some(0.1),
            b: Some(0.7),
        })
        .unwrap();
    wait_for(&receiver, |state| {
        state.ab_loop_a.is_some() && state.ab_loop_b.is_some()
    });
    player.send(load("moving-square.avi")).unwrap();
    // Going beyond the old B point proves the decoder reset its loop, rather
    // than merely making our cached markers disappear during begin_load.
    let replacement = wait_for(&receiver, |state| {
        state.generation > first.generation
            && state.phase == Phase::Playing
            && state.position.is_some_and(|p| p > 1.0)
    });
    assert!(replacement.ab_loop_a.is_none() && replacement.ab_loop_b.is_none());
    assert_eq!(replacement.error, None);
    player
        .send(Command::SetAbLoop {
            a: Some(0.8),
            b: Some(1.8),
        })
        .unwrap();
    wait_for(&receiver, |state| {
        state.ab_loop_a.is_some() && state.ab_loop_b.is_some()
    });
    player.send(Command::Stop).unwrap();
    let stopped = wait_for(&receiver, |state| state.phase == Phase::Stopped);
    assert!(stopped.ab_loop_a.is_none() && stopped.ab_loop_b.is_none());
    player.send(load("catalog.mkv")).unwrap();
    let reopened = wait_for(&receiver, |state| {
        state.generation > replacement.generation
            && state.phase == Phase::Playing
            && state.position.is_some_and(|p| p > 2.0)
    });
    assert!(reopened.ab_loop_a.is_none() && reopened.ab_loop_b.is_none());
    assert_eq!(reopened.error, None);
    player
        .send(Command::SetAbLoop {
            a: Some(0.8),
            b: Some(1.8),
        })
        .unwrap();
    wait_for(&receiver, |state| {
        state.ab_loop_a.is_some() && state.ab_loop_b.is_some()
    });
    player
        .send(Command::AppendFiles(vec![
            root.join("tests/fixtures/moving-square.avi")
                .to_string_lossy()
                .into_owned(),
        ]))
        .unwrap();
    wait_for(&receiver, |state| state.playlist.len() == 2);
    player.send(Command::PlaylistNext).unwrap();
    let advanced = wait_for(&receiver, |state| {
        state.generation > reopened.generation
            && state.phase == Phase::Playing
            && state.position.is_some_and(|p| p > 2.0)
    });
    assert!(advanced.ab_loop_a.is_none() && advanced.ab_loop_b.is_none());
    assert_eq!(advanced.error, None);
}

#[test]
#[ignore = "requires provisioned libmpv; decodes original PQ/HLG and checks replacement metadata"]
fn hdr_metadata_replacement_and_stop() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let (sender, receiver) = mpsc::channel();
    let player = Player::start(&test_library(&root), true, move |state| {
        let _ = sender.send(state);
    })
    .unwrap();
    let mut generation = 0;
    for (file, gamma, pixel_format, size) in [
        ("hdr-pq.mkv", "pq", "yuv422p10", (192, 108)),
        ("hdr-hlg.mkv", "hlg", "yuv422p10", (192, 108)),
        ("catalog.mkv", "srgb", "yuv420p", (320, 180)),
    ] {
        player
            .send(Command::Load(
                root.join("tests/fixtures")
                    .join(file)
                    .to_str()
                    .unwrap()
                    .into(),
            ))
            .unwrap();
        let loaded = wait_for(&receiver, |state| {
            state.generation > generation
                && state.position.is_some_and(|p| p > 0.1)
                && state
                    .video
                    .as_ref()
                    .is_some_and(|video| video.pixelformat.is_some())
        });
        generation = loaded.generation;
        assert_eq!(loaded.error, None);
        let video = loaded.video.unwrap();
        assert_eq!(video.is_hdr(), file.starts_with("hdr-"));
        println!("{file}: {video:?}");
        assert_eq!(video.gamma.as_deref(), Some(gamma));
        assert_eq!(video.pixelformat.as_deref(), Some(pixel_format));
        assert_eq!((video.w, video.h), size);
        if file.starts_with("hdr-") {
            assert_eq!(video.primaries.as_deref(), Some("bt.2020"));
            assert_eq!(video.colormatrix.as_deref(), Some("bt.2020-ncl"));
            assert_eq!(video.colorlevels.as_deref(), Some("limited"));
            assert!(loaded.duration.is_some_and(|d| (d - 3.0).abs() < 0.1));
        }
    }
    player.send(Command::Stop).unwrap();
    let stopped = wait_for(&receiver, |state| {
        state.phase == Phase::Stopped && state.video.is_none()
    });
    assert_eq!(stopped.error, None);
    drop(player);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies ordered multi-file replacement and resume"]
fn multi_file_open_replaces_old_playlist_and_resumes() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let fixture = |name: &str| {
        root.join("tests/fixtures")
            .join(name)
            .to_str()
            .unwrap()
            .trim_start_matches(r"\\?\")
            .to_owned()
    };
    let (sender, receiver) = mpsc::channel();
    let player = Player::start(&test_library(&root), true, move |state| {
        let _ = sender.send(state);
    })
    .unwrap();
    player.send(Command::Load(fixture("catalog.mkv"))).unwrap();
    wait_for(&receiver, |state| state.position.is_some_and(|p| p > 0.1));
    player.send(Command::TogglePause).unwrap();
    let old = wait_for(&receiver, |state| state.phase == Phase::Paused);
    let paths = vec![
        fixture("moving-square.avi"),
        fixture("hdr-hlg.mkv"),
        fixture("external-tone.wav"),
    ];
    player.send(Command::OpenFiles(paths.clone())).unwrap();
    let opened = wait_for(&receiver, |state| {
        state.playlist.len() == 3
            && state.generation > old.generation
            && state.playlist[0].filename == paths[0]
            && state.playlist[0].current
            && state
                .duration
                .is_some_and(|duration| (duration - 5.0).abs() < 0.1)
            && state.position.is_some_and(|p| p > 0.1)
            && !state.paused
    });
    assert_eq!(opened.error, None);
    assert_eq!(
        opened
            .playlist
            .iter()
            .map(|entry| &entry.filename)
            .collect::<Vec<_>>(),
        paths.iter().collect::<Vec<_>>()
    );
    player.send(Command::PlaylistNext).unwrap();
    let next = wait_for(&receiver, |state| {
        state.playlist.get(1).is_some_and(|entry| entry.current)
            && state
                .video
                .as_ref()
                .is_some_and(|video| video.gamma.as_deref() == Some("hlg"))
            && state.position.is_some_and(|p| p > 0.1)
    });
    assert_eq!(next.error, None);
    assert!(!next.paused);
    drop(player);
}

fn wait_for(
    receiver: &mpsc::Receiver<Playback>,
    predicate: impl Fn(&Playback) -> bool,
) -> Playback {
    let deadline = Instant::now() + Duration::from_secs(12);
    let mut last = None;
    loop {
        assert!(
            Instant::now() < deadline,
            "Timed out; last observed state: {last:?}"
        );
        if let Ok(state) = receiver.recv_timeout(Duration::from_millis(200)) {
            if predicate(&state) {
                return state;
            }
            last = Some(state);
        }
    }
}

#[test]
#[ignore = "requires provisioned libmpv; exercises real native playback"]
fn playback_pause_seek_stop_failure_and_shutdown() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let (sender, receiver) = mpsc::channel();
    let player = Player::start(&test_library(&root), true, move |state| {
        let _ = sender.send(state);
    })
    .expect("Prerequisite failure: provision libmpv and select its absolute path with RIINA_TEST_MPV_LIBRARY");
    player
        .send(Command::Load(
            root.join("tests/fixtures/moving-square.avi")
                .to_str()
                .unwrap()
                .into(),
        ))
        .unwrap();
    let loaded = wait_for(&receiver, |state| {
        state.phase == Phase::Playing && state.position.is_some_and(|p| p > 0.2)
    });
    assert!(loaded.duration.is_some_and(|d| (d - 5.0).abs() < 0.1));
    assert!(
        loaded.audio_output.is_some(),
        "No active native audio output was observed"
    );
    let catalog = wait_for(&receiver, |state| {
        state
            .tracks
            .iter()
            .any(|track| track.kind == TrackKind::Audio && track.selected)
            && state
                .tracks
                .iter()
                .any(|track| track.kind == TrackKind::Video)
            && state.playlist.len() == 1
    });
    let audio_id = catalog
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Audio)
        .unwrap()
        .id;
    player
        .send(Command::SelectTrack {
            kind: TrackKind::Audio,
            id: None,
        })
        .unwrap();
    wait_for(&receiver, |state| {
        !state
            .tracks
            .iter()
            .any(|track| track.kind == TrackKind::Audio && track.selected)
    });
    player
        .send(Command::SelectTrack {
            kind: TrackKind::Audio,
            id: Some(audio_id),
        })
        .unwrap();
    wait_for(&receiver, |state| {
        state
            .tracks
            .iter()
            .any(|track| track.kind == TrackKind::Audio && track.id == audio_id && track.selected)
    });
    player.send(Command::TogglePause).unwrap();
    let paused = wait_for(&receiver, |state| state.phase == Phase::Paused);
    player
        .send(Command::Seek {
            seconds: 2.0,
            absolute: true,
        })
        .unwrap();
    let seeked = wait_for(&receiver, |state| {
        state.paused
            && state.position.is_some_and(|p| (p - 2.0).abs() < 0.15)
            && state.completed_seeks > paused.completed_seeks
    });
    assert_eq!(paused.generation, seeked.generation);
    player.send(Command::Volume(50.0)).unwrap();
    wait_for(&receiver, |state| (state.volume - 50.0).abs() < 0.1);
    for _ in 0..12 {
        player
            .send(Command::AdjustVolume {
                delta: 5.0,
                boost: false,
            })
            .unwrap();
    }
    let capped = wait_for(&receiver, |state| (state.volume - 100.0).abs() < 0.1);
    assert!(capped.paused);
    assert_eq!(capped.error, None);
    for _ in 0..22 {
        player
            .send(Command::AdjustVolume {
                delta: -5.0,
                boost: false,
            })
            .unwrap();
    }
    wait_for(&receiver, |state| state.volume.abs() < 0.1);
    player.send(Command::SetVolumeBoost(true)).unwrap();
    for volume in [150.0, 200.0] {
        player.send(Command::Volume(volume)).unwrap();
        wait_for(&receiver, |state| (state.volume - volume).abs() < 0.1);
    }
    player.send(Command::Volume(150.0)).unwrap();
    wait_for(&receiver, |state| (state.volume - 150.0).abs() < 0.1);
    for _ in 0..12 {
        player
            .send(Command::AdjustVolume {
                delta: 5.0,
                boost: true,
            })
            .unwrap();
    }
    wait_for(&receiver, |state| (state.volume - 200.0).abs() < 0.1);
    player.send(Command::SetVolumeBoost(false)).unwrap();
    wait_for(&receiver, |state| (state.volume - 100.0).abs() < 0.1);
    player.send(Command::Speed(1.0)).unwrap();
    wait_for(&receiver, |state| (state.speed - 1.0).abs() < 0.001);
    for _ in 0..3 {
        player.send(Command::MultiplySpeed(2.0)).unwrap();
    }
    wait_for(&receiver, |state| (state.speed - 8.0).abs() < 0.001);
    for _ in 0..3 {
        player.send(Command::MultiplySpeed(0.5)).unwrap();
    }
    wait_for(&receiver, |state| (state.speed - 1.0).abs() < 0.001);
    for _ in 0..10 {
        player.send(Command::MultiplySpeed(1.1)).unwrap();
    }
    wait_for(&receiver, |state| {
        (state.speed - 1.1_f64.powi(10)).abs() < 0.001
    });
    player.send(Command::Speed(1.0)).unwrap();
    let normal = wait_for(&receiver, |state| (state.speed - 1.0).abs() < 0.001);
    assert!(normal.paused);
    assert_eq!(normal.error, None);
    player.send(Command::Volume(25.0)).unwrap();
    wait_for(&receiver, |state| (state.volume - 25.0).abs() < 0.1);
    player.send(Command::ToggleMute).unwrap();
    wait_for(&receiver, |state| state.muted);
    player.send(Command::TogglePause).unwrap();
    player
        .send(Command::Seek {
            seconds: 4.8,
            absolute: true,
        })
        .unwrap();
    wait_for(&receiver, |state| state.phase == Phase::Ended);
    player.send(Command::Stop).unwrap();
    wait_for(&receiver, |state| state.phase == Phase::Stopped);
    player
        .send(Command::Load(
            root.join("tests/fixtures/does-not-exist.mkv")
                .to_str()
                .unwrap()
                .into(),
        ))
        .unwrap();
    wait_for(&receiver, |state| {
        state.phase == Phase::Failed && state.error.is_some()
    });
    player
        .send(Command::Load(
            root.join("tests/fixtures/moving-square.avi")
                .to_str()
                .unwrap()
                .into(),
        ))
        .unwrap();
    let reloaded = wait_for(&receiver, |state| {
        state.generation > loaded.generation
            && state.phase == Phase::Playing
            && !state.paused
            && state.position.is_some_and(|p| p > 0.2)
    });
    assert!(reloaded.error.is_none());
    drop(player);
}

#[test]
#[ignore = "requires provisioned libmpv; exercises real catalog workflows"]
fn chapters_external_tracks_and_playlist_editing() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let fixture = |name: &str| {
        root.join("tests/fixtures")
            .join(name)
            .to_str()
            .unwrap()
            .to_owned()
    };
    let (sender, receiver) = mpsc::channel();
    let player = Player::start(&test_library(&root), true, move |state| {
        let _ = sender.send(state);
    })
    .unwrap();
    player.send(Command::Load(fixture("catalog.mkv"))).unwrap();
    let loaded = wait_for(&receiver, |state| {
        state.phase == Phase::Playing && state.chapters.len() == 2
    });
    assert_eq!(loaded.chapters[0].title.as_deref(), Some("Opening"));
    assert_eq!(loaded.chapters[1].title.as_deref(), Some("Moving square"));
    assert!((loaded.chapters[1].time - 2.0).abs() < 0.001);
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| state.phase == Phase::Paused);
    player
        .send(Command::AddSubtitle(fixture("manual-captions.srt")))
        .unwrap();
    let subs = wait_for(&receiver, |state| {
        state
            .tracks
            .iter()
            .any(|track| track.kind == TrackKind::Sub && track.external && track.selected)
    });
    let sub_count = subs
        .tracks
        .iter()
        .filter(|track| track.kind == TrackKind::Sub)
        .count();
    player
        .send(Command::Seek {
            seconds: 0.5,
            absolute: true,
        })
        .unwrap();
    wait_for(&receiver, |state| {
        state
            .subtitle_text
            .as_deref()
            .is_some_and(|text| text.contains("café") && text.contains("Привет"))
    });
    player
        .send(Command::AddSubtitle(fixture("manual-captions.srt")))
        .unwrap();
    // Reload can temporarily remove its catalog entry. Delay observation alone
    // orders commands but does not prove the independent catalog has settled.
    // Wait for its selected external entry as well, without a timing sleep.
    player.send(Command::SubtitleDelay(0.25)).unwrap();
    let reloaded_subs = wait_for(&receiver, |state| {
        (state.subtitle_delay - 0.25).abs() < 0.001
            && state
                .tracks
                .iter()
                .any(|track| track.kind == TrackKind::Sub && track.external && track.selected)
    });
    assert_eq!(
        reloaded_subs
            .tracks
            .iter()
            .filter(|track| track.kind == TrackKind::Sub)
            .count(),
        sub_count,
        "Reloaded subtitle catalog: {:?}; original catalog: {:?}; requested path: {}",
        reloaded_subs.tracks,
        subs.tracks,
        fixture("manual-captions.srt")
    );
    player.send(Command::ToggleSubtitleVisibility).unwrap();
    wait_for(&receiver, |state| !state.subtitle_visible);
    player.send(Command::ToggleSubtitleVisibility).unwrap();
    wait_for(&receiver, |state| state.subtitle_visible);
    player
        .send(Command::AddAudio(fixture("external-tone.wav")))
        .unwrap();
    wait_for(&receiver, |state| {
        state
            .tracks
            .iter()
            .any(|track| track.kind == TrackKind::Audio && track.external && track.selected)
    });
    player.send(Command::AudioDelay(-0.2)).unwrap();
    wait_for(&receiver, |state| (state.audio_delay + 0.2).abs() < 0.001);
    // Native relative changes must accumulate even when no UI snapshots are
    // consumed between commands, and must leave the playback clock paused.
    for _ in 0..12 {
        player.send(Command::AdjustAudioDelay(0.1)).unwrap();
    }
    let delayed = wait_for(&receiver, |state| (state.audio_delay - 1.0).abs() < 0.001);
    assert!(delayed.paused);
    assert_eq!(delayed.error, None);
    for _ in 0..2 {
        player.send(Command::AdjustAudioDelay(-0.5)).unwrap();
    }
    let restored = wait_for(&receiver, |state| state.audio_delay.abs() < 0.001);
    assert!(restored.paused);
    player.send(Command::AdjustAudioDelay(-0.1)).unwrap();
    wait_for(&receiver, |state| (state.audio_delay + 0.1).abs() < 0.001);
    player.send(Command::AudioDelay(0.0)).unwrap();
    let reset = wait_for(&receiver, |state| state.audio_delay.abs() < 0.001);
    assert!(reset.paused);
    assert_eq!(reset.error, None);
    player.send(Command::SelectChapter(1)).unwrap();
    let selected_chapter = wait_for(&receiver, |state| {
        !state.paused
            && state
                .position
                .is_some_and(|position| (2.0..2.5).contains(&position))
    });
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| state.phase == Phase::Paused);
    player.send(Command::AdjustChapter(-1)).unwrap();
    let previous_chapter = wait_for(&receiver, |state| {
        state.paused
            && !state.seeking
            && state.completed_seeks > selected_chapter.completed_seeks
            && state.position.is_some_and(|p| p < 0.1)
    });
    assert_eq!(previous_chapter.error, None);
    player.send(Command::AdjustChapter(1)).unwrap();
    let next_chapter = wait_for(&receiver, |state| {
        state.paused
            && !state.seeking
            && state.completed_seeks > previous_chapter.completed_seeks
            && state.position.is_some_and(|p| (p - 2.0).abs() < 0.05)
    });
    assert_eq!(next_chapter.error, None);
    player.send(Command::AdjustChapter(-1)).unwrap();
    let first_chapter = wait_for(&receiver, |state| {
        state.paused
            && !state.seeking
            && state.completed_seeks > next_chapter.completed_seeks
            && state.position.is_some_and(|p| p < 0.1)
    });
    player
        .send(Command::Seek {
            seconds: 0.5,
            absolute: true,
        })
        .unwrap();
    let inside_first = wait_for(&receiver, |state| {
        state.paused
            && !state.seeking
            && state.completed_seeks > first_chapter.completed_seeks
            && state.position.is_some_and(|p| (p - 0.5).abs() < 0.05)
    });
    player.send(Command::AdjustChapter(-1)).unwrap();
    let boundary = wait_for(&receiver, |state| {
        state.paused
            && !state.seeking
            && state.completed_seeks > inside_first.completed_seeks
            && state.position.is_some_and(|p| p < 0.1)
    });
    assert_eq!(boundary.generation, loaded.generation);
    assert_eq!(boundary.error, None);
    player
        .send(Command::AppendFiles(vec![
            fixture("moving-square.avi"),
            fixture("external-tone.wav"),
        ]))
        .unwrap();
    let appended = wait_for(&receiver, |state| state.playlist.len() == 3);
    assert_eq!(
        appended.generation, loaded.generation,
        "Appending must not replace playback"
    );
    let avi_id = appended.playlist[1].id;
    let wav_id = appended.playlist[2].id;
    player
        .send(Command::MovePlaylistId {
            id: wav_id,
            direction: -1,
        })
        .unwrap();
    wait_for(&receiver, |state| {
        state.playlist.len() == 3 && state.playlist[2].id == avi_id
    });
    player
        .send(Command::MovePlaylistId {
            id: wav_id,
            direction: 1,
        })
        .unwrap();
    wait_for(&receiver, |state| {
        state.playlist.len() == 3 && state.playlist[1].id == avi_id
    });
    player
        .send(Command::MovePlaylistId {
            id: wav_id,
            direction: -1,
        })
        .unwrap();
    wait_for(&receiver, |state| {
        state.playlist.len() == 3 && state.playlist[2].id == avi_id
    });
    player.send(Command::RemovePlaylistId(wav_id)).unwrap();
    wait_for(&receiver, |state| {
        state.playlist.len() == 2 && state.playlist[1].id == avi_id
    });
    player.send(Command::SelectPlaylistId(avi_id)).unwrap();
    let advanced = wait_for(&receiver, |state| {
        state.generation > loaded.generation
            && state.phase == Phase::Playing
            && !state.paused
            && state
                .playlist
                .iter()
                .any(|entry| entry.id == avi_id && entry.current)
    });
    player.send(Command::SelectPlaylistEntry(1)).unwrap();
    let same_entry = wait_for(&receiver, |state| {
        state.generation > advanced.generation && state.phase == Phase::Playing && !state.paused
    });
    assert!(same_entry.position.is_some_and(|position| position < 1.0));
    player.send(Command::CycleLoop).unwrap();
    wait_for(&receiver, |state| {
        state.loop_mode() == riina_core::LoopMode::File
    });
    player.send(Command::CycleLoop).unwrap();
    wait_for(&receiver, |state| {
        state.loop_mode() == riina_core::LoopMode::Playlist
    });
    player.send(Command::CycleLoop).unwrap();
    wait_for(&receiver, |state| {
        state.loop_mode() == riina_core::LoopMode::Off
    });
    player.send(Command::ClearPlaylist).unwrap();
    let cleared = wait_for(&receiver, |state| state.playlist.len() == 1);
    assert_eq!(
        cleared.playlist[0].id, avi_id,
        "Clear keeps the playing entry, as in IINA"
    );
    assert!(
        cleared.error.is_none(),
        "Unexpected native error: {:?}",
        cleared.error
    );
    drop(player);
}
