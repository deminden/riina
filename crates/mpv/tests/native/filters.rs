use super::*;
use riina_mpv::EngineError;

#[test]
#[ignore = "requires provisioned libmpv; submits filter toggles without waiting for property notifications"]
fn rapid_audio_and_video_toggles_consume_each_previous_native_edit() -> Result<(), EngineError> {
    let harness = paused_filter_fixture()?;
    let initial = harness.wait(|state| {
        state.phase == Phase::Paused
            && state.audio_filters.as_ref().is_some_and(Vec::is_empty)
            && state.video_filters.as_ref().is_some_and(Vec::is_empty)
    });
    let sender = harness.player.command_sender();
    let mut previous_reply = 0;
    for command in [
        Command::ToggleAudioFilter("lavfi=[volume=0.5]".into()),
        Command::ToggleVideoFilter("hflip".into()),
    ] {
        for _ in 0..3 {
            for _ in 0..2 {
                sender.send_filter_edit(initial.generation, command.clone(), "Preset".into())?;
            }
            let state = harness.wait(|state| {
                state.filter_feedback.as_ref().is_some_and(|feedback| {
                    feedback.sequence > previous_reply
                        && feedback.change == riina_core::FilterChange::Removed
                }) && state.audio_filters.as_ref().is_some_and(Vec::is_empty)
                    && state.video_filters.as_ref().is_some_and(Vec::is_empty)
            });
            previous_reply = state.filter_feedback.as_ref().unwrap().sequence;
            assert_eq!(state.generation, initial.generation);
            assert_eq!(
                state.completed_navigation_seeks,
                initial.completed_navigation_seeks
            );
            assert_eq!(state.error(), None);
            assert!(state.paused);
        }
    }
    Ok(())
}

#[test]
#[ignore = "requires provisioned libmpv; checks row identity and an obsolete filter chain"]
fn indexed_edits_keep_the_selected_occurrence_and_reject_a_changed_chain() -> Result<(), EngineError>
{
    let harness = paused_filter_fixture()?;
    let initial = harness.wait(|state| state.phase == Phase::Paused);
    let sender = harness.player.command_sender();
    for filter in ["hflip", "vflip", "hflip"] {
        harness.send(Command::AddVideoFilter(filter.into()))?;
    }
    let filter_names = |state: &Playback| {
        state
            .video_filters
            .as_ref()
            .map(|filters| {
                filters
                    .iter()
                    .map(|filter| filter.name.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    let before = harness.wait(|state| filter_names(state) == ["hflip", "vflip", "hflip"]);
    let displayed = before.video_filters.unwrap();
    sender.send_indexed_filter_edit(
        initial.generation,
        Command::RemoveVideoFilterAt(2),
        displayed.clone(),
    )?;
    sender.send_indexed_filter_edit(
        initial.generation,
        Command::SetVideoFilterAt {
            index: 0,
            filter: "vflip".into(),
        },
        displayed,
    )?;
    let rejected = harness.wait(|state| {
        filter_names(state) == ["hflip", "vflip"]
            && state
                .error()
                .is_some_and(|message| message.contains("active filters changed"))
    });
    let removed = rejected.filter_feedback.unwrap();
    assert_eq!(removed.change, riina_core::FilterChange::Removed);

    sender.send_indexed_filter_edit(
        initial.generation,
        Command::SetVideoFilterAt {
            index: 0,
            filter: "vflip".into(),
        },
        rejected.video_filters.unwrap(),
    )?;
    let edited = harness.wait(|state| filter_names(state) == ["vflip", "vflip"]);
    assert_eq!(edited.filter_feedback.unwrap(), removed);
    assert!(edited.paused);
    Ok(())
}

fn paused_filter_fixture() -> Result<Harness, EngineError> {
    let root = fixture_root();
    let harness = Harness::start(&root)?;
    harness.send(Command::SetPauseWhenOpen(true))?;
    harness.send(Command::Load(
        root.join("tests/fixtures/catalog.mkv")
            .to_string_lossy()
            .into_owned(),
    ))?;
    Ok(harness)
}

#[test]
#[ignore = "requires provisioned libmpv; checks native filter replies and media ownership"]
fn user_filter_feedback_requires_native_success_for_the_current_media() {
    use riina_core::FilterChange;

    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    let media = |name: &str| {
        Command::Load(
            root.join("tests/fixtures")
                .join(name)
                .to_string_lossy()
                .into_owned(),
        )
    };
    harness.send(Command::SetPauseWhenOpen(true)).unwrap();
    harness.send(media("catalog.mkv")).unwrap();
    let initial = harness.wait(|state| {
        state.phase == Phase::Paused
            && state.video_filters.is_some()
            && state.audio_filters.is_some()
    });
    let sender = harness.player.command_sender();
    harness
        .send(Command::AddVideoFilter("@keep:lavfi=[hflip,vflip]".into()))
        .unwrap();
    let untouched = harness
        .wait(|state| {
            state.video_filters.as_ref().is_some_and(|filters| {
                filters.len() == 1 && filters[0].label.as_deref() == Some("keep")
            })
        })
        .video_filters
        .unwrap();
    let acknowledged = |predicate: fn(&FilterChange) -> bool| {
        harness.wait(|state| {
            state
                .filter_feedback
                .as_ref()
                .is_some_and(|reply| predicate(&reply.change))
        })
    };

    sender
        .send_filter_edit(
            initial.generation,
            Command::ToggleVideoFilter("hflip".into()),
            "Mirror".into(),
        )
        .unwrap();
    acknowledged(|change| matches!(change, FilterChange::Added(name) if name.as_ref() == "Mirror"));
    harness.wait(|state| {
        state
            .video_filters
            .as_ref()
            .is_some_and(|filters| filters.len() == 2)
    });
    sender
        .send_filter_edit(
            initial.generation,
            Command::ToggleVideoFilter("hflip".into()),
            "Mirror".into(),
        )
        .unwrap();
    acknowledged(|change| *change == FilterChange::Removed);
    harness.wait(|state| state.video_filters.as_ref() == Some(&untouched));

    sender
        .send_filter_edit(
            initial.generation,
            Command::AddAudioFilter("lavfi=[volume=0.5]".into()),
            "Attenuate".into(),
        )
        .unwrap();
    let accepted = acknowledged(
        |change| matches!(change, FilterChange::Added(name) if name.as_ref() == "Attenuate"),
    );
    sender
        .send_filter_edit(
            initial.generation,
            Command::AddVideoFilter("riina_no_such_filter".into()),
            "Invalid".into(),
        )
        .unwrap();
    let rejected = harness.wait(|state| state.error().is_some());
    assert_eq!(rejected.filter_feedback, accepted.filter_feedback);
    assert!(rejected.paused);

    harness.send(media("moving-square.avi")).unwrap();
    let replacement = harness
        .wait(|state| state.generation != initial.generation && state.phase == Phase::Paused);
    assert_eq!(replacement.filter_feedback, None);
    sender
        .send_filter_edit(
            initial.generation,
            Command::AddVideoFilter("vflip".into()),
            "Stale".into(),
        )
        .unwrap();
    harness.send(Command::Speed(1.25)).unwrap();
    let observed = harness.wait(|state| state.speed == 1.25);
    assert_eq!(observed.filter_feedback, None);
    assert_eq!(observed.video_filters, replacement.video_filters);
}

#[test]
#[ignore = "requires provisioned libmpv; applies and replaces a 3D LUT during playback"]
fn dropped_luts_with_delimiters_replace_only_the_labelled_filter() {
    use riina_core::video_filters::{QUICK_LUT_LABEL, format_filter, quick_lut_filter};

    let root = fixture_root();
    let directory = riina_test_support::TempDir::new("native-quick-lut").unwrap();
    let harness = Harness::start(&root).unwrap();
    let video = root.join("tests/fixtures/moving-square.avi");
    harness
        .send(Command::Load(video.to_str().unwrap().into()))
        .unwrap();
    let loaded = harness.wait(|state| state.phase == Phase::Playing);
    harness.send(Command::SetPaused(true)).unwrap();
    harness.wait(|state| state.paused && state.position.is_some());
    harness
        .send(Command::AddVideoFilter("hflip".into()))
        .unwrap();

    for filename in ["look [café], first's.cube", "replacement ] красный.cube"] {
        let path = directory.path().join(filename);
        let lut = format!("LUT_3D_SIZE 2\n{}", "0 1 0\n".repeat(8));
        std::fs::write(&path, lut).unwrap();
        let filter = quick_lut_filter(path.to_str().unwrap()).unwrap();
        harness
            .send(Command::AddVideoFilter(filter.clone()))
            .unwrap();
        let observed = harness.wait(|state| {
            state.error().is_some()
                || state.video_filters.as_deref().is_some_and(|filters| {
                    filters.iter().any(|active| format_filter(active) == filter)
                })
        });
        assert_eq!(observed.error(), None);
        assert_eq!(observed.generation, loaded.generation);
        assert!(observed.paused);
        let filters = observed.video_filters.as_deref().unwrap();
        assert_eq!(filters.len(), 2);
        assert_eq!(filters[0].name, "hflip");
        assert_eq!(filters[1].label.as_deref(), Some(QUICK_LUT_LABEL));
    }
}

#[test]
#[ignore = "requires provisioned libmpv; verifies original woven fields and 25-to-50 Hz cadence"]
fn interlaced_fields_decode_and_deinterlace_at_field_cadence() {
    let root = fixture_root();
    let output_dir = CaptureOutput::new("interlaced-native-test").unwrap();
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
