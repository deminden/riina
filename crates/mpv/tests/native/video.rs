use super::*;

#[test]
#[ignore = "requires provisioned libmpv; verifies paused-frame pixels for Live Text"]
fn live_text_capture_returns_a_tightly_packed_frame_only_while_paused() {
    let root = fixture_root();
    // The null driver's default 256-sample chunk exceeds its buffer for the
    // 1 kHz WAV below. Keep its playback clock without relying on audio reuse.
    let Harness { player, receiver } =
        Harness::with_options(&root, vec![("ao-null-outburst".into(), "128".into())]).unwrap();
    player
        .send(Command::Load(
            root.join("tests/fixtures/catalog.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    let playing = wait_for(&receiver, |state| state.phase == Phase::Playing);
    player.send(Command::CaptureFrame).unwrap();
    let rejected = wait_for(&receiver, |state| {
        state.finished_frame_capture_requests > playing.finished_frame_capture_requests
    });
    assert_eq!(
        rejected.captured_frame.revision,
        playing.captured_frame.revision
    );
    assert!(rejected.error().is_some());
    player.send(Command::SetPaused(true)).unwrap();
    let paused = wait_for(&receiver, |state| state.phase == Phase::Paused);
    player.send(Command::CaptureFrame).unwrap();
    let captured = wait_for(&receiver, |state| {
        state.captured_frame.revision > paused.captured_frame.revision
    });
    assert_eq!(
        captured.finished_frame_capture_requests,
        paused.finished_frame_capture_requests + 1
    );
    let frame = captured.captured_frame.frame.unwrap();
    assert!(frame.width > 0 && frame.height > 0);
    assert_eq!(
        frame.rgba.len(),
        frame.width as usize * frame.height as usize * 4
    );
    player.send(Command::SetPauseWhenOpen(true)).unwrap();
    player
        .send(Command::Load(
            root.join("tests/fixtures/long-silence.wav")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    let audio = wait_for(&receiver, |state| {
        state.generation > captured.generation && state.phase == Phase::Paused
    });
    player.send(Command::CaptureFrame).unwrap();
    let failed = wait_for(&receiver, |state| {
        state.finished_frame_capture_requests > audio.finished_frame_capture_requests
    });
    assert_eq!(
        failed.captured_frame.revision,
        audio.captured_frame.revision
    );
    assert!(failed.captured_frame.frame.is_none());
}

#[expect(
    clippy::expect_used,
    reason = "Invalid fixtures must fail this native test"
)]
fn longer_thumbnail_fixture(source: &std::path::Path, destination: &std::path::Path) {
    // Extend the original MJPEG/PCM packets without introducing another fixture
    // or encoder. The shorter file does not trigger the decoder's paused EOF case.
    const REPEATS: u32 = 7;
    let source = std::fs::read(source).expect("read the original AVI fixture");
    let offset = |tag: &[u8]| {
        source
            .windows(tag.len())
            .position(|bytes| bytes == tag)
            .expect("locate the original AVI header field")
    };
    let dword = |bytes: &[u8], start: usize| {
        u32::from_le_bytes(
            bytes[start..start + 4]
                .try_into()
                .expect("read a 32-bit AVI field"),
        )
    };
    let movi = offset(b"movi");
    let movi_size = dword(&source, movi - 4);
    let packets_size = movi_size - 4;
    let index_start = movi + movi_size as usize;
    assert_eq!(&source[index_start..index_start + 4], b"idx1");
    let packets = &source[movi + 4..index_start];
    let mut output = source[..movi + 4].to_vec();
    for count in [
        offset(b"avih") + 24,
        offset(b"vids") + 32,
        offset(b"auds") + 32,
    ] {
        let length = dword(&source, count) * REPEATS;
        output[count..count + 4].copy_from_slice(&length.to_le_bytes());
    }
    output[movi - 4..movi].copy_from_slice(&(4 + packets_size * REPEATS).to_le_bytes());
    for _ in 0..REPEATS {
        output.extend_from_slice(packets);
    }
    output.extend_from_slice(b"idx1");
    let index_size = dword(&source, index_start + 4);
    output.extend_from_slice(&(index_size * REPEATS).to_le_bytes());
    let entries = &source[index_start + 8..index_start + 8 + index_size as usize];
    let (entries, trailing) = entries.as_chunks::<16>();
    assert!(trailing.is_empty());
    for repeat in 0..REPEATS {
        for entry in entries {
            let packet_offset = dword(entry, 8) + packets_size * repeat;
            output.extend_from_slice(&entry[..8]);
            output.extend_from_slice(&packet_offset.to_le_bytes());
            output.extend_from_slice(&entry[12..]);
        }
    }
    let riff_size = u32::try_from(output.len() - 8).expect("the extended fixture fits AVI limits");
    output[4..8].copy_from_slice(&riff_size.to_le_bytes());
    std::fs::write(destination, output).expect("write the extended AVI fixture");
}

#[test]
#[ignore = "requires provisioned libmpv; decodes all thumbnail samples through paused EOF"]
fn thumbnail_generator_extracts_one_hundred_and_one_isolated_frames() {
    let root = fixture_root();
    let directory = riina_test_support::TempDir::new("thumbnail-eof").unwrap();
    let media = directory.path().join("thumbnail-eof.avi");
    longer_thumbnail_fixture(&root.join("tests/fixtures/moving-square.avi"), &media);
    let generator = riina_mpv::thumbnail_generator::Generator::start(
        &test_library(&root),
        media.to_str().unwrap(),
        64,
    )
    .unwrap();
    let mut samples = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        assert!(Instant::now() < deadline, "thumbnail generation timed out");
        let event = match generator.try_recv() {
            Ok(event) => event,
            Err(mpsc::TryRecvError::Empty) => {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(mpsc::TryRecvError::Disconnected) => panic!("thumbnail worker stopped"),
        };
        match event {
            riina_mpv::thumbnail_generator::Event::Sample(sample) => {
                assert_eq!((sample.frame.width, sample.frame.height), (64, 36));
                samples.push(sample);
            }
            riina_mpv::thumbnail_generator::Event::Finished(Ok(())) => break,
            riina_mpv::thumbnail_generator::Event::Finished(Err(error)) => panic!("{error}"),
        }
    }
    assert_eq!(samples.len(), 101);
    assert_eq!(samples.first().unwrap().index, 0);
    assert_eq!(samples.last().unwrap().index, 100);
    assert!((samples.last().unwrap().timestamp - 34.999).abs() < 0.001);
    assert!(
        samples
            .windows(2)
            .all(|pair| pair[1].timestamp >= pair[0].timestamp)
    );
    assert!(
        samples
            .windows(2)
            .any(|pair| pair[0].frame.rgba != pair[1].frame.rgba)
    );
}

#[test]
#[ignore = "requires provisioned libmpv; verifies viewport option acceptance and playback isolation"]
fn picture_viewport_preserves_pause_volume_and_source_geometry() {
    use riina_core::{crop_geometry::Projection, video_viewport::Viewport};
    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    harness
        .send(Command::Load(
            root.join("tests/fixtures/catalog.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    harness.wait(|state| state.phase == Phase::Playing && state.video.is_some());
    harness.send(Command::TogglePause).unwrap();
    let paused = harness.wait(|state| state.paused);
    harness.send(Command::Volume(37.0)).unwrap();
    harness.wait(|state| state.volume == 37.0);
    for (index, size) in [(960.0, 600.0), (520.0, 300.0)].into_iter().enumerate() {
        let picture = Projection::interactive(paused.video.as_ref().unwrap(), size)
            .unwrap()
            .content();
        harness
            .send(Command::VideoViewport(Some(
                Viewport::new(size, picture).unwrap(),
            )))
            .unwrap();
        let speed = 1.25 + index as f64 * 0.25;
        // A distinct observed property follows all four queued margin setters.
        harness.send(Command::Speed(speed)).unwrap();
        let state = harness.wait(|state| state.speed == speed);
        assert!(state.paused);
        assert_eq!(state.volume, 37.0);
        assert_eq!(
            state.video.as_ref().map(|video| (video.w, video.h)),
            Some((320, 180))
        );
        assert_eq!(
            state.completed_navigation_seeks,
            paused.completed_navigation_seeks
        );
        assert_eq!(state.error(), None);
    }
    harness.send(Command::VideoViewport(None)).unwrap();
    harness.send(Command::Speed(1.0)).unwrap();
    let reset = harness.wait(|state| state.speed == 1.0);
    assert!(reset.paused);
    assert_eq!(reset.volume, 37.0);
    assert_eq!(reset.error(), None);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies independent flip/mirror managed filters"]
fn video_reflection_axes_preserve_crop_and_playback() {
    use riina_core::{crop::Preset, video_reflection::Axis};
    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    harness
        .send(Command::Load(
            root.join("tests/fixtures/catalog.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    harness.wait(|state| state.phase == Phase::Playing && state.video_filters.is_some());
    harness.send(Command::TogglePause).unwrap();
    harness.wait(|state| state.paused);
    harness.send(Command::Volume(37.0)).unwrap();
    harness.wait(|state| state.volume == 37.0);
    harness
        .send(Command::VideoCrop(Some(Preset::FourThree)))
        .unwrap();
    let cropped = harness.wait(|state| {
        riina_core::crop::observed_index(state) == Some(1)
            && state.video_output.as_ref().map(|video| (video.w, video.h)) == Some((240, 180))
    });
    for (axis, enabled, horizontal, vertical) in [
        (Axis::Horizontal, true, true, false),
        (Axis::Vertical, true, true, true),
        (Axis::Horizontal, false, false, true),
        (Axis::Vertical, false, false, false),
    ] {
        harness
            .send(Command::VideoReflection { axis, enabled })
            .unwrap();
        let state = harness.wait(|state| {
            Axis::Horizontal.observed(state.video_filters.as_deref()) == Some(horizontal)
                && Axis::Vertical.observed(state.video_filters.as_deref()) == Some(vertical)
        });
        assert_eq!(riina_core::crop::observed_index(&state), Some(1));
        assert!(state.paused);
        assert_eq!(state.volume, 37.0);
        assert_eq!(
            state.completed_navigation_seeks,
            cropped.completed_navigation_seeks
        );
        assert_eq!(state.error(), None);
        // Repeated explicit state is idempotent, including removal of absent labels.
        harness
            .send(Command::VideoReflection { axis, enabled })
            .unwrap();
    }
    harness.send(Command::Speed(1.25)).unwrap();
    let settled = harness.wait(|state| state.speed == 1.25);
    assert_eq!(settled.error(), None);
    assert_eq!(
        settled
            .video_output
            .as_ref()
            .map(|video| (video.w, video.h)),
        Some((240, 180))
    );
    harness
        .send(Command::ToggleVideoReflection(Axis::Horizontal))
        .unwrap();
    harness.wait(|state| Axis::Horizontal.observed(state.video_filters.as_deref()) == Some(true));
    harness
        .send(Command::ToggleVideoReflection(Axis::Horizontal))
        .unwrap();
    harness.wait(|state| Axis::Horizontal.observed(state.video_filters.as_deref()) == Some(false));
    // Native chain ordering must survive a burst without UI observation.
    for axis in [
        Axis::Horizontal,
        Axis::Vertical,
        Axis::Horizontal,
        Axis::Vertical,
    ] {
        harness.send(Command::ToggleVideoReflection(axis)).unwrap();
    }
    harness.send(Command::Speed(1.5)).unwrap();
    let toggled = harness.wait(|state| state.speed == 1.5);
    assert_eq!(
        Axis::Horizontal.observed(toggled.video_filters.as_deref()),
        Some(false)
    );
    assert_eq!(
        Axis::Vertical.observed(toggled.video_filters.as_deref()),
        Some(false)
    );
    assert_eq!(riina_core::crop::observed_index(&toggled), Some(1));
    assert!(toggled.paused);
    assert_eq!(toggled.volume, 37.0);
    assert_eq!(
        toggled.completed_navigation_seeks,
        cropped.completed_navigation_seeks
    );
    assert_eq!(toggled.error(), None);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies native rejection of an above-limit custom speed"]
fn excessive_custom_speed_reports_native_error_without_changing_playback() {
    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    harness
        .send(Command::Load(
            root.join("tests/fixtures/catalog.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    harness.wait(|state| state.phase == Phase::Playing);
    harness.send(Command::TogglePause).unwrap();
    harness.wait(|state| state.paused);
    harness.send(Command::Volume(37.0)).unwrap();
    harness.wait(|state| state.volume == 37.0);
    harness.send(Command::Speed(1.25)).unwrap();
    harness.wait(|state| state.speed == 1.25);
    let request = riina_core::speed_controls::from_text("101").unwrap();
    harness.send(request).unwrap();
    let rejected = harness.wait(|state| state.error().is_some());
    assert_eq!(rejected.speed, 1.25);
    assert!(rejected.paused);
    assert_eq!(rejected.volume, 37.0);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies speed slider ticks and precise custom entries"]
fn speed_controls_preserve_pause_volume_and_custom_precision() {
    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    harness
        .send(Command::Load(
            root.join("tests/fixtures/catalog.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    harness.wait(|state| state.phase == Phase::Playing);
    harness.send(Command::TogglePause).unwrap();
    harness.wait(|state| state.paused);
    harness.send(Command::Volume(37.0)).unwrap();
    harness.wait(|state| state.volume == 37.0);
    for (command, expected) in [
        (riina_core::speed_controls::from_slider(0.0).unwrap(), 0.25),
        (riina_core::speed_controls::from_slider(9.0).unwrap(), 1.19),
        (riina_core::speed_controls::from_slider(24.0).unwrap(), 16.0),
        (
            riina_core::speed_controls::from_text("1.2345675").unwrap(),
            1.234567,
        ),
        (riina_core::speed_controls::from_text("0.01").unwrap(), 0.01),
        (riina_core::speed_controls::from_text("32").unwrap(), 32.0),
        (riina_core::speed_controls::from_text("").unwrap(), 1.0),
    ] {
        harness.send(command).unwrap();
        let state = harness.wait(|state| (state.speed - expected).abs() < 0.0000001);
        assert!(state.paused);
        assert_eq!(state.volume, 37.0);
        assert!(state.error().is_none(), "{:?}", state.error());
    }
}

#[test]
#[ignore = "requires provisioned libmpv; verifies configured decoding independently of codec fallback"]
fn hardware_decoding_policy_survives_fallback_and_file_replacement() {
    use riina_core::video_controls::HardwareDecoder;
    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    let fixture = root
        .join("tests/fixtures/catalog.mkv")
        .to_string_lossy()
        .into_owned();
    harness.send(Command::Load(fixture.clone())).unwrap();
    harness.wait(|state| state.phase == Phase::Playing);
    harness.send(Command::TogglePause).unwrap();
    harness.wait(|state| state.paused);
    harness.send(Command::Volume(37.0)).unwrap();
    harness.wait(|state| state.volume == 37.0);
    for mode in [
        HardwareDecoder::Disabled,
        HardwareDecoder::AutoCopy,
        HardwareDecoder::Auto,
    ] {
        harness.send(Command::HardwareDecoding(mode)).unwrap();
        let state =
            harness.wait(|state| state.hardware_decoding.as_deref() == Some(mode.native_value()));
        assert!(state.paused);
        assert_eq!(state.volume, 37.0);
        assert!(state.error().is_none(), "{:?}", state.error());
        // Headless MJPEG has no hardware presentation. The configured switch
        // must still be on for auto modes instead of following fallback state.
        assert!(matches!(
            state.hardware_decoder.as_deref(),
            None | Some("no")
        ));
    }
    harness
        .send(Command::HardwareDecoding(HardwareDecoder::AutoCopy))
        .unwrap();
    let previous = harness.wait(|state| state.hardware_decoding.as_deref() == Some("auto-copy"));
    harness.send(Command::Load(fixture)).unwrap();
    let replaced = harness
        .wait(|state| state.generation > previous.generation && state.phase == Phase::Playing);
    assert_eq!(replaced.hardware_decoding.as_deref(), Some("auto-copy"));
    // Explicit replacement resumes playback by contract; the decoding policy
    // must survive that transition rather than resetting to startup defaults.
    assert_eq!(replaced.volume, 37.0);
    assert!(replaced.error().is_none(), "{:?}", replaced.error());
}

#[test]
#[ignore = "requires provisioned libmpv; verifies Unicode ratio text is not silently normalized"]
fn unicode_aspect_input_preserves_text_and_native_failure_state() {
    use riina_core::video_controls::AspectRatio;
    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    harness
        .send(Command::Load(
            root.join("tests/fixtures/catalog.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    harness.wait(|state| state.phase == Phase::Playing);
    harness.send(Command::TogglePause).unwrap();
    harness.wait(|state| state.paused);
    harness.send(Command::Volume(37.0)).unwrap();
    harness.wait(|state| state.volume == 37.0);
    harness
        .send(Command::VideoAspect(AspectRatio::from_text("4:3")))
        .unwrap();
    let mut accepted = harness.wait(|state| {
        state.video_aspect_override == Some(4.0 / 3.0) && state.video_aspect.label() == "4:3"
    });
    // Equal numeric ratios and repeated requests still carry distinct intent
    // and feedback. Native property-change events alone cannot establish this.
    for text in ["8:6", "8:6", "4:3"] {
        let previous = accepted.aspect_requests;
        harness
            .send(Command::VideoAspect(AspectRatio::from_text(text)))
            .unwrap();
        accepted = harness.wait(|state| {
            state.aspect_requests != previous
                && state.video_aspect.label() == text
                && state.video_aspect_override == Some(4.0 / 3.0)
        });
        assert!(accepted.paused);
        assert_eq!(accepted.volume, 37.0);
        assert_eq!(
            accepted.video_aspect.menu_index(),
            (text == "4:3").then_some(1)
        );
    }
    harness
        .send(Command::VideoAspect(AspectRatio::from_text("١٦:٩")))
        .unwrap();
    let rejected = harness.wait(|state| state.error().is_some());
    assert_eq!(rejected.video_aspect.label(), "١٦:٩");
    assert_eq!(rejected.video_aspect.requested_text(), "١٦:٩");
    assert_eq!(rejected.aspect_requests, accepted.aspect_requests + 1);
    assert_eq!(rejected.video_aspect_override, Some(4.0 / 3.0));
    assert!(rejected.paused);
    assert_eq!(rejected.volume, 37.0);
    assert_eq!(rejected.completed_navigation_seeks, 0);
    harness
        .send(Command::Load(
            root.join("tests/fixtures/catalog.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    let replaced = harness.wait(|state| {
        state.generation > rejected.generation
            && state.phase == Phase::Playing
            && state.video_aspect_override == Some(4.0 / 3.0)
    });
    assert_eq!(replaced.video_aspect.label(), "١٦:٩");
    assert_eq!(replaced.aspect_requests, rejected.aspect_requests);
    assert_eq!(replaced.error(), None);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies video aspect and rotation while paused"]
fn video_geometry_controls_preserve_pause_and_unrelated_options() {
    use riina_core::video_controls::{ASPECT_MENU_PRESETS, AspectRatio, Rotation};
    let root = fixture_root();
    let Harness { player, receiver } = Harness::start(&root).unwrap();
    player
        .send(Command::Load(
            root.join("tests/fixtures/catalog.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    let loaded = wait_for(&receiver, |state| {
        state.phase == Phase::Playing && state.video_output.is_some()
    });
    let video_track = loaded
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Video)
        .unwrap();
    assert_eq!(video_track.width, Some(320));
    assert_eq!(video_track.height, Some(180));
    assert!(
        video_track
            .frame_rate
            .is_some_and(|rate| (rate - 24.0).abs() < 1e-5)
    );
    let track_label = video_track.display_name();
    assert_eq!(track_label, "No Title (mjpeg, 320×180, 24fps, Default)");
    // The current engine reports its initial "no" override as -2. The explicit
    // Default action uses the reference player's container-preference sentinel.
    assert!(matches!(loaded.video_aspect_override, Some(-2.0 | -1.0)));
    assert_eq!(loaded.video_rotation, Some(0));
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| state.paused);
    player.send(Command::Volume(37.0)).unwrap();
    wait_for(&receiver, |state| state.volume == 37.0);
    for ratio in ASPECT_MENU_PRESETS
        .into_iter()
        .skip(1)
        .chain(["21:9", "8:6"])
    {
        let (width, height) = ratio.split_once(':').unwrap();
        let expected = width.parse::<f64>().unwrap() / height.parse::<f64>().unwrap();
        player
            .send(Command::VideoAspect(AspectRatio::from_text(ratio)))
            .unwrap();
        let changed = wait_for(&receiver, |state| {
            state.video_aspect.label() == ratio
                && state
                    .video_aspect_override
                    .is_some_and(|value| (value - expected).abs() < 1e-9)
                && state.video_output.as_ref().is_some_and(|video| {
                    video
                        .dw
                        .zip(video.dh)
                        .is_some_and(|(w, h)| ((w as f64 / h as f64) - expected).abs() < 0.01)
                })
        });
        assert!(changed.paused);
        assert_eq!(changed.volume, 37.0);
        assert_eq!(changed.video_rotation, Some(0));
        assert_eq!(changed.error(), None);
        assert_eq!(
            changed
                .tracks
                .iter()
                .find(|track| track.kind == TrackKind::Video)
                .unwrap()
                .display_name(),
            track_label,
        );
    }
    player
        .send(Command::VideoAspect(AspectRatio::from_text("invalid")))
        .unwrap();
    let reset = wait_for(&receiver, |state| state.video_aspect_override == Some(-1.0));
    assert_eq!(reset.video_aspect.label(), "Default");
    assert_eq!(reset.video_aspect.requested_text(), "invalid");
    for degrees in [90, 180, 270, 0] {
        player
            .send(Command::VideoRotation(
                Rotation::from_degrees(degrees).unwrap(),
            ))
            .unwrap();
        let changed = wait_for(&receiver, |state| state.video_rotation == Some(degrees));
        assert!(changed.paused);
        assert_eq!(changed.volume, 37.0);
        assert_eq!(changed.video_aspect_override, Some(-1.0));
        assert_eq!(changed.error(), None);
        assert_eq!(
            changed
                .tracks
                .iter()
                .find(|track| track.kind == TrackKind::Video)
                .unwrap()
                .display_name(),
            track_label,
        );
    }
}

#[test]
#[ignore = "requires provisioned libmpv; decodes original PQ/HLG and checks replacement metadata"]
fn hdr_metadata_replacement_and_stop() {
    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    let mut generation = 0;
    for (file, gamma, pixel_format, size) in [
        ("hdr-pq.mkv", "pq", "yuv422p10", (192, 108)),
        ("hdr-hlg.mkv", "hlg", "yuv422p10", (192, 108)),
        ("catalog.mkv", "srgb", "yuv420p", (320, 180)),
    ] {
        harness
            .send(Command::Load(
                root.join("tests/fixtures")
                    .join(file)
                    .to_str()
                    .unwrap()
                    .into(),
            ))
            .unwrap();
        let loaded = harness.wait(|state| {
            state.generation > generation
                && state.position.is_some_and(|p| p > 0.1)
                && state
                    .video
                    .as_ref()
                    .is_some_and(|video| video.pixelformat.is_some())
        });
        generation = loaded.generation;
        assert_eq!(loaded.error(), None);
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
    harness.send(Command::Stop).unwrap();
    let stopped = harness.wait(|state| state.phase == Phase::Stopped && state.video.is_none());
    assert_eq!(stopped.error(), None);
    drop(harness);
}
