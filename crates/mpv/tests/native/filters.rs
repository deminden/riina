use super::*;

#[test]
#[ignore = "requires provisioned libmpv; verifies original woven fields and 25-to-50 Hz cadence"]
fn interlaced_fields_decode_and_deinterlace_at_field_cadence() {
    let root = fixture_root();
    let output_dir = riina_test_support::TempDir::new("interlaced-native-test").unwrap();
    let output = output_dir.path().to_path_buf();
    let mut captures = Vec::new();
    for name in [
        "interlaced-tff.mkv",
        "interlaced-bff.mkv",
        "interlaced-mpeg2-tff.mkv",
        "interlaced-mpeg2-bff.mkv",
    ] {
        let Harness { player, receiver } = Harness::start(&root).unwrap();
        player
            .send(Command::HardwareDecoding(
                riina_core::video_controls::HardwareDecoder::Disabled,
            ))
            .unwrap();
        player
            .send(Command::Load(
                root.join("tests/fixtures")
                    .join(name)
                    .to_string_lossy()
                    .into_owned(),
            ))
            .unwrap();
        wait_for(&receiver, |state| {
            state.phase == Phase::Playing && state.position.is_some()
        });
        player.send(Command::TogglePause).unwrap();
        let mut previous = wait_for(&receiver, |state| state.paused && state.position.is_some());
        assert_eq!(previous.deinterlace, Some(false));
        assert_eq!(previous.deinterlace_active, Some(false));
        assert_eq!(
            previous.video.as_ref().map(|video| (video.w, video.h)),
            Some((96, 64))
        );
        for enabled in [false, true, false] {
            player.send(Command::Deinterlace(enabled)).unwrap();
            previous = wait_for(&receiver, |state| {
                state.paused
                    && !state.seeking
                    && state.deinterlace == Some(enabled)
                    && state.deinterlace_active == Some(enabled)
                    && state.position.is_some()
            });
            for _ in 0..6 {
                let from = previous.position.unwrap();
                player.send(Command::FrameStep).unwrap();
                let next = wait_for(&receiver, |state| {
                    state.paused
                        && !state.seeking
                        && state
                            .position
                            .is_some_and(|position| position > from + 0.005)
                });
                let delta = next.position.unwrap() - from;
                let expected = if enabled { 0.020 } else { 0.040 };
                assert!(
                    (delta - expected).abs() < 0.002,
                    "{name}: enabled={enabled}, step={delta}"
                );
                assert_eq!(next.error(), None);
                assert_eq!(
                    next.video_output.as_ref().map(|video| (video.w, video.h)),
                    Some((96, 64))
                );
                let path = output.join(format!("capture-{}.png", captures.len()));
                player
                    .send(Command::ScreenshotToFile {
                        path: path.to_string_lossy().into_owned(),
                        content: riina_core::screenshot::Content::Video,
                    })
                    .unwrap();
                let captured = wait_for(&receiver, |state| {
                    state.completed_screenshots > next.completed_screenshots
                        || state.error().is_some()
                });
                assert_eq!(captured.error(), None);
                assert!(captured.paused);
                assert_eq!(captured.position, next.position);
                captures.push(serde_json::json!({
                    "fixture": name, "deinterlace": enabled,
                    "position": captured.position, "path": path,
                }));
                previous = captured;
            }
            println!(
                "{name}: deinterlace={enabled}; six {} ms steps; frame flags {:?}",
                if enabled { 20 } else { 40 },
                previous.video_frame_info
            );
        }
    }
    let manifest = output.join("captures.json");
    std::fs::write(&manifest, serde_json::to_vec_pretty(&captures).unwrap()).unwrap();
    println!("Native field captures: {}", manifest.display());
}

#[test]
#[ignore = "requires provisioned libmpv; verifies deinterlace state and automatic filter coexistence"]
fn deinterlace_preserves_filters_playback_and_two_state_toggles() {
    use riina_core::{crop::Preset, video_reflection::Axis};
    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    let fixture = root
        .join("tests/fixtures/catalog.mkv")
        .to_string_lossy()
        .into_owned();
    harness.send(Command::Load(fixture.clone())).unwrap();
    let initial = harness.wait(|state| {
        state.phase == Phase::Playing
            && state.deinterlace == Some(false)
            && state.deinterlace_active == Some(false)
    });
    harness.send(Command::TogglePause).unwrap();
    harness.wait(|state| state.paused);
    harness.send(Command::Volume(37.0)).unwrap();
    harness.wait(|state| state.volume == 37.0);
    harness.send(Command::Speed(1.5)).unwrap();
    harness.wait(|state| state.speed == 1.5);
    harness
        .send(Command::VideoCrop(Some(Preset::FourThree)))
        .unwrap();
    harness.wait(|state| riina_core::crop::observed_index(state) == Some(1));
    harness
        .send(Command::VideoReflection {
            axis: Axis::Horizontal,
            enabled: true,
        })
        .unwrap();
    harness.wait(|state| Axis::Horizontal.observed(state.video_filters.as_deref()) == Some(true));
    for (index, enabled) in [true, true, false, false, true].into_iter().enumerate() {
        harness.send(Command::Deinterlace(enabled)).unwrap();
        // The speed barrier distinguishes a repeated no-op from a stale snapshot.
        let speed = 1.0 + index as f64 * 0.125;
        harness.send(Command::Speed(speed)).unwrap();
        let changed = harness.wait(|state| {
            state.deinterlace == Some(enabled)
                && state.deinterlace_active == Some(enabled)
                && state.speed == speed
        });
        assert!(changed.paused);
        assert_eq!(changed.volume, 37.0);
        assert_eq!(
            changed.completed_navigation_seeks,
            initial.completed_navigation_seeks
        );
        assert_eq!(changed.error(), None);
        assert_eq!(riina_core::crop::observed_index(&changed), Some(1));
        assert_eq!(
            Axis::Horizontal.observed(changed.video_filters.as_deref()),
            Some(true)
        );
        assert_eq!(
            changed.video.as_ref().map(|video| (video.w, video.h)),
            Some((320, 180))
        );
    }
    for _ in 0..3 {
        harness.send(Command::ToggleDeinterlace).unwrap();
    }
    harness.send(Command::Speed(2.0)).unwrap();
    let toggled = harness.wait(|state| {
        state.deinterlace == Some(false)
            && state.deinterlace_active == Some(false)
            && state.speed == 2.0
    });
    assert!(toggled.paused);
    assert_eq!(toggled.volume, 37.0);
    assert_eq!(toggled.error(), None);
    assert_eq!(
        toggled.completed_navigation_seeks,
        initial.completed_navigation_seeks
    );
    assert_eq!(riina_core::crop::observed_index(&toggled), Some(1));
    assert_eq!(
        Axis::Horizontal.observed(toggled.video_filters.as_deref()),
        Some(true)
    );
    harness.send(Command::Deinterlace(true)).unwrap();
    harness.wait(|state| state.deinterlace == Some(true) && state.deinterlace_active == Some(true));
    harness.send(Command::Load(fixture)).unwrap();
    let replaced = harness.wait(|state| {
        state.generation > toggled.generation
            && state.phase == Phase::Playing
            && state.deinterlace == Some(true)
            && state.deinterlace_active == Some(true)
    });
    assert_eq!(replaced.volume, 37.0);
    assert_eq!(replaced.error(), None);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies independent video equalizer channels and resets"]
fn video_equalizer_preserves_other_channels_and_playback_options() {
    use riina_core::video_controls::Equalizer;
    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    harness
        .send(Command::Load(
            root.join("tests/fixtures/catalog.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    harness.wait(|state| state.phase == Phase::Playing && state.video_equalizer == [Some(0); 5]);
    harness.send(Command::TogglePause).unwrap();
    harness.wait(|state| state.paused);
    harness.send(Command::Volume(37.0)).unwrap();
    harness.wait(|state| state.volume == 37.0);
    harness.send(Command::Speed(1.5)).unwrap();
    harness.wait(|state| state.speed == 1.5);
    let mut expected = [Some(0); 5];
    for (option, value) in Equalizer::ALL.into_iter().zip([-100, 100, -27, 13, 61]) {
        expected[option.index()] = Some(value);
        harness
            .send(Command::VideoEqualizer { option, value })
            .unwrap();
        let state = harness.wait(|state| state.video_equalizer == expected);
        assert!(state.paused);
        assert_eq!(state.volume, 37.0);
        assert_eq!(state.speed, 1.5);
        assert!(state.error().is_none(), "{:?}", state.error());
    }
    for option in Equalizer::ALL {
        expected[option.index()] = Some(0);
        harness
            .send(Command::VideoEqualizer { option, value: 0 })
            .unwrap();
        let state = harness.wait(|state| state.video_equalizer == expected);
        assert!(state.paused);
        assert_eq!(state.volume, 37.0);
        assert_eq!(state.speed, 1.5);
        assert!(state.error().is_none(), "{:?}", state.error());
    }
}
