//! Run explicitly: cargo test -p riina-mpv --test native --locked -- --ignored --nocapture
use riina_core::{Command, Phase, Playback, TrackKind};
use riina_mpv::Player;
use std::{
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

#[test]
#[ignore = "requires provisioned libmpv; verifies viewport option acceptance and playback isolation"]
fn picture_viewport_preserves_pause_volume_and_source_geometry() {
    use riina_core::{crop_geometry::Projection, video_viewport::Viewport};
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
        state.phase == Phase::Playing && state.video.is_some()
    });
    player.send(Command::TogglePause).unwrap();
    let paused = wait_for(&receiver, |state| state.paused);
    player.send(Command::Volume(37.0)).unwrap();
    wait_for(&receiver, |state| state.volume == 37.0);
    for (index, size) in [(960.0, 600.0), (520.0, 300.0)].into_iter().enumerate() {
        let picture = Projection::interactive(paused.video.as_ref().unwrap(), size)
            .unwrap()
            .content();
        player
            .send(Command::VideoViewport(Some(
                Viewport::new(size, picture).unwrap(),
            )))
            .unwrap();
        let speed = 1.25 + index as f64 * 0.25;
        // A distinct observed property follows all four queued margin setters.
        player.send(Command::Speed(speed)).unwrap();
        let state = wait_for(&receiver, |state| state.speed == speed);
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
        assert_eq!(state.error, None);
    }
    player.send(Command::VideoViewport(None)).unwrap();
    player.send(Command::Speed(1.0)).unwrap();
    let reset = wait_for(&receiver, |state| state.speed == 1.0);
    assert!(reset.paused);
    assert_eq!(reset.volume, 37.0);
    assert_eq!(reset.error, None);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies independent flip/mirror managed filters"]
fn video_reflection_axes_preserve_crop_and_playback() {
    use riina_core::{crop::Preset, video_reflection::Axis};
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
        state.phase == Phase::Playing && state.video_filters.is_some()
    });
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| state.paused);
    player.send(Command::Volume(37.0)).unwrap();
    wait_for(&receiver, |state| state.volume == 37.0);
    player
        .send(Command::VideoCrop(Some(Preset::FourThree)))
        .unwrap();
    let cropped = wait_for(&receiver, |state| {
        riina_core::crop::observed_index(state) == Some(1)
            && state.video_output.as_ref().map(|video| (video.w, video.h)) == Some((240, 180))
    });
    for (axis, enabled, horizontal, vertical) in [
        (Axis::Horizontal, true, true, false),
        (Axis::Vertical, true, true, true),
        (Axis::Horizontal, false, false, true),
        (Axis::Vertical, false, false, false),
    ] {
        player
            .send(Command::VideoReflection { axis, enabled })
            .unwrap();
        let state = wait_for(&receiver, |state| {
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
        assert_eq!(state.error, None);
        // Repeated explicit state is idempotent, including removal of absent labels.
        player
            .send(Command::VideoReflection { axis, enabled })
            .unwrap();
    }
    player.send(Command::Speed(1.25)).unwrap();
    let settled = wait_for(&receiver, |state| state.speed == 1.25);
    assert_eq!(settled.error, None);
    assert_eq!(
        settled
            .video_output
            .as_ref()
            .map(|video| (video.w, video.h)),
        Some((240, 180))
    );
    player
        .send(Command::ToggleVideoReflection(Axis::Horizontal))
        .unwrap();
    wait_for(&receiver, |state| {
        Axis::Horizontal.observed(state.video_filters.as_deref()) == Some(true)
    });
    player
        .send(Command::ToggleVideoReflection(Axis::Horizontal))
        .unwrap();
    wait_for(&receiver, |state| {
        Axis::Horizontal.observed(state.video_filters.as_deref()) == Some(false)
    });
    // Native chain ordering must survive a burst without UI observation.
    for axis in [
        Axis::Horizontal,
        Axis::Vertical,
        Axis::Horizontal,
        Axis::Vertical,
    ] {
        player.send(Command::ToggleVideoReflection(axis)).unwrap();
    }
    player.send(Command::Speed(1.5)).unwrap();
    let toggled = wait_for(&receiver, |state| state.speed == 1.5);
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
    assert_eq!(toggled.error, None);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies zero-size crop behavior with explicit offsets"]
fn zero_size_crop_offsets_preserve_requested_parameters() {
    use riina_core::crop::{FILTER_LABEL, Rectangle};
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
        state.phase == Phase::Playing && state.video_output.is_some()
    });
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| state.paused);
    player.send(Command::Volume(37.0)).unwrap();
    let baseline = wait_for(&receiver, |state| state.volume == 37.0);
    let mut completed = baseline.completed_seeks;
    for (width, height) in [(0, 160), (300, 0), (0, 0)] {
        player
            .send(Command::VideoCropRectangle(Rectangle {
                x: 10,
                y: 12,
                width,
                height,
            }))
            .unwrap();
        let state = wait_for(&receiver, |state| {
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
    player.send(Command::VideoCrop(None)).unwrap();
    let reset = wait_for(&receiver, |state| {
        riina_core::crop::observed_index(state) == Some(0)
    });
    assert_eq!(reset.error, None);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies explicit zero-size crop native semantics"]
fn zero_size_crop_uses_native_full_source_dimensions() {
    use riina_core::crop::{FILTER_LABEL, Rectangle};
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
        state.phase == Phase::Playing && state.video_output.is_some()
    });
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| state.paused);
    player
        .send(Command::VideoCropRectangle(Rectangle {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        }))
        .unwrap();
    let state = wait_for(&receiver, |state| {
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
    player.send(Command::VideoCrop(None)).unwrap();
    let reset = wait_for(&receiver, |state| {
        riina_core::crop::observed_index(state) == Some(0)
    });
    assert_eq!(reset.error, None);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies managed crop replacement and source geometry"]
fn crop_presets_replace_the_managed_crop_and_restore_source_output() {
    use riina_core::crop::{FILTER_LABEL, Preset};
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
        state.phase == Phase::Playing
            && state.video_filters.is_some()
            && state.video_output.is_some()
    });
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| state.paused);
    player.send(Command::Volume(37.0)).unwrap();
    wait_for(&receiver, |state| state.volume == 37.0);
    for preset in Preset::MENU_ALL.into_iter().chain([Preset::TwentyOneNine]) {
        let (width, height) = preset.dimensions(320, 180).unwrap();
        // This fixture decodes to YUV420: libavfilter aligns odd dimensions
        // down to chroma boundaries, while vf retains the requested dimensions.
        let output_width = width & !1;
        let output_height = height & !1;
        player.send(Command::VideoCrop(Some(preset))).unwrap();
        let changed = wait_for(&receiver, |state| {
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
    player.send(Command::VideoCrop(None)).unwrap();
    let reset = wait_for(&receiver, |state| {
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
    player
        .send(Command::VideoAspect(
            riina_core::video_controls::AspectRatio::from_text("1:1"),
        ))
        .unwrap();
    wait_for(&receiver, |state| state.video_aspect_override == Some(1.0));
    for rotation in [90, 180, 270, 0] {
        player
            .send(Command::VideoRotation(
                riina_core::video_controls::Rotation::from_degrees(rotation).unwrap(),
            ))
            .unwrap();
        wait_for(&receiver, |state| state.video_rotation == Some(rotation));
        for preset in Preset::ALL {
            let (width, height) = preset.dimensions(320, 180).unwrap();
            player.send(Command::VideoCrop(Some(preset))).unwrap();
            let changed = wait_for(&receiver, |state| {
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
        player.send(Command::VideoCrop(None)).unwrap();
        let cleared = wait_for(&receiver, |state| {
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
    player.send(Command::VideoCropRectangle(rectangle)).unwrap();
    let custom = wait_for(&receiver, |state| {
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
    player
        .send(Command::VideoCropRectangle(riina_core::crop::Rectangle {
            x: 0,
            y: 0,
            width: 320,
            height: 180,
        }))
        .unwrap();
    wait_for(&receiver, |state| {
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
    player.send(Command::VideoCrop(None)).unwrap();
    player.send(Command::Volume(38.0)).unwrap();
    assert_eq!(
        wait_for(&receiver, |state| state.volume == 38.0).error,
        None
    );
}

#[test]
#[ignore = "requires provisioned libmpv; verifies crop entry and exit restore playback state"]
fn interactive_crop_session_restores_pause_on_done_and_cancel() {
    use riina_core::{
        crop::{FILTER_LABEL, Preset, Rectangle},
        crop_session::Session,
    };
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
    let mut state = wait_for(&receiver, |state| {
        state.phase == Phase::Playing && state.video.is_some() && state.video_filters.is_some()
    });
    player.send(Command::Volume(37.0)).unwrap();
    for (was_paused, confirm) in [(false, false), (true, false), (false, true), (true, true)] {
        if state.paused != was_paused {
            player.send(Command::TogglePause).unwrap();
            wait_for(&receiver, |state| state.paused == was_paused);
        }
        player.send(Command::Speed(1.5)).unwrap();
        wait_for(&receiver, |state| {
            state.speed == 1.5 && state.volume == 37.0
        });
        player
            .send(Command::VideoCrop(Some(Preset::FourThree)))
            .unwrap();
        state = wait_for(&receiver, |state| {
            state.video_filters.as_ref().is_some_and(|filters| {
                filters
                    .iter()
                    .any(|filter| filter.label.as_deref() == Some(FILTER_LABEL))
            })
        });
        assert_eq!(state.paused, was_paused);
        let session = Session::begin(&state).unwrap();
        player.send(Command::EnterCrop(session.clone())).unwrap();
        let editing = wait_for(&receiver, |state| session.ready(state));
        assert_eq!(editing.volume, 37.0);
        assert_eq!(editing.speed, 1.5);
        assert_eq!(editing.error, None);
        let rectangle = confirm.then_some(Rectangle {
            x: 10,
            y: 12,
            width: 300,
            height: 160,
        });
        player
            .send(Command::ExitCrop { session, rectangle })
            .unwrap();
        // An unrelated changed option provides a barrier even when restoring
        // an already-paused source does not produce a new pause observation.
        player.send(Command::Speed(1.25)).unwrap();
        state = wait_for(&receiver, |state| {
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
    player
        .send(Command::Load(
            root.join("tests/fixtures/catalog.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    let replacement = wait_for(&receiver, |state| {
        state.generation > previous_generation
            && state.phase == Phase::Playing
            && !state.paused
            && state.video.is_some()
            && state.video_filters.is_some()
    });
    player
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
    let rejected = wait_for(&receiver, |state| {
        state.error.as_deref() == Some("The video changed while cropping")
    });
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
        state.phase == Phase::Playing && state.video.is_some()
    });
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| state.paused);
    let rectangle = Rectangle {
        x: 23,
        y: 17,
        width: 251,
        height: 137,
    };
    for (aspect, ratio) in [("Default", -1.0), ("1:1", 1.0), ("4:3", 4.0 / 3.0)] {
        player
            .send(Command::VideoAspect(AspectRatio::from_text(aspect)))
            .unwrap();
        wait_for(&receiver, |state| {
            state.video_aspect_override == Some(ratio)
        });
        for degrees in [0, 90, 180, 270] {
            player
                .send(Command::VideoRotation(
                    Rotation::from_degrees(degrees).unwrap(),
                ))
                .unwrap();
            let state = wait_for(&receiver, |state| {
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
#[ignore = "requires provisioned libmpv; verifies selected last-cue navigation before an automatic loop"]
fn last_cue_navigation_does_not_label_a_later_automatic_loop() {
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
    wait_for(&receiver, |state| state.phase == Phase::Playing);
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| state.paused);
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
            .any(|track| track.kind == TrackKind::Sub && track.is_primary_selected())
    });
    player
        .send(Command::Seek {
            seconds: 3.0,
            absolute: true,
        })
        .unwrap();
    wait_for(&receiver, |state| {
        !state.seeking
            && state
                .position
                .is_some_and(|clock| (clock - 3.0).abs() < 0.05)
            && state
                .subtitle_text
                .as_deref()
                .is_some_and(|text| text.contains("Chapter two"))
    });
    player
        .send(Command::SetAbLoop {
            a: Some(2.8),
            b: Some(3.2),
        })
        .unwrap();
    let before = wait_for(&receiver, |state| {
        state.ab_loop_a == Some(2.8) && state.ab_loop_b == Some(3.2)
    });
    player.send(Command::SubtitleSeek(1)).unwrap();
    player.send(Command::Volume(37.0)).unwrap();
    let baseline = wait_for(&receiver, |state| state.volume == 37.0);
    assert_eq!(
        baseline.completed_navigation_seeks, before.completed_navigation_seeks,
        "Past-last-cue request unexpectedly moved playback"
    );
    player.send(Command::TogglePause).unwrap();
    let looped = wait_for(&receiver, |state| {
        state.completed_seeks > baseline.completed_seeks && !state.seeking
    });
    assert_eq!(
        looped.completed_navigation_seeks,
        baseline.completed_navigation_seeks
    );
    assert_eq!(looped.volume, 37.0);
    assert!(looped.error.is_none(), "{:?}", looped.error);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies no-cue navigation does not label an automatic loop"]
fn no_cue_navigation_does_not_arm_automatic_loop_feedback() {
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
    wait_for(&receiver, |state| state.phase == Phase::Playing);
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| state.paused);
    player
        .send(Command::Seek {
            seconds: 0.3,
            absolute: true,
        })
        .unwrap();
    wait_for(&receiver, |state| {
        state.completed_navigation_seeks > 0 && !state.seeking
    });
    player
        .send(Command::SetAbLoop {
            a: Some(0.2),
            b: Some(0.6),
        })
        .unwrap();
    wait_for(&receiver, |state| {
        state.ab_loop_a == Some(0.2) && state.ab_loop_b == Some(0.6)
    });
    player.send(Command::SubtitleSeek(1)).unwrap();
    player.send(Command::Volume(37.0)).unwrap();
    let baseline = wait_for(&receiver, |state| state.volume == 37.0);
    player.send(Command::TogglePause).unwrap();
    let looped = wait_for(&receiver, |state| {
        state.completed_seeks > baseline.completed_seeks && !state.seeking
    });
    assert_eq!(
        looped.completed_navigation_seeks,
        baseline.completed_navigation_seeks
    );
    assert_eq!(looped.volume, 37.0);
    assert!(looped.error.is_none(), "{:?}", looped.error);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies navigation feedback provenance"]
fn navigation_completions_follow_explicit_seek_and_same_position_reseek() {
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
    let loaded = wait_for(&receiver, |state| state.phase == Phase::Playing);
    assert_eq!(loaded.completed_navigation_seeks, 0);
    player.send(Command::TogglePause).unwrap();
    let mut previous = wait_for(&receiver, |state| state.paused);
    for position in [2.0, 2.0, 0.5] {
        player
            .send(Command::Seek {
                seconds: position,
                absolute: true,
            })
            .unwrap();
        let completed = wait_for(&receiver, |state| {
            state.completed_navigation_seeks > previous.completed_navigation_seeks
                && !state.seeking
                && state
                    .position
                    .is_some_and(|clock| (clock - position).abs() < 0.1)
        });
        assert!(completed.completed_seeks > previous.completed_seeks);
        assert!(completed.paused);
        assert!(completed.error.is_none(), "{:?}", completed.error);
        previous = completed;
    }
}

#[test]
#[ignore = "requires provisioned libmpv; verifies native rejection of an above-limit custom speed"]
fn excessive_custom_speed_reports_native_error_without_changing_playback() {
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
    wait_for(&receiver, |state| state.phase == Phase::Playing);
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| state.paused);
    player.send(Command::Volume(37.0)).unwrap();
    wait_for(&receiver, |state| state.volume == 37.0);
    player.send(Command::Speed(1.25)).unwrap();
    wait_for(&receiver, |state| state.speed == 1.25);
    let request = riina_core::speed_controls::from_text("101").unwrap();
    player.send(request).unwrap();
    let rejected = wait_for(&receiver, |state| state.error.is_some());
    assert_eq!(rejected.speed, 1.25);
    assert!(rejected.paused);
    assert_eq!(rejected.volume, 37.0);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies speed slider ticks and precise custom entries"]
fn speed_controls_preserve_pause_volume_and_custom_precision() {
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
    wait_for(&receiver, |state| state.phase == Phase::Playing);
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| state.paused);
    player.send(Command::Volume(37.0)).unwrap();
    wait_for(&receiver, |state| state.volume == 37.0);
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
        player.send(command).unwrap();
        let state = wait_for(&receiver, |state| {
            (state.speed - expected).abs() < 0.0000001
        });
        assert!(state.paused);
        assert_eq!(state.volume, 37.0);
        assert!(state.error.is_none(), "{:?}", state.error);
    }
}

#[test]
#[ignore = "requires provisioned libmpv; verifies independent video equalizer channels and resets"]
fn video_equalizer_preserves_other_channels_and_playback_options() {
    use riina_core::video_controls::Equalizer;
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
        state.phase == Phase::Playing && state.video_equalizer == [Some(0); 5]
    });
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| state.paused);
    player.send(Command::Volume(37.0)).unwrap();
    wait_for(&receiver, |state| state.volume == 37.0);
    player.send(Command::Speed(1.5)).unwrap();
    wait_for(&receiver, |state| state.speed == 1.5);
    let mut expected = [Some(0); 5];
    for (option, value) in Equalizer::ALL.into_iter().zip([-100, 100, -27, 13, 61]) {
        expected[option.index()] = Some(value);
        player
            .send(Command::VideoEqualizer { option, value })
            .unwrap();
        let state = wait_for(&receiver, |state| state.video_equalizer == expected);
        assert!(state.paused);
        assert_eq!(state.volume, 37.0);
        assert_eq!(state.speed, 1.5);
        assert!(state.error.is_none(), "{:?}", state.error);
    }
    for option in Equalizer::ALL {
        expected[option.index()] = Some(0);
        player
            .send(Command::VideoEqualizer { option, value: 0 })
            .unwrap();
        let state = wait_for(&receiver, |state| state.video_equalizer == expected);
        assert!(state.paused);
        assert_eq!(state.volume, 37.0);
        assert_eq!(state.speed, 1.5);
        assert!(state.error.is_none(), "{:?}", state.error);
    }
}

#[test]
#[ignore = "requires provisioned libmpv; verifies configured decoding independently of codec fallback"]
fn hardware_decoding_policy_survives_fallback_and_file_replacement() {
    use riina_core::video_controls::HardwareDecoder;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let (sender, receiver) = mpsc::channel();
    let player = Player::start(&test_library(&root), true, move |state| {
        let _ = sender.send(state);
    })
    .unwrap();
    let fixture = root
        .join("tests/fixtures/catalog.mkv")
        .to_string_lossy()
        .into_owned();
    player.send(Command::Load(fixture.clone())).unwrap();
    wait_for(&receiver, |state| state.phase == Phase::Playing);
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| state.paused);
    player.send(Command::Volume(37.0)).unwrap();
    wait_for(&receiver, |state| state.volume == 37.0);
    for mode in [
        HardwareDecoder::Disabled,
        HardwareDecoder::AutoCopy,
        HardwareDecoder::Auto,
    ] {
        player.send(Command::HardwareDecoding(mode)).unwrap();
        let state = wait_for(&receiver, |state| {
            state.hardware_decoding.as_deref() == Some(mode.native_value())
        });
        assert!(state.paused);
        assert_eq!(state.volume, 37.0);
        assert!(state.error.is_none(), "{:?}", state.error);
        // Headless MJPEG has no hardware presentation. The configured switch
        // must still be on for auto modes instead of following fallback state.
        assert!(matches!(
            state.hardware_decoder.as_deref(),
            None | Some("no")
        ));
    }
    player
        .send(Command::HardwareDecoding(HardwareDecoder::AutoCopy))
        .unwrap();
    let previous = wait_for(&receiver, |state| {
        state.hardware_decoding.as_deref() == Some("auto-copy")
    });
    player.send(Command::Load(fixture)).unwrap();
    let replaced = wait_for(&receiver, |state| {
        state.generation > previous.generation && state.phase == Phase::Playing
    });
    assert_eq!(replaced.hardware_decoding.as_deref(), Some("auto-copy"));
    // Explicit replacement resumes playback by contract; the decoding policy
    // must survive that transition rather than resetting to startup defaults.
    assert_eq!(replaced.volume, 37.0);
    assert!(replaced.error.is_none(), "{:?}", replaced.error);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies Unicode ratio text is not silently normalized"]
fn unicode_aspect_input_preserves_text_and_native_failure_state() {
    use riina_core::video_controls::AspectRatio;
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
    wait_for(&receiver, |state| state.phase == Phase::Playing);
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| state.paused);
    player.send(Command::Volume(37.0)).unwrap();
    wait_for(&receiver, |state| state.volume == 37.0);
    player
        .send(Command::VideoAspect(AspectRatio::from_text("4:3")))
        .unwrap();
    wait_for(&receiver, |state| {
        state.video_aspect_override == Some(4.0 / 3.0)
    });
    player
        .send(Command::VideoAspect(AspectRatio::from_text("١٦:٩")))
        .unwrap();
    let rejected = wait_for(&receiver, |state| state.error.is_some());
    assert_eq!(rejected.video_aspect_override, Some(4.0 / 3.0));
    assert!(rejected.paused);
    assert_eq!(rejected.volume, 37.0);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies video aspect and rotation while paused"]
fn video_geometry_controls_preserve_pause_and_unrelated_options() {
    use riina_core::video_controls::{ASPECT_PRESETS, AspectRatio, Rotation};
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
    for ratio in ASPECT_PRESETS.into_iter().skip(1).chain(["2.39:1", "8:6"]) {
        let (width, height) = ratio.split_once(':').unwrap();
        let expected = width.parse::<f64>().unwrap() / height.parse::<f64>().unwrap();
        player
            .send(Command::VideoAspect(AspectRatio::from_text(ratio)))
            .unwrap();
        let changed = wait_for(&receiver, |state| {
            state
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
        assert_eq!(changed.error, None);
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
    wait_for(&receiver, |state| state.video_aspect_override == Some(-1.0));
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
        assert_eq!(changed.error, None);
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
#[ignore = "requires provisioned libmpv; verifies real ISO and BCP 47 track metadata"]
fn multilingual_tracks_preserve_language_metadata_across_subtitle_lanes() {
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
            root.join("tests/fixtures/track-languages.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    let loaded = wait_for(&receiver, |state| {
        state.phase == Phase::Playing
            && state
                .tracks
                .iter()
                .filter(|t| t.kind == TrackKind::Sub)
                .count()
                == 7
    });
    for (title, language) in [
        ("English captions", Some("eng")),
        ("German captions", Some("ger")),
        ("French captions", Some("fre")),
        ("Traditional Chinese captions", Some("zh-Hant")),
        ("Brazilian Portuguese captions", Some("pt-BR")),
        // mpv omits und instead of supplying an explicit language property.
        ("Unspecified captions", None),
        ("Unknown captions", Some("qzz")),
    ] {
        let track = loaded
            .tracks
            .iter()
            .find(|track| track.title.as_deref() == Some(title))
            .unwrap();
        assert_eq!(track.lang.as_deref(), language, "{title}");
    }
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| state.paused);
    let id = |title: &str| {
        loaded
            .tracks
            .iter()
            .find(|track| track.title.as_deref() == Some(title))
            .unwrap()
            .id
    };
    let primary = id("German captions");
    let secondary = id("Traditional Chinese captions");
    player
        .send(Command::SelectTrack {
            kind: TrackKind::Sub,
            id: Some(primary),
        })
        .unwrap();
    wait_for(&receiver, |state| {
        selected_track(state, TrackKind::Sub) == Some(primary)
    });
    player
        .send(Command::SelectSecondarySubtitle(Some(secondary)))
        .unwrap();
    let both = wait_for(&receiver, |state| {
        state
            .tracks
            .iter()
            .any(|track| track.id == secondary && track.selected && track.main_selection == Some(1))
    });
    assert_eq!(selected_track(&both, TrackKind::Sub), Some(primary));
    assert!(both.paused);
    assert_eq!(both.error, None);
    for track in &loaded.tracks {
        // mpv numbers each track kind independently; ID alone is not identity.
        let observed = both
            .tracks
            .iter()
            .find(|t| t.kind == track.kind && t.id == track.id)
            .unwrap();
        assert_eq!(observed.lang, track.lang);
        assert_eq!(observed.title, track.title);
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
        .find(|t| t.kind == kind && t.is_primary_selected())
        .map(|t| t.id)
}

#[test]
#[ignore = "requires provisioned libmpv; verifies independent subtitle color channels and defaults"]
fn subtitle_colors_preserve_alpha_and_other_style_options() {
    use riina_core::subtitle_color::SubtitleColor;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let (sender, receiver) = mpsc::channel();
    let player = Player::start(&test_library(&root), true, move |state| {
        let _ = sender.send(state);
    })
    .unwrap();
    let defaults = wait_for(&receiver, |state| {
        state.subtitle_text_color == Some(SubtitleColor::WHITE)
            && state.subtitle_border_color == Some(SubtitleColor::BLACK)
            && state.subtitle_shadow_color == Some(SubtitleColor::CLEAR)
            && state.subtitle_font_size == Some(55.0)
            && state.subtitle_outline_size == Some(3.0)
    });
    assert_eq!(defaults.error, None);
    player
        .send(Command::Load(
            root.join("tests/fixtures/catalog.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    wait_for(&receiver, |state| state.phase == Phase::Playing);
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| state.paused);
    player
        .send(Command::AddSubtitle(
            root.join("tests/fixtures/manual-captions.srt")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    let selected = wait_for(&receiver, |state| {
        selected_track(state, TrackKind::Sub).is_some()
    });
    let subtitle_id = selected_track(&selected, TrackKind::Sub);
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
            .is_some_and(|text| text.contains("café"))
    });
    let text = SubtitleColor::from_mpv("#80402010").unwrap();
    let border = SubtitleColor::from_mpv("#C0102040").unwrap();
    let shadow = SubtitleColor::from_mpv("#40000000").unwrap();
    player.send(Command::SubtitleTextColor(text)).unwrap();
    let colored = wait_for(&receiver, |state| state.subtitle_text_color == Some(text));
    assert_eq!(colored.subtitle_border_color, Some(SubtitleColor::BLACK));
    assert_eq!(colored.subtitle_shadow_color, Some(SubtitleColor::CLEAR));
    player.send(Command::SubtitleBorderColor(border)).unwrap();
    let outlined = wait_for(&receiver, |state| {
        state.subtitle_border_color == Some(border)
    });
    assert_eq!(outlined.subtitle_text_color, Some(text));
    assert_eq!(outlined.subtitle_shadow_color, Some(SubtitleColor::CLEAR));
    player.send(Command::SubtitleShadowColor(shadow)).unwrap();
    let shaded = wait_for(&receiver, |state| {
        state.subtitle_shadow_color == Some(shadow)
    });
    assert_eq!(shaded.subtitle_text_color, Some(text));
    assert_eq!(shaded.subtitle_border_color, Some(border));
    assert_eq!(shaded.subtitle_font_size, Some(55.0));
    assert_eq!(shaded.subtitle_outline_size, Some(3.0));
    assert!(shaded.paused);
    assert_eq!(selected_track(&shaded, TrackKind::Sub), subtitle_id);
    assert_eq!(shaded.subtitle_delay, 0.0);
    assert_eq!(shaded.error, None);
    for command in [
        Command::SubtitleTextColor(SubtitleColor::WHITE),
        Command::SubtitleBorderColor(SubtitleColor::BLACK),
        Command::SubtitleShadowColor(SubtitleColor::CLEAR),
    ] {
        player.send(command).unwrap();
    }
    let restored = wait_for(&receiver, |state| {
        state.subtitle_text_color == Some(SubtitleColor::WHITE)
            && state.subtitle_border_color == Some(SubtitleColor::BLACK)
            && state.subtitle_shadow_color == Some(SubtitleColor::CLEAR)
    });
    assert!(restored.paused);
    assert_eq!(selected_track(&restored, TrackKind::Sub), subtitle_id);
    assert_eq!(restored.error, None);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies configured subtitle font names and independent style state"]
fn subtitle_font_selection_preserves_other_style_options() {
    use riina_core::subtitle_controls::{DEFAULT_SUBTITLE_FONT, font_from_text};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let (sender, receiver) = mpsc::channel();
    let player = Player::start(&test_library(&root), true, move |state| {
        let _ = sender.send(state);
    })
    .unwrap();
    wait_for(&receiver, |state| {
        state.subtitle_font.as_deref() == Some(DEFAULT_SUBTITLE_FONT)
    });
    player
        .send(Command::Load(
            root.join("tests/fixtures/catalog.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    wait_for(&receiver, |state| state.phase == Phase::Playing);
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| state.paused);
    player
        .send(Command::AddSubtitle(
            root.join("tests/fixtures/manual-captions.srt")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    let selected = wait_for(&receiver, |state| {
        selected_track(state, TrackKind::Sub).is_some()
    });
    let subtitle_id = selected_track(&selected, TrackKind::Sub);
    player
        .send(Command::Seek {
            seconds: 0.5,
            absolute: true,
        })
        .unwrap();
    let before = wait_for(&receiver, |state| {
        state
            .subtitle_text
            .as_deref()
            .is_some_and(|text| text.contains("café"))
    });
    // The custom entry is an engine preference, not proof a matching font is
    // installed. Missing families must remain eligible for libass fallback.
    for name in ["serif", "monospace", "Riina 字体 Привет", ""] {
        player.send(font_from_text(name).unwrap()).unwrap();
        let expected = if name.is_empty() {
            DEFAULT_SUBTITLE_FONT
        } else {
            name
        };
        let changed = wait_for(&receiver, |state| {
            state.subtitle_font.as_deref() == Some(expected)
        });
        assert!(changed.paused);
        assert_eq!(selected_track(&changed, TrackKind::Sub), subtitle_id);
        assert_eq!(changed.subtitle_font_size, before.subtitle_font_size);
        assert_eq!(changed.subtitle_outline_size, before.subtitle_outline_size);
        assert_eq!(changed.subtitle_text_color, before.subtitle_text_color);
        assert_eq!(changed.subtitle_border_color, before.subtitle_border_color);
        assert_eq!(changed.subtitle_shadow_color, before.subtitle_shadow_color);
        assert_eq!(changed.subtitle_delay, before.subtitle_delay);
        assert_eq!(changed.error, None);
    }
}

#[test]
#[ignore = "requires provisioned libmpv; verifies ten-minute relative seeks away from endpoints"]
fn long_relative_seeks_preserve_pause_and_volume() {
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
            root.join("tests/fixtures/long-silence.wav")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    wait_for(&receiver, |state| {
        state.phase == Phase::Playing
            && state
                .duration
                .is_some_and(|value| (value - 1300.0).abs() < 0.01)
    });
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| state.paused);
    player.send(Command::Volume(37.0)).unwrap();
    wait_for(&receiver, |state| (state.volume - 37.0).abs() < 0.01);
    player
        .send(Command::Seek {
            seconds: 100.0,
            absolute: true,
        })
        .unwrap();
    wait_for(&receiver, |state| {
        state
            .position
            .is_some_and(|value| (value - 100.0).abs() < 0.1)
    });
    // Stay away from boundaries: clamping could hide an incorrect seek amount.
    for (seconds, expected) in [(600.0, 700.0), (-600.0, 100.0)] {
        player
            .send(Command::Seek {
                seconds,
                absolute: false,
            })
            .unwrap();
        let sought = wait_for(&receiver, |state| {
            state
                .position
                .is_some_and(|value| (value - expected).abs() < 0.1)
        });
        assert!(sought.paused);
        assert!((sought.volume - 37.0).abs() < 0.01);
        assert_eq!(sought.error, None);
    }
}

#[test]
#[ignore = "requires provisioned libmpv; verifies selected ASS warning across subtitle lanes"]
fn subtitle_style_warning_follows_selected_ass_lanes() {
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
    wait_for(&receiver, |state| state.phase == Phase::Playing);
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| state.paused);
    player
        .send(Command::AddSubtitle(fixture("styled-captions.ass")))
        .unwrap();
    let styled = wait_for(&receiver, |state| {
        state.subtitle_style_availability().ass_warning
    });
    let ass_id = selected_track(&styled, TrackKind::Sub).unwrap();
    assert!(styled.subtitle_style_availability().text_controls);
    player
        .send(Command::Seek {
            seconds: 0.5,
            absolute: true,
        })
        .unwrap();
    let decoded = wait_for(&receiver, |state| {
        state
            .subtitle_text
            .as_deref()
            .is_some_and(|text| text.contains("Styled first cue"))
    });
    assert!(decoded.paused);
    assert_eq!(decoded.error, None);
    player
        .send(Command::AddSubtitle(fixture("manual-captions.srt")))
        .unwrap();
    let plain = wait_for(&receiver, |state| {
        selected_track(state, TrackKind::Sub).is_some_and(|id| id != ass_id)
    });
    let plain_id = selected_track(&plain, TrackKind::Sub).unwrap();
    assert!(!plain.subtitle_style_availability().ass_warning);
    player
        .send(Command::SelectSecondarySubtitle(Some(ass_id)))
        .unwrap();
    let mixed = wait_for(&receiver, |state| {
        state.subtitle_style_availability().ass_warning
    });
    assert_eq!(selected_track(&mixed, TrackKind::Sub), Some(plain_id));
    assert!(mixed.subtitle_style_availability().text_controls);
    assert!(mixed.paused);
    assert_eq!(mixed.error, None);
    player
        .send(Command::SelectTrack {
            kind: TrackKind::Sub,
            id: None,
        })
        .unwrap();
    let secondary_only = wait_for(&receiver, |state| {
        selected_track(state, TrackKind::Sub).is_none()
    });
    assert!(secondary_only.subtitle_style_availability().ass_warning);
    player.send(Command::SelectSecondarySubtitle(None)).unwrap();
    let disabled = wait_for(&receiver, |state| {
        !state.subtitle_style_availability().ass_warning
    });
    assert!(disabled.subtitle_style_availability().text_controls);
    assert!(disabled.paused);
    assert_eq!(disabled.error, None);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies independent primary and secondary subtitle lanes"]
fn secondary_subtitles_keep_selection_and_visibility_independent() {
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
    wait_for(&receiver, |state| state.phase == Phase::Playing);
    player.send(Command::TogglePause).unwrap();
    wait_for(&receiver, |state| state.paused);
    player
        .send(Command::AddSubtitle(fixture("manual-captions.srt")))
        .unwrap();
    let first = wait_for(&receiver, |state| {
        selected_track(state, TrackKind::Sub).is_some()
    });
    let primary_id = selected_track(&first, TrackKind::Sub).unwrap();
    player
        .send(Command::AddSubtitle(fixture("secondary-captions.srt")))
        .unwrap();
    let second = wait_for(&receiver, |state| {
        selected_track(state, TrackKind::Sub).is_some_and(|id| id != primary_id)
    });
    let secondary_id = selected_track(&second, TrackKind::Sub).unwrap();
    player
        .send(Command::SelectTrack {
            kind: TrackKind::Sub,
            id: Some(primary_id),
        })
        .unwrap();
    player
        .send(Command::SelectSecondarySubtitle(Some(secondary_id)))
        .unwrap();
    player
        .send(Command::Seek {
            seconds: 0.5,
            absolute: true,
        })
        .unwrap();
    let both = wait_for(&receiver, |state| {
        state.paused
            && !state.seeking
            && state
                .subtitle_text
                .as_deref()
                .is_some_and(|s| s.contains("Привет"))
            && state
                .secondary_subtitle_text
                .as_deref()
                .is_some_and(|s| s.contains("bonjour"))
            && state.tracks.iter().any(|t| {
                t.id == secondary_id
                    && t.kind == TrackKind::Sub
                    && t.selected
                    && t.main_selection == Some(1)
            })
    });
    assert_eq!(selected_track(&both, TrackKind::Sub), Some(primary_id));
    assert!(both.subtitle_visible);
    assert_eq!(both.secondary_subtitle_visible, Some(true));
    player.send(Command::SecondarySubtitleDelay(1.0)).unwrap();
    wait_for(&receiver, |state| {
        state.secondary_subtitle_delay == Some(1.0)
    });
    player
        .send(Command::Seek {
            seconds: 2.3,
            absolute: true,
        })
        .unwrap();
    let shifted = wait_for(&receiver, |state| {
        state.paused
            && !state.seeking
            && state.position.is_some_and(|p| (p - 2.3).abs() < 0.05)
            && state
                .subtitle_text
                .as_deref()
                .is_some_and(|s| s.contains("Chapter two"))
            && state
                .secondary_subtitle_text
                .as_deref()
                .is_some_and(|s| s.contains("bonjour"))
    });
    assert_eq!(shifted.subtitle_delay, both.subtitle_delay);
    assert_eq!(selected_track(&shifted, TrackKind::Sub), Some(primary_id));
    assert_eq!(shifted.error, None);
    for position in [25.0, 0.0, 150.0, 100.0] {
        player
            .send(Command::SecondarySubtitlePosition(position))
            .unwrap();
        let positioned = wait_for(&receiver, |state| {
            state.secondary_subtitle_position == Some(position)
        });
        assert!(positioned.paused);
        assert_eq!(positioned.subtitle_position, both.subtitle_position);
        assert_eq!(positioned.subtitle_delay, both.subtitle_delay);
        assert_eq!(positioned.secondary_subtitle_delay, Some(1.0));
        assert_eq!(
            selected_track(&positioned, TrackKind::Sub),
            Some(primary_id)
        );
        assert_eq!(positioned.error, None);
    }
    // Exercise the opposite lane after secondary edits to catch target aliasing.
    for position in [40.0, 0.0, 150.0, 100.0] {
        player.send(Command::SubtitlePosition(position)).unwrap();
        let positioned = wait_for(&receiver, |state| state.subtitle_position == Some(position));
        assert!(positioned.paused);
        assert_eq!(positioned.secondary_subtitle_position, Some(100.0));
        assert_eq!(positioned.subtitle_delay, both.subtitle_delay);
        assert_eq!(positioned.secondary_subtitle_delay, Some(1.0));
        assert_eq!(
            selected_track(&positioned, TrackKind::Sub),
            Some(primary_id)
        );
        assert!(positioned.tracks.iter().any(|track| {
            track.id == secondary_id
                && track.kind == TrackKind::Sub
                && track.selected
                && track.main_selection == Some(1)
        }));
        assert_eq!(positioned.error, None);
    }
    // Numeric entry can exceed the visual slider's range in either lane.
    player.send(Command::SubtitleDelay(6.125)).unwrap();
    wait_for(&receiver, |state| state.subtitle_delay == 6.125);
    player
        .send(Command::SecondarySubtitleDelay(-7.375))
        .unwrap();
    let wide_delays = wait_for(&receiver, |state| {
        state.secondary_subtitle_delay == Some(-7.375)
    });
    assert_eq!(wide_delays.subtitle_delay, 6.125);
    assert!(wide_delays.paused);
    assert_eq!(
        selected_track(&wide_delays, TrackKind::Sub),
        Some(primary_id)
    );
    assert_eq!(wide_delays.error, None);
    player
        .send(Command::SubtitleDelay(both.subtitle_delay))
        .unwrap();
    let before_seek = wait_for(&receiver, |state| {
        state.subtitle_delay == both.subtitle_delay
    });
    player.send(Command::SecondarySubtitleDelay(0.0)).unwrap();
    player
        .send(Command::Seek {
            seconds: 0.5,
            absolute: true,
        })
        .unwrap();
    wait_for(&receiver, |state| {
        // The requested position can already equal the previous clock. Wait
        // for this seek's completion before treating cached cue text as current;
        // decoder refresh can otherwise clear it while visibility is toggled.
        state.completed_seeks > before_seek.completed_seeks
            && !state.seeking
            && state.secondary_subtitle_delay == Some(0.0)
            && state
                .secondary_subtitle_text
                .as_deref()
                .is_some_and(|s| s.contains("bonjour"))
            && state.position.is_some_and(|p| (p - 0.5).abs() < 0.05)
    });
    player
        .send(Command::ToggleSecondarySubtitleVisibility)
        .unwrap();
    let hidden = wait_for(&receiver, |state| {
        state.secondary_subtitle_visible == Some(false)
    });
    assert!(hidden.subtitle_visible);
    assert_eq!(selected_track(&hidden, TrackKind::Sub), Some(primary_id));
    assert!(
        hidden
            .secondary_subtitle_text
            .as_deref()
            .is_some_and(|s| s.contains("bonjour")),
        "Hidden secondary text {:?}, delay {:?}, clock {:?}",
        hidden.secondary_subtitle_text,
        hidden.secondary_subtitle_delay,
        hidden.position,
    );
    player
        .send(Command::ToggleSecondarySubtitleVisibility)
        .unwrap();
    wait_for(&receiver, |state| {
        state.secondary_subtitle_visible == Some(true)
    });
    player.send(Command::ToggleSubtitleVisibility).unwrap();
    let primary_hidden = wait_for(&receiver, |state| !state.subtitle_visible);
    assert_eq!(primary_hidden.secondary_subtitle_visible, Some(true));
    player.send(Command::ToggleSubtitleVisibility).unwrap();
    wait_for(&receiver, |state| state.subtitle_visible);
    player
        .send(Command::SelectTrack {
            kind: TrackKind::Sub,
            id: None,
        })
        .unwrap();
    let secondary_only = wait_for(&receiver, |state| {
        selected_track(state, TrackKind::Sub).is_none()
            && state.tracks.iter().any(|t| {
                t.kind == TrackKind::Sub
                    && t.id == secondary_id
                    && t.selected
                    && t.main_selection == Some(1)
            })
    });
    assert!(secondary_only.paused);
    assert_eq!(secondary_only.error, None);
    player.send(Command::SelectSecondarySubtitle(None)).unwrap();
    let off = wait_for(&receiver, |state| {
        !state
            .tracks
            .iter()
            .any(|t| t.kind == TrackKind::Sub && t.selected)
    });
    assert!(off.paused);
    assert_eq!(off.error, None);
    player.send(Command::Load(fixture("catalog.mkv"))).unwrap();
    let replaced = wait_for(&receiver, |state| {
        state.generation > both.generation
            && state.phase == Phase::Playing
            && !state.tracks.iter().any(|t| t.kind == TrackKind::Sub)
    });
    assert!(
        !replaced
            .secondary_subtitle_text
            .as_deref()
            .is_some_and(|s| s.contains("bonjour"))
    );
    assert_eq!(replaced.error, None);
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
    assert_eq!(first.subtitle_font_size, Some(55.0));
    assert_eq!(first.subtitle_outline_size, Some(3.0));
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
    assert!(second.completed_navigation_seeks > first.completed_navigation_seeks);
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
    assert!(returned.completed_navigation_seeks > second.completed_navigation_seeks);
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
    wait_for(&receiver, |state| {
        state
            .subtitle_scale
            .is_some_and(|s| (s - 1.0).abs() < 0.001)
            && state.subtitle_position == Some(100.0)
    });
    // Drain neither observations nor UI updates during the bursts: native
    // relative commands must accumulate against the actual option values.
    for _ in 0..10 {
        player.send(Command::AdjustSubtitleScale(0.1)).unwrap();
    }
    wait_for(&receiver, |state| {
        state
            .subtitle_scale
            .is_some_and(|s| (s - 2.0).abs() < 0.001)
    });
    for _ in 0..10 {
        player.send(Command::AdjustSubtitleScale(-0.1)).unwrap();
    }
    wait_for(&receiver, |state| {
        state
            .subtitle_scale
            .is_some_and(|s| (s - 1.0).abs() < 0.001)
    });
    for _ in 0..15 {
        player.send(Command::AdjustSubtitlePosition(-1.0)).unwrap();
    }
    wait_for(&receiver, |state| state.subtitle_position == Some(85.0));
    for _ in 0..15 {
        player.send(Command::AdjustSubtitlePosition(1.0)).unwrap();
    }
    wait_for(&receiver, |state| state.subtitle_position == Some(100.0));
    for (delta, expected) in [(-100.0, 0.0), (200.0, 100.0), (-99.0, 1.0)] {
        player.send(Command::AdjustSubtitleScale(delta)).unwrap();
        wait_for(&receiver, |state| {
            state
                .subtitle_scale
                .is_some_and(|s| (s - expected).abs() < 0.001)
        });
    }
    for size in [25.0, 75.0, 55.0] {
        player.send(Command::SubtitleFontSize(size)).unwrap();
        let resized = wait_for(&receiver, |state| state.subtitle_font_size == Some(size));
        assert!(resized.paused);
        assert_eq!(resized.subtitle_position, Some(100.0));
        assert!(resized.subtitle_delay.abs() < 0.001);
        assert_eq!(resized.error, None);
    }
    for width in [0.0, 0.25, 5.0, 3.0] {
        player.send(Command::SubtitleOutlineSize(width)).unwrap();
        let outlined = wait_for(&receiver, |state| {
            state.subtitle_outline_size == Some(width)
        });
        assert!(outlined.paused);
        assert_eq!(outlined.subtitle_font_size, Some(55.0));
        assert_eq!(outlined.subtitle_position, Some(100.0));
        assert!(outlined.subtitle_delay.abs() < 0.001);
        assert_eq!(outlined.error, None);
    }
    for scale in [1.0 / 6.0, 6.0, 1.0] {
        player.send(Command::SubtitleScale(scale)).unwrap();
        let scaled = wait_for(&receiver, |state| {
            state
                .subtitle_scale
                .is_some_and(|s| (s - scale).abs() < 0.001)
        });
        assert!(scaled.paused);
        assert_eq!(scaled.subtitle_position, Some(100.0));
        assert!(scaled.subtitle_delay.abs() < 0.001);
        assert!(
            scaled
                .tracks
                .iter()
                .any(|t| t.kind == TrackKind::Sub && t.external && t.selected)
        );
        assert_eq!(scaled.error, None);
    }
    for (delta, expected) in [(-1000.0, 0.0), (1000.0, 150.0), (-50.0, 100.0)] {
        player.send(Command::AdjustSubtitlePosition(delta)).unwrap();
        let adjusted = wait_for(&receiver, |state| state.subtitle_position == Some(expected));
        assert!(adjusted.paused);
        assert_eq!(adjusted.error, None);
        assert!((adjusted.subtitle_delay).abs() < 0.001);
        assert!(
            adjusted
                .tracks
                .iter()
                .any(|t| t.kind == TrackKind::Sub && t.external && t.selected)
        );
    }
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
