use super::*;

#[expect(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "Incomplete PCM fixture creation must fail the test"
)]
fn create_long_seek_fixture(path: &std::path::Path) {
    // Keep the sample rate typical so headless audio backends do not add
    // seconds of timestamp latency while a seek is paused and observed.
    const SAMPLE_RATE: u32 = 48_000;
    const DURATION_SECONDS: u32 = 1_300;
    const DATA_BYTES: u32 = SAMPLE_RATE * DURATION_SECONDS;
    let mut output = BufWriter::new(File::create(path).expect("create long-seek fixture"));
    output.write_all(b"RIFF").unwrap();
    output
        .write_all(&(36_u32 + DATA_BYTES).to_le_bytes())
        .unwrap();
    output.write_all(b"WAVEfmt ").unwrap();
    output.write_all(&16_u32.to_le_bytes()).unwrap();
    output.write_all(&1_u16.to_le_bytes()).unwrap();
    output.write_all(&1_u16.to_le_bytes()).unwrap();
    output.write_all(&SAMPLE_RATE.to_le_bytes()).unwrap();
    output.write_all(&SAMPLE_RATE.to_le_bytes()).unwrap();
    output.write_all(&1_u16.to_le_bytes()).unwrap();
    output.write_all(&8_u16.to_le_bytes()).unwrap();
    output.write_all(b"data").unwrap();
    output.write_all(&DATA_BYTES.to_le_bytes()).unwrap();

    let silence = [0x80; 1024 * 1024];
    let mut remaining = DATA_BYTES as usize;
    while remaining > 0 {
        let chunk_len = remaining.min(silence.len());
        output.write_all(&silence[..chunk_len]).unwrap();
        remaining -= chunk_len;
    }
    output.flush().expect("finish long-seek fixture");
}

#[test]
#[ignore = "requires provisioned libmpv; verifies navigation feedback provenance"]
fn navigation_completions_follow_explicit_seek_and_same_position_reseek() {
    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    harness
        .send(Command::Load(
            root.join("tests/fixtures/catalog.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    let loaded = harness.wait(|state| state.phase == Phase::Playing);
    assert_eq!(loaded.completed_navigation_seeks, 0);
    harness.send(Command::TogglePause).unwrap();
    let mut previous = harness.wait(|state| state.paused);
    for position in [2.0, 2.0, 0.5] {
        harness
            .send(Command::Seek {
                seconds: position,
                absolute: true,
            })
            .unwrap();
        let completed = harness.wait(|state| {
            state.completed_navigation_seeks > previous.completed_navigation_seeks
                && !state.seeking
                && state
                    .position
                    .is_some_and(|clock| (clock - position).abs() < 0.1)
        });
        assert!(completed.completed_seeks > previous.completed_seeks);
        assert!(completed.paused);
        assert!(completed.error().is_none(), "{:?}", completed.error());
        previous = completed;
    }
}

#[test]
#[ignore = "requires provisioned libmpv; verifies ten-minute relative seeks away from endpoints"]
fn long_relative_seeks_preserve_pause_and_volume() {
    let root = fixture_root();
    let directory = riina_test_support::TempDir::new("long-seek").unwrap();
    let silence = directory.path().join("silence.wav");
    create_long_seek_fixture(&silence);
    let harness = Harness::start(&root).unwrap();
    harness
        .send(Command::Load(silence.to_string_lossy().into_owned()))
        .unwrap();
    harness.wait(|state| {
        state.phase == Phase::Playing
            && state
                .duration
                .is_some_and(|value| (value - 1300.0).abs() < 0.01)
    });
    harness.send(Command::TogglePause).unwrap();
    harness.wait(|state| state.paused);
    harness.send(Command::Volume(37.0)).unwrap();
    harness.wait(|state| (state.volume - 37.0).abs() < 0.01);
    harness
        .send(Command::Seek {
            seconds: 100.0,
            absolute: true,
        })
        .unwrap();
    harness.wait(|state| {
        state
            .position
            .is_some_and(|value| (value - 100.0).abs() < 0.1)
    });
    // Stay away from boundaries: clamping could hide an incorrect seek amount.
    for (seconds, expected) in [(600.0, 700.0), (-600.0, 100.0)] {
        harness
            .send(Command::Seek {
                seconds,
                absolute: false,
            })
            .unwrap();
        let sought = harness.wait(|state| {
            state
                .position
                // Audio packet timestamps can quantize a keyframe seek by one
                // packet; keep the assertion strict enough to catch clamping.
                .is_some_and(|value| (value - expected).abs() < 2.0)
        });
        assert!(sought.paused);
        assert!((sought.volume - 37.0).abs() < 0.01);
        assert_eq!(sought.error(), None);
    }
}

#[test]
#[ignore = "requires provisioned libmpv; verifies A–B point cycling and actual loop playback"]
fn ab_repeat_observes_points_and_wraps_playback() {
    let root = fixture_root();
    let Harness { player, receiver } = Harness::start(&root).unwrap();
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
    eprintln!("ab-repeat: playback started");
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| state.paused);
    eprintln!("ab-repeat: paused");
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
    eprintln!("ab-repeat: loop points set");
    player
        .send(Command::Seek {
            seconds: 0.25,
            absolute: true,
        })
        .unwrap();
    let start = wait_for(&receiver, |state| {
        state.paused && !state.seeking && state.position.is_some_and(|p| (p - 0.25).abs() < 0.01)
    });
    eprintln!("ab-repeat: start seek complete");
    player.send(Command::TogglePause).unwrap();
    let wrapped = wait_for(&receiver, |state| {
        !state.paused
            && !state.seeking
            && state.completed_seeks > start.completed_seeks
            && state.position.is_some_and(|p| p < 0.2)
    });
    eprintln!("ab-repeat: playback wrapped");
    assert_eq!(wrapped.error(), None);
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| state.paused);
    eprintln!("ab-repeat: wrapped playback paused");
    player.send(Command::CycleAbLoop).unwrap();
    wait_for(&receiver, |state| {
        state.ab_loop_a.is_none() && state.ab_loop_b.is_none()
    });
    eprintln!("ab-repeat: loop cleared");
    player.send(Command::CycleAbLoop).unwrap();
    let a_set = wait_for(&receiver, |state| {
        state.ab_loop_a.is_some() && state.ab_loop_b.is_none()
    });
    eprintln!("ab-repeat: A set");
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
    eprintln!("ab-repeat: seek after A complete");
    player.send(Command::CycleAbLoop).unwrap();
    wait_for(&receiver, |state| {
        state.ab_loop_a.is_some()
            && state
                .ab_loop_b
                .is_some_and(|b| (b - sought.position.unwrap()).abs() < 0.05)
    });
    eprintln!("ab-repeat: A and B set");
    player.send(Command::CycleAbLoop).unwrap();
    let cleared = wait_for(&receiver, |state| {
        state.ab_loop_a.is_none() && state.ab_loop_b.is_none()
    });
    assert_eq!(cleared.error(), None);
    eprintln!("ab-repeat: loop cleared; shutting down player");
    drop(player);
    eprintln!("ab-repeat: player shut down");
}

#[test]
#[ignore = "requires provisioned libmpv; verifies observed forward/backward frame navigation"]
fn frame_navigation_moves_one_frame_and_retains_pause() {
    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    harness
        .send(Command::Load(
            root.join("tests/fixtures/catalog.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    harness.wait(|state| state.phase == Phase::Playing && state.position.is_some_and(|p| p > 0.1));
    harness.send(Command::TogglePause).unwrap();
    let paused = harness.wait(|state| state.paused);
    harness
        .send(Command::Seek {
            seconds: 2.0,
            absolute: true,
        })
        .unwrap();
    let mut previous = harness.wait(|state| {
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
        harness.send(command).unwrap();
        let next = harness.wait(|state| {
            state.paused
                && !state.seeking
                && state
                    .position
                    .is_some_and(|p| (p - from) * direction > 0.02)
        });
        assert!(((next.position.unwrap() - from) * direction - 1.0 / 24.0).abs() < 0.002);
        assert_eq!(next.error(), None);
        previous = next;
    }
    assert!((previous.position.unwrap() - 2.0).abs() < 0.002);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies file-local A–B loop ownership"]
fn ab_repeat_resets_on_replacement_and_stop() {
    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    let load = |name: &str| {
        Command::Load(
            root.join("tests/fixtures")
                .join(name)
                .to_string_lossy()
                .into_owned(),
        )
    };
    harness.send(load("catalog.mkv")).unwrap();
    let first = harness
        .wait(|state| state.phase == Phase::Playing && state.position.is_some_and(|p| p > 0.1));
    harness
        .send(Command::SetAbLoop {
            a: Some(0.1),
            b: Some(0.7),
        })
        .unwrap();
    harness.wait(|state| state.ab_loop_a.is_some() && state.ab_loop_b.is_some());
    harness.send(load("moving-square.avi")).unwrap();
    // Going beyond the old B point proves the decoder reset its loop, rather
    // than merely making our cached markers disappear during begin_load.
    let replacement = harness.wait(|state| {
        state.generation > first.generation
            && state.phase == Phase::Playing
            && state.position.is_some_and(|p| p > 1.0)
    });
    assert!(replacement.ab_loop_a.is_none() && replacement.ab_loop_b.is_none());
    assert_eq!(replacement.error(), None);
    harness
        .send(Command::SetAbLoop {
            a: Some(0.8),
            b: Some(1.8),
        })
        .unwrap();
    harness.wait(|state| state.ab_loop_a.is_some() && state.ab_loop_b.is_some());
    harness.send(Command::Stop).unwrap();
    let stopped = harness.wait(|state| state.phase == Phase::Stopped);
    assert!(stopped.ab_loop_a.is_none() && stopped.ab_loop_b.is_none());
    harness.send(load("catalog.mkv")).unwrap();
    let reopened = harness.wait(|state| {
        state.generation > replacement.generation
            && state.phase == Phase::Playing
            && state.position.is_some_and(|p| p > 2.0)
    });
    assert!(reopened.ab_loop_a.is_none() && reopened.ab_loop_b.is_none());
    assert_eq!(reopened.error(), None);
    harness
        .send(Command::SetAbLoop {
            a: Some(0.8),
            b: Some(1.8),
        })
        .unwrap();
    harness.wait(|state| state.ab_loop_a.is_some() && state.ab_loop_b.is_some());
    harness
        .send(Command::AppendFiles(vec![
            root.join("tests/fixtures/moving-square.avi")
                .to_string_lossy()
                .into_owned(),
        ]))
        .unwrap();
    harness.wait(|state| state.playlist.len() == 2);
    harness.send(Command::PlaylistNext).unwrap();
    let advanced = harness.wait(|state| {
        state.generation > reopened.generation
            && state.phase == Phase::Playing
            && state.position.is_some_and(|p| p > 2.0)
    });
    assert!(advanced.ab_loop_a.is_none() && advanced.ab_loop_b.is_none());
    assert_eq!(advanced.error(), None);
}
