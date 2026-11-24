use super::*;

#[test]
#[ignore = "requires provisioned libmpv; verifies active subtitle composition in saved frames"]
fn screenshot_content_respects_active_subtitle_visibility() {
    use riina_core::screenshot::Content;
    let root = fixture_root();
    let output_dir = CaptureOutput::new("screenshot-subtitles").unwrap();
    let output = output_dir.path().to_path_buf();
    let harness = Harness::start(&root).unwrap();
    harness
        .send(Command::Load(
            root.join("tests/fixtures/catalog.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    harness.wait(|state| state.phase == Phase::Playing && state.position.is_some());
    harness.send(Command::TogglePause).unwrap();
    harness.wait(|state| state.paused);
    harness
        .send(Command::AddSubtitle(
            root.join("tests/fixtures/styled-captions.ass")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    harness.wait(|state| selected_track(state, TrackKind::Sub).is_some());
    harness
        .send(Command::Seek {
            seconds: 0.5,
            absolute: true,
        })
        .unwrap();
    let paused = harness.wait(|state| {
        state.paused
            && state
                .position
                .is_some_and(|position| (position - 0.5).abs() < 0.001)
            && state
                .subtitle_text
                .as_deref()
                .is_some_and(|text| text.contains("Styled first cue"))
    });
    assert!(paused.subtitle_visible);
    assert_eq!(paused.error(), None);
    // Three captures share one paused frame. Hiding the cue must make the
    // subtitles mode equivalent to video-only, without changing capture mode.
    for (index, (name, content)) in [
        ("video", Content::Video),
        ("subtitles", Content::Subtitles),
        ("hidden", Content::Subtitles),
    ]
    .into_iter()
    .enumerate()
    {
        if name == "hidden" {
            harness.send(Command::ToggleSubtitleVisibility).unwrap();
            harness.wait(|state| !state.subtitle_visible);
        }
        let path = output
            .join(format!("{name}.png"))
            .to_string_lossy()
            .into_owned();
        harness
            .send(Command::ScreenshotToFile {
                path: path.clone(),
                content,
            })
            .unwrap();
        let saved = harness.wait(|state| {
            state.completed_screenshots == index as u64 + 1 || state.error().is_some()
        });
        assert_eq!(saved.error(), None);
        assert_eq!(saved.screenshot_file.as_deref(), Some(path.as_str()));
        assert!(saved.paused);
        assert_eq!(saved.position, paused.position);
        assert!(std::fs::metadata(path).unwrap().len() > 20);
    }
    println!("Subtitle capture comparison: {}", output.display());
}

#[test]
#[ignore = "requires provisioned libmpv; writes native PNG captures to ignored tools"]
#[expect(
    clippy::too_many_lines,
    reason = "decomposition tracked in docs/review"
)]
fn screenshots_complete_only_after_native_success() {
    let root = fixture_root();
    let output_dir = CaptureOutput::new("screenshot-test").unwrap();
    let output = output_dir.path().to_path_buf();
    let harness = Harness::start(&root).unwrap();
    // Failure before any frame exists cannot advertise a saved image.
    harness
        .send(Command::ScreenshotToFile {
            path: output.join("no-frame.png").to_string_lossy().into_owned(),
            content: riina_core::screenshot::Content::Video,
        })
        .unwrap();
    let failed = harness.wait(|state| state.error().is_some());
    assert_eq!(failed.completed_screenshots, 0);
    assert_eq!(failed.screenshot_file, None);
    harness
        .send(Command::Load(
            root.join("tests/fixtures/interlaced-tff.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    // The clock can become available before mpv has presented its first frame.
    harness.wait(|state| {
        state.phase == Phase::Playing
            && state.position.is_some()
            && state
                .mpv_events
                .iter()
                .any(|event| event.name.as_ref() == "playback-restart")
    });
    harness.send(Command::TogglePause).unwrap();
    let paused = harness.wait(|state| state.paused && state.position.is_some());
    for (index, content) in [
        riina_core::screenshot::Content::Video,
        riina_core::screenshot::Content::Subtitles,
    ]
    .into_iter()
    .enumerate()
    {
        let path = output
            .join(format!("capture-{index}.png"))
            .to_string_lossy()
            .into_owned();
        harness
            .send(Command::ScreenshotToFile {
                path: path.clone(),
                content,
            })
            .unwrap();
        let saved = harness.wait(|state| {
            state.completed_screenshots == index as u64 + 1 || state.error().is_some()
        });
        assert_eq!(saved.error(), None);
        assert_eq!(saved.screenshot_file.as_deref(), Some(path.as_str()));
        assert!(saved.paused);
        assert_eq!(saved.position, paused.position);
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(u32::from_be_bytes(bytes[16..20].try_into().unwrap()), 96);
        assert_eq!(u32::from_be_bytes(bytes[20..24].try_into().unwrap()), 64);
        assert_eq!(&bytes[bytes.len() - 12..bytes.len() - 8], &[0, 0, 0, 0]);
        assert_eq!(&bytes[bytes.len() - 8..bytes.len() - 4], b"IEND");
    }
    println!("Native completed PNGs: {}", output.display());
    let directory = output.join("Снимки 字");
    std::fs::create_dir(&directory).unwrap();
    let existing = directory.join("interlaced-tff-0001.png");
    std::fs::write(&existing, b"existing image must survive").unwrap();
    for (index, number) in ["0002", "0003"].into_iter().enumerate() {
        harness
            .send(Command::Screenshot {
                directory: directory.to_string_lossy().into_owned(),
                content: riina_core::screenshot::Content::default(),
                encoding: riina_core::screenshot::Encoding::default(),
            })
            .unwrap();
        let saved = harness.wait(|state| {
            state.completed_screenshots == index as u64 + 3 || state.error().is_some()
        });
        assert_eq!(saved.error(), None);
        let path = PathBuf::from(saved.screenshot_file.unwrap());
        assert_eq!(path, directory.join(format!("interlaced-tff-{number}.png")));
        assert_eq!(&std::fs::read(&path).unwrap()[..8], b"\x89PNG\r\n\x1a\n");
        assert!(saved.paused);
        assert_eq!(saved.position, paused.position);
    }
    assert_eq!(
        std::fs::read(existing).unwrap(),
        b"existing image must survive"
    );
    harness
        .send(Command::Screenshot {
            directory: directory.to_string_lossy().into_owned(),
            content: riina_core::screenshot::Content::Video,
            encoding: riina_core::screenshot::Encoding {
                format: riina_core::screenshot::Format::Avif,
                template: "unsupported-422-%n".into(),
            },
        })
        .unwrap();
    let rejected = harness.wait(|state| state.error().is_some());
    assert_eq!(rejected.completed_screenshots, 4);
    assert!(
        rejected
            .screenshot_file
            .as_ref()
            .unwrap()
            .ends_with("interlaced-tff-0003.png")
    );
    assert!(rejected.paused);
    assert_eq!(rejected.position, paused.position);
    harness
        .send(Command::Load(
            root.join("tests/fixtures/interlaced-mpeg2-tff.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    harness.wait(|state| {
        state.generation > paused.generation
            && state.phase == Phase::Playing
            && state.position.is_some()
    });
    harness.send(Command::TogglePause).unwrap();
    let format_paused = harness.wait(|state| state.paused && state.position.is_some());
    let mut formats = Vec::new();
    for (index, format) in [
        riina_core::screenshot::Format::Png,
        riina_core::screenshot::Format::Jpg,
        riina_core::screenshot::Format::Jpeg,
        riina_core::screenshot::Format::Webp,
        riina_core::screenshot::Format::Jxl,
        riina_core::screenshot::Format::Avif,
    ]
    .into_iter()
    .enumerate()
    {
        harness
            .send(Command::Screenshot {
                directory: directory.to_string_lossy().into_owned(),
                content: riina_core::screenshot::Content::Video,
                encoding: riina_core::screenshot::Encoding {
                    format,
                    template: format!("{}-%n", format.native_value()),
                },
            })
            .unwrap();
        let saved = harness.wait(|state| {
            state.completed_screenshots == index as u64 + 5 || state.error().is_some()
        });
        assert_eq!(saved.error(), None, "{format:?}");
        let path = PathBuf::from(saved.screenshot_file.unwrap());
        assert!(path.is_file());
        assert!(std::fs::metadata(&path).unwrap().len() > 20);
        assert!(saved.paused);
        assert_eq!(saved.position, format_paused.position);
        println!("{format:?}: {}", path.display());
        let (decode_sender, decode_receiver) = mpsc::channel();
        let decoder = start_test_player(&test_library(&root), true, move |state| {
            let _ = decode_sender.send(state);
        })
        .unwrap();
        decoder
            .send(Command::Load(path.to_string_lossy().into_owned()))
            .unwrap();
        let decoded = wait_for(&decode_receiver, |state| {
            (state.video.is_some()
                && state.position.is_some()
                && matches!(state.phase, Phase::Playing | Phase::Ended))
                || state.phase == Phase::Failed
        });
        assert_eq!(decoded.error(), None, "decode {format:?}");
        assert_eq!(
            decoded.video.as_ref().map(|video| (video.w, video.h)),
            Some((96, 64))
        );
        let round_trip = output.join(format!("decoded-{}.png", format.native_value()));
        decoder
            .send(Command::ScreenshotToFile {
                path: round_trip.to_string_lossy().into_owned(),
                content: riina_core::screenshot::Content::Video,
            })
            .unwrap();
        let captured = wait_for(&decode_receiver, |state| {
            state.completed_screenshots == 1 || state.error().is_some()
        });
        assert_eq!(captured.error(), None, "decoded frame capture {format:?}");
        assert_eq!(
            &std::fs::read(&round_trip).unwrap()[..8],
            b"\x89PNG\r\n\x1a\n"
        );
        formats.push(serde_json::json!({"format": format.native_value(),
            "path": path, "decoded_png": round_trip, "position": format_paused.position}));
    }
    let manifest = output.join("formats.json");
    std::fs::write(&manifest, serde_json::to_vec_pretty(&formats).unwrap()).unwrap();
    println!("Native screenshot formats: {}", manifest.display());
}
