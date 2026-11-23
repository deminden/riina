//! Same original fields through an alternative demuxer, without changing defaults.
use super::*;

#[track_caller]
fn wait(harness: &Harness, predicate: impl Fn(&Playback) -> bool) -> Playback {
    riina_test_support::wait_for(&harness.receiver, Duration::from_secs(10), |state| {
        assert_eq!(state.error(), None);
        predicate(state)
    })
}

#[test]
#[ignore = "requires provisioned libmpv; compares lavf container field metadata"]
fn lavf_woven_field_captures() {
    let root = fixture_root();
    let output_dir = CaptureOutput::new("interlaced-lavf-test").unwrap();
    let output = output_dir.path();
    let mut captures = Vec::new();
    for name in ["interlaced-tff.mkv", "interlaced-bff.mkv"] {
        let player = Harness::with_options(
            &root,
            vec![
                ("demuxer".into(), "lavf".into()),
                ("hwdec".into(), "no".into()),
            ],
        )
        .unwrap();
        player
            .send(Command::Load(
                root.join("tests/fixtures")
                    .join(name)
                    .to_string_lossy()
                    .into_owned(),
            ))
            .unwrap();
        wait(&player, |state| {
            state.phase == Phase::Playing && state.position.is_some()
        });
        player.send(Command::TogglePause).unwrap();
        let mut previous = wait(&player, |state| state.paused && state.position.is_some());
        assert!(
            previous
                .video_frame_info
                .as_ref()
                .is_some_and(|frame| frame.interlaced == Some(true)
                    && frame.top_field_first == Some(name.ends_with("-tff.mkv")))
        );
        for enabled in [false, true, false] {
            player.send(Command::Deinterlace(enabled)).unwrap();
            previous = wait(&player, |state| {
                state.paused
                    && !state.seeking
                    && state.deinterlace == Some(enabled)
                    && state.deinterlace_active == Some(enabled)
            });
            for _ in 0..6 {
                let from = previous.position.unwrap();
                player.send(Command::FrameStep).unwrap();
                let next = wait(&player, |state| {
                    state.paused
                        && !state.seeking
                        && state
                            .position
                            .is_some_and(|position| position > from + 0.005)
                });
                let expected = if enabled { 0.020 } else { 0.040 };
                assert!((next.position.unwrap() - from - expected).abs() < 0.002);
                let path = output.join(format!("capture-{}.png", captures.len()));
                player
                    .send(Command::ScreenshotToFile {
                        path: path.to_string_lossy().into_owned(),
                        content: riina_core::screenshot::Content::Video,
                    })
                    .unwrap();
                let captured = wait(&player, |state| {
                    state.completed_screenshots > next.completed_screenshots
                });
                assert_eq!(captured.position, next.position);
                captures.push(serde_json::json!({"fixture": name, "deinterlace": enabled,
                    "position": captured.position, "path": path}));
                previous = captured;
            }
        }
    }
    let manifest = output.join("captures.json");
    std::fs::write(&manifest, serde_json::to_vec_pretty(&captures).unwrap()).unwrap();
    println!("Forced lavf captures: {}", manifest.display());
}

#[test]
#[ignore = "requires provisioned libmpv; checks screenshot template result"]
fn screenshot_template_returns_saved_path() {
    let root = fixture_root();
    let output_dir = CaptureOutput::new("screenshot-template-log").unwrap();
    let output = output_dir.path();
    let player = Harness::with_options(
        &root,
        vec![
            (
                "log-file".into(),
                output.join("native.log").to_string_lossy().into_owned(),
            ),
            ("msg-level".into(), "all=v".into()),
        ],
    )
    .unwrap();
    player
        .send(Command::Load(
            root.join("tests/fixtures/interlaced-mpeg2-tff.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    wait(&player, |state| {
        state.phase == Phase::Playing && state.position.is_some()
    });
    player.send(Command::TogglePause).unwrap();
    wait(&player, |state| state.paused);
    player
        .send(Command::Screenshot {
            directory: output.to_string_lossy().into_owned(),
            content: riina_core::screenshot::Content::default(),
            encoding: riina_core::screenshot::Encoding {
                format: riina_core::screenshot::Format::Avif,
                ..Default::default()
            },
        })
        .unwrap();
    let saved = wait(&player, |state| state.completed_screenshots == 1);
    println!("Saved {:?}", saved.screenshot_file);
}
