use super::*;

#[test]
#[ignore = "requires provisioned libmpv; verifies zero-size crop behavior with explicit offsets"]
fn zero_size_crop_offsets_preserve_requested_parameters() {
    use riina_core::crop::{FILTER_LABEL, Rectangle};
    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    harness
        .send(Command::Load(
            root.join("tests/fixtures/catalog.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    harness.wait(|state| state.phase == Phase::Playing && state.video_output.is_some());
    harness.send(Command::TogglePause).unwrap();
    harness.wait(|state| state.paused);
    harness.send(Command::Volume(37.0)).unwrap();
    let baseline = harness.wait(|state| state.volume == 37.0);
    let mut completed = baseline.completed_seeks;
    for (width, height) in [(0, 160), (300, 0), (0, 0)] {
        harness
            .send(Command::VideoCropRectangle(Rectangle {
                x: 10,
                y: 12,
                width,
                height,
            }))
            .unwrap();
        let state = harness.wait(|state| {
            state.completed_seeks > completed
                && !state.seeking
                && state.video_output.as_ref().map(|video| (video.w, video.h)) == Some((320, 180))
                && state.video_filters.as_ref().is_some_and(|filters| {
                    filters.iter().any(|filter| {
                        filter.label.as_deref() == Some(FILTER_LABEL)
                            && filter.params.get("@0") == Some(&width.to_string())
                            && filter.params.get("@1") == Some(&height.to_string())
                    })
                })
        });
        assert!(state.paused);
        assert_eq!(state.volume, 37.0);
        assert_eq!(state.error, None);
        completed = state.completed_seeks;
        assert_eq!(
            state.completed_navigation_seeks,
            baseline.completed_navigation_seeks
        );
        let crop = state
            .video_filters
            .as_ref()
            .unwrap()
            .iter()
            .find(|filter| filter.label.as_deref() == Some(FILTER_LABEL))
            .unwrap();
        assert_eq!(crop.params.get("@2").map(String::as_str), Some("10"));
        assert_eq!(crop.params.get("@3").map(String::as_str), Some("12"));
        assert_eq!(
            state.video.as_ref().map(|video| (video.w, video.h)),
            Some((320, 180))
        );
    }
    harness.send(Command::VideoCrop(None)).unwrap();
    let reset = harness.wait(|state| riina_core::crop::observed_index(state) == Some(0));
    assert_eq!(reset.error, None);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies explicit zero-size crop native semantics"]
fn zero_size_crop_uses_native_full_source_dimensions() {
    use riina_core::crop::{FILTER_LABEL, Rectangle};
    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    harness
        .send(Command::Load(
            root.join("tests/fixtures/catalog.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    harness.wait(|state| state.phase == Phase::Playing && state.video_output.is_some());
    harness.send(Command::TogglePause).unwrap();
    harness.wait(|state| state.paused);
    harness
        .send(Command::VideoCropRectangle(Rectangle {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        }))
        .unwrap();
    let state = harness.wait(|state| {
        state.video_filters.as_ref().is_some_and(|filters| {
            filters
                .iter()
                .any(|filter| filter.label.as_deref() == Some(FILTER_LABEL))
        })
    });
    assert!(state.paused);
    assert_eq!(state.error, None);
    assert_eq!(
        state.video_output.as_ref().map(|video| (video.w, video.h)),
        Some((320, 180))
    );
    assert_eq!(riina_core::crop::observed_index(&state), Some(6));
    harness.send(Command::VideoCrop(None)).unwrap();
    let reset = harness.wait(|state| riina_core::crop::observed_index(state) == Some(0));
    assert_eq!(reset.error, None);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies managed crop replacement and source geometry"]
#[expect(
    clippy::too_many_lines,
    reason = "decomposition tracked in docs/review"
)]
fn crop_presets_replace_the_managed_crop_and_restore_source_output() {
    use riina_core::crop::{FILTER_LABEL, Preset};
    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    harness
        .send(Command::Load(
            root.join("tests/fixtures/catalog.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    harness.wait(|state| {
        state.phase == Phase::Playing
            && state.video_filters.is_some()
            && state.video_output.is_some()
    });
    harness.send(Command::TogglePause).unwrap();
    harness.wait(|state| state.paused);
    harness.send(Command::Volume(37.0)).unwrap();
    harness.wait(|state| state.volume == 37.0);
    for preset in Preset::MENU_ALL.into_iter().chain([Preset::TwentyOneNine]) {
        let (width, height) = preset.dimensions(320, 180).unwrap();
        // This fixture decodes to YUV420: libavfilter aligns odd dimensions
        // down to chroma boundaries, while vf retains the requested dimensions.
        let output_width = width & !1;
        let output_height = height & !1;
        harness.send(Command::VideoCrop(Some(preset))).unwrap();
        let changed = harness.wait(|state| {
            state
                .video_output
                .as_ref()
                .is_some_and(|video| video.w == output_width && video.h == output_height)
                && state.video_filters.as_ref().is_some_and(|filters| {
                    filters
                        .iter()
                        .filter(|filter| filter.label.as_deref() == Some(FILTER_LABEL))
                        .count()
                        == 1
                })
        });
        assert!(changed.paused);
        assert_eq!(changed.volume, 37.0);
        assert_eq!(changed.error, None);
        let crop = changed
            .video_filters
            .as_ref()
            .unwrap()
            .iter()
            .find(|filter| filter.label.as_deref() == Some(FILTER_LABEL))
            .unwrap();
        assert_eq!(crop.params.get("@0"), Some(&width.to_string()));
        assert_eq!(crop.params.get("@1"), Some(&height.to_string()));
        assert_eq!(
            changed.video.as_ref().map(|video| (video.w, video.h)),
            Some((320, 180))
        );
        assert_eq!(changed.completed_navigation_seeks, 0);
        assert_eq!(
            riina_core::crop::observed_index(&changed),
            Some(
                Preset::ALL
                    .iter()
                    .position(|value| *value == preset)
                    .map_or(6, |index| index + 1)
            )
        );
        assert_eq!(
            riina_core::crop::observed_menu_index(&changed),
            Preset::MENU_ALL
                .iter()
                .position(|value| *value == preset)
                .map(|index| index + 1)
        );
    }
    harness.send(Command::VideoCrop(None)).unwrap();
    let reset = harness.wait(|state| {
        state
            .video_output
            .as_ref()
            .is_some_and(|video| video.w == 320 && video.h == 180)
            && state.video_filters.as_ref().is_some_and(|filters| {
                !filters
                    .iter()
                    .any(|filter| filter.label.as_deref() == Some(FILTER_LABEL))
            })
    });
    assert!(reset.paused);
    assert_eq!(reset.volume, 37.0);
    assert_eq!(reset.error, None);
    // Display overrides must not become the coordinate system for later crop
    // requests: every quarter turn still crops the original 320×180 pixels.
    harness
        .send(Command::VideoAspect(
            riina_core::video_controls::AspectRatio::from_text("1:1"),
        ))
        .unwrap();
    harness.wait(|state| state.video_aspect_override == Some(1.0));
    for rotation in [90, 180, 270, 0] {
        harness
            .send(Command::VideoRotation(
                riina_core::video_controls::Rotation::from_degrees(rotation).unwrap(),
            ))
            .unwrap();
        harness.wait(|state| state.video_rotation == Some(rotation));
        for preset in Preset::ALL {
            let (width, height) = preset.dimensions(320, 180).unwrap();
            harness.send(Command::VideoCrop(Some(preset))).unwrap();
            let changed = harness.wait(|state| {
                state.video_filters.as_ref().is_some_and(|filters| {
                    filters.iter().any(|filter| {
                        filter.label.as_deref() == Some(FILTER_LABEL)
                            && filter.params.get("@0") == Some(&width.to_string())
                            && filter.params.get("@1") == Some(&height.to_string())
                    })
                })
            });
            assert_eq!(
                changed.video.as_ref().map(|video| (video.w, video.h)),
                Some((320, 180))
            );
            assert_eq!(changed.video_aspect_override, Some(1.0));
            assert_eq!(changed.video_rotation, Some(rotation));
            assert!(changed.paused);
            assert_eq!(changed.volume, 37.0);
            assert_eq!(changed.error, None);
        }
        harness.send(Command::VideoCrop(None)).unwrap();
        let cleared = harness.wait(|state| {
            state.video_filters.as_ref().is_some_and(|filters| {
                !filters
                    .iter()
                    .any(|filter| filter.label.as_deref() == Some(FILTER_LABEL))
            })
        });
        assert_eq!(cleared.video_aspect_override, Some(1.0));
        assert_eq!(cleared.video_rotation, Some(rotation));
        assert_eq!(cleared.error, None);
    }
    let rectangle = riina_core::crop::Rectangle {
        x: 10,
        y: 12,
        width: 300,
        height: 160,
    };
    harness
        .send(Command::VideoCropRectangle(rectangle))
        .unwrap();
    let custom = harness.wait(|state| {
        state
            .video_output
            .as_ref()
            .is_some_and(|video| video.w == 300 && video.h == 160)
            && state.video_filters.as_ref().is_some_and(|filters| {
                filters.iter().any(|filter| {
                    filter.label.as_deref() == Some(FILTER_LABEL)
                        && filter.params.get("@2").is_some_and(|x| x == "10")
                        && filter.params.get("@3").is_some_and(|y| y == "12")
                })
            })
    });
    assert!(custom.paused);
    assert_eq!(custom.volume, 37.0);
    assert_eq!(custom.video_aspect_override, Some(1.0));
    assert_eq!(custom.error, None);
    assert_eq!(riina_core::crop::observed_index(&custom), Some(6));
    harness
        .send(Command::VideoCropRectangle(riina_core::crop::Rectangle {
            x: 0,
            y: 0,
            width: 320,
            height: 180,
        }))
        .unwrap();
    harness.wait(|state| {
        state
            .video_output
            .as_ref()
            .is_some_and(|video| video.w == 320 && video.h == 180)
            && state.video_filters.as_ref().is_some_and(|filters| {
                !filters
                    .iter()
                    .any(|filter| filter.label.as_deref() == Some(FILTER_LABEL))
            })
    });
    harness.send(Command::VideoCrop(None)).unwrap();
    harness.send(Command::Volume(38.0)).unwrap();
    assert_eq!(harness.wait(|state| state.volume == 38.0).error, None);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies crop entry and exit restore playback state"]
fn interactive_crop_session_restores_pause_on_done_and_cancel() {
    use riina_core::{
        crop::{FILTER_LABEL, Preset, Rectangle},
        crop_session::Session,
    };
    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    harness
        .send(Command::Load(
            root.join("tests/fixtures/catalog.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    let mut state = harness.wait(|state| {
        state.phase == Phase::Playing && state.video.is_some() && state.video_filters.is_some()
    });
    harness.send(Command::Volume(37.0)).unwrap();
    for (was_paused, confirm) in [(false, false), (true, false), (false, true), (true, true)] {
        if state.paused != was_paused {
            harness.send(Command::TogglePause).unwrap();
            harness.wait(|state| state.paused == was_paused);
        }
        harness.send(Command::Speed(1.5)).unwrap();
        harness.wait(|state| state.speed == 1.5 && state.volume == 37.0);
        harness
            .send(Command::VideoCrop(Some(Preset::FourThree)))
            .unwrap();
        state = harness.wait(|state| {
            state.video_filters.as_ref().is_some_and(|filters| {
                filters
                    .iter()
                    .any(|filter| filter.label.as_deref() == Some(FILTER_LABEL))
            })
        });
        assert_eq!(state.paused, was_paused);
        let session = Session::begin(&state).unwrap();
        harness.send(Command::EnterCrop(session.clone())).unwrap();
        let editing = harness.wait(|state| session.ready(state));
        assert_eq!(editing.volume, 37.0);
        assert_eq!(editing.speed, 1.5);
        assert_eq!(editing.error, None);
        let rectangle = confirm.then_some(Rectangle {
            x: 10,
            y: 12,
            width: 300,
            height: 160,
        });
        harness
            .send(Command::ExitCrop { session, rectangle })
            .unwrap();
        // An unrelated changed option provides a barrier even when restoring
        // an already-paused source does not produce a new pause observation.
        harness.send(Command::Speed(1.25)).unwrap();
        state = harness.wait(|state| {
            state.speed == 1.25
                && state.paused == was_paused
                && state.video_filters.as_ref().is_some_and(|filters| {
                    filters
                        .iter()
                        .any(|filter| filter.label.as_deref() == Some(FILTER_LABEL))
                        == confirm
                })
        });
        assert_eq!(state.volume, 37.0);
        assert_eq!(state.error, None);
        if confirm {
            let filter = state
                .video_filters
                .as_ref()
                .unwrap()
                .iter()
                .find(|filter| filter.label.as_deref() == Some(FILTER_LABEL))
                .unwrap();
            assert_eq!(filter.params.get("@0").map(String::as_str), Some("300"));
            assert_eq!(filter.params.get("@2").map(String::as_str), Some("10"));
        }
    }
    let stale = Session::begin(&state).unwrap();
    let previous_generation = state.generation;
    harness
        .send(Command::Load(
            root.join("tests/fixtures/catalog.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    let replacement = harness.wait(|state| {
        state.generation > previous_generation
            && state.phase == Phase::Playing
            && !state.paused
            && state.video.is_some()
            && state.video_filters.is_some()
    });
    harness
        .send(Command::ExitCrop {
            session: stale,
            rectangle: Some(Rectangle {
                x: 0,
                y: 0,
                width: 240,
                height: 180,
            }),
        })
        .unwrap();
    let rejected =
        harness.wait(|state| state.error.as_deref() == Some("The video changed while cropping"));
    assert_eq!(rejected.generation, replacement.generation);
    assert!(!rejected.paused);
    assert_eq!(rejected.volume, 37.0);
    assert_eq!(rejected.video_filters, replacement.video_filters);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies crop mapping uses observed source display parameters"]
fn crop_projection_preserves_source_selection_under_native_display_overrides() {
    use riina_core::{
        crop::Rectangle,
        crop_geometry::{DisplayRect, Point, Projection},
        video_controls::{AspectRatio, Rotation},
    };
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
    harness.wait(|state| state.paused);
    let rectangle = Rectangle {
        x: 23,
        y: 17,
        width: 251,
        height: 137,
    };
    for (aspect, ratio) in [("Default", -1.0), ("1:1", 1.0), ("4:3", 4.0 / 3.0)] {
        harness
            .send(Command::VideoAspect(AspectRatio::from_text(aspect)))
            .unwrap();
        harness.wait(|state| state.video_aspect_override == Some(ratio));
        for degrees in [0, 90, 180, 270] {
            harness
                .send(Command::VideoRotation(
                    Rotation::from_degrees(degrees).unwrap(),
                ))
                .unwrap();
            let state = harness.wait(|state| {
                state
                    .video
                    .as_ref()
                    .is_some_and(|video| video.rotate == Some(degrees))
                    && state.video_rotation == Some(degrees)
            });
            let video = state.video.as_ref().unwrap();
            assert_eq!((video.w, video.h), (320, 180));
            let projection = Projection::for_video(
                video,
                DisplayRect {
                    x: 20.0,
                    y: 20.0,
                    width: 920.0,
                    height: 492.0,
                },
            )
            .unwrap();
            let content = projection.content();
            let (dw, dh) = (video.dw.unwrap() as f64, video.dh.unwrap() as f64);
            let expected = if degrees == 90 || degrees == 270 {
                dh / dw
            } else {
                dw / dh
            };
            assert!((content.width / content.height - expected).abs() < 1e-9);
            let screen = projection.project(rectangle).unwrap();
            assert_eq!(
                projection
                    .select(
                        Point {
                            x: screen.x,
                            y: screen.y
                        },
                        Point {
                            x: screen.x + screen.width,
                            y: screen.y + screen.height
                        }
                    )
                    .unwrap(),
                rectangle
            );
            assert!(state.paused);
            assert_eq!(state.error, None);
        }
    }
}
