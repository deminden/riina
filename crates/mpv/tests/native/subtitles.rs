use super::*;

#[test]
#[ignore = "requires provisioned libmpv; verifies selected last-cue navigation before an automatic loop"]
fn last_cue_navigation_does_not_label_a_later_automatic_loop() {
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
    harness
        .send(Command::AddSubtitle(
            root.join("tests/fixtures/manual-captions.srt")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    harness.wait(|state| {
        state
            .tracks
            .iter()
            .any(|track| track.kind == TrackKind::Sub && track.is_primary_selected())
    });
    harness
        .send(Command::Seek {
            seconds: 3.0,
            absolute: true,
        })
        .unwrap();
    harness.wait(|state| {
        !state.seeking
            && state
                .position
                .is_some_and(|clock| (clock - 3.0).abs() < 0.05)
            && state
                .subtitle_text
                .as_deref()
                .is_some_and(|text| text.contains("Chapter two"))
    });
    harness
        .send(Command::SetAbLoop {
            a: Some(2.8),
            b: Some(3.2),
        })
        .unwrap();
    let before = harness.wait(|state| state.ab_loop_a == Some(2.8) && state.ab_loop_b == Some(3.2));
    harness.send(Command::SubtitleSeek(1)).unwrap();
    harness.send(Command::Volume(37.0)).unwrap();
    let baseline = harness.wait(|state| state.volume == 37.0);
    assert_eq!(
        baseline.completed_navigation_seeks, before.completed_navigation_seeks,
        "Past-last-cue request unexpectedly moved playback"
    );
    harness.send(Command::TogglePause).unwrap();
    let looped =
        harness.wait(|state| state.completed_seeks > baseline.completed_seeks && !state.seeking);
    assert_eq!(
        looped.completed_navigation_seeks,
        baseline.completed_navigation_seeks
    );
    assert_eq!(looped.volume, 37.0);
    assert!(looped.error().is_none(), "{:?}", looped.error());
}

#[test]
#[ignore = "requires provisioned libmpv; verifies no-cue navigation does not label an automatic loop"]
fn no_cue_navigation_does_not_arm_automatic_loop_feedback() {
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
    harness
        .send(Command::Seek {
            seconds: 0.3,
            absolute: true,
        })
        .unwrap();
    harness.wait(|state| state.completed_navigation_seeks > 0 && !state.seeking);
    harness
        .send(Command::SetAbLoop {
            a: Some(0.2),
            b: Some(0.6),
        })
        .unwrap();
    harness.wait(|state| state.ab_loop_a == Some(0.2) && state.ab_loop_b == Some(0.6));
    harness.send(Command::SubtitleSeek(1)).unwrap();
    harness.send(Command::Volume(37.0)).unwrap();
    let baseline = harness.wait(|state| state.volume == 37.0);
    harness.send(Command::TogglePause).unwrap();
    let looped =
        harness.wait(|state| state.completed_seeks > baseline.completed_seeks && !state.seeking);
    assert_eq!(
        looped.completed_navigation_seeks,
        baseline.completed_navigation_seeks
    );
    assert_eq!(looped.volume, 37.0);
    assert!(looped.error().is_none(), "{:?}", looped.error());
}

#[test]
#[ignore = "requires provisioned libmpv; verifies independent subtitle color channels and defaults"]
fn subtitle_colors_preserve_alpha_and_other_style_options() {
    use riina_core::subtitle_color::SubtitleColor;
    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    let defaults = harness.wait(|state| {
        state.subtitle_text_color == Some(SubtitleColor::WHITE)
            && state.subtitle_border_color == Some(SubtitleColor::BLACK)
            && state.subtitle_shadow_color == Some(SubtitleColor::CLEAR)
            && state.subtitle_font_size == Some(55.0)
            && state.subtitle_outline_size == Some(3.0)
            && state.subtitle_bold == Some(false)
            && state.subtitle_italic == Some(false)
            && state.subtitle_blur == Some(0.0)
            && state.subtitle_spacing == Some(0.0)
            && state.subtitle_shadow_offset == Some(0.0)
    });
    assert_eq!(defaults.error(), None);
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
    harness
        .send(Command::AddSubtitle(
            root.join("tests/fixtures/manual-captions.srt")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    let selected = harness.wait(|state| selected_track(state, TrackKind::Sub).is_some());
    let subtitle_id = selected_track(&selected, TrackKind::Sub);
    harness
        .send(Command::Seek {
            seconds: 0.5,
            absolute: true,
        })
        .unwrap();
    harness.wait(|state| {
        state
            .subtitle_text
            .as_deref()
            .is_some_and(|text| text.contains("café"))
    });
    let text = SubtitleColor::from_mpv("#80402010").unwrap();
    let border = SubtitleColor::from_mpv("#C0102040").unwrap();
    let shadow = SubtitleColor::from_mpv("#40000000").unwrap();
    harness.send(Command::SubtitleTextColor(text)).unwrap();
    let colored = harness.wait(|state| state.subtitle_text_color == Some(text));
    assert_eq!(colored.subtitle_border_color, Some(SubtitleColor::BLACK));
    assert_eq!(colored.subtitle_shadow_color, Some(SubtitleColor::CLEAR));
    harness.send(Command::SubtitleBorderColor(border)).unwrap();
    let outlined = harness.wait(|state| state.subtitle_border_color == Some(border));
    assert_eq!(outlined.subtitle_text_color, Some(text));
    assert_eq!(outlined.subtitle_shadow_color, Some(SubtitleColor::CLEAR));
    harness.send(Command::SubtitleShadowColor(shadow)).unwrap();
    let shaded = harness.wait(|state| state.subtitle_shadow_color == Some(shadow));
    assert_eq!(shaded.subtitle_text_color, Some(text));
    assert_eq!(shaded.subtitle_border_color, Some(border));
    assert_eq!(shaded.subtitle_font_size, Some(55.0));
    assert_eq!(shaded.subtitle_outline_size, Some(3.0));
    assert!(shaded.paused);
    assert_eq!(selected_track(&shaded, TrackKind::Sub), subtitle_id);
    assert_eq!(shaded.subtitle_delay, 0.0);
    assert_eq!(shaded.error(), None);
    for command in [
        Command::SubtitleBold(true),
        Command::SubtitleItalic(true),
        Command::SubtitleBlur(1.25),
        Command::SubtitleSpacing(-0.5),
        Command::SubtitleOutlineSize(4.5),
        Command::SubtitleShadowOffset(2.25),
    ] {
        harness.send(command).unwrap();
    }
    let styled = harness.wait(|state| {
        state.subtitle_bold == Some(true)
            && state.subtitle_italic == Some(true)
            && state.subtitle_blur == Some(1.25)
            && state.subtitle_spacing == Some(-0.5)
            && state.subtitle_outline_size == Some(4.5)
            && state.subtitle_shadow_offset == Some(2.25)
    });
    assert_eq!(styled.subtitle_text_color, Some(text));
    assert_eq!(styled.subtitle_border_color, Some(border));
    assert_eq!(styled.subtitle_shadow_color, Some(shadow));
    assert!(styled.paused);
    assert_eq!(selected_track(&styled, TrackKind::Sub), subtitle_id);
    assert_eq!(styled.error(), None);
    for command in [
        Command::SubtitleBold(false),
        Command::SubtitleItalic(false),
        Command::SubtitleBlur(0.0),
        Command::SubtitleSpacing(0.0),
        Command::SubtitleOutlineSize(3.0),
        Command::SubtitleShadowOffset(0.0),
    ] {
        harness.send(command).unwrap();
    }
    let neutral = harness.wait(|state| {
        state.subtitle_bold == Some(false)
            && state.subtitle_italic == Some(false)
            && state.subtitle_blur == Some(0.0)
            && state.subtitle_spacing == Some(0.0)
            && state.subtitle_outline_size == Some(3.0)
            && state.subtitle_shadow_offset == Some(0.0)
    });
    assert!(neutral.paused);
    assert_eq!(selected_track(&neutral, TrackKind::Sub), subtitle_id);
    for command in [
        Command::SubtitleTextColor(SubtitleColor::WHITE),
        Command::SubtitleBorderColor(SubtitleColor::BLACK),
        Command::SubtitleShadowColor(SubtitleColor::CLEAR),
    ] {
        harness.send(command).unwrap();
    }
    let restored = harness.wait(|state| {
        state.subtitle_text_color == Some(SubtitleColor::WHITE)
            && state.subtitle_border_color == Some(SubtitleColor::BLACK)
            && state.subtitle_shadow_color == Some(SubtitleColor::CLEAR)
    });
    assert!(restored.paused);
    assert_eq!(selected_track(&restored, TrackKind::Sub), subtitle_id);
    assert_eq!(restored.error(), None);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies configured subtitle font names and independent style state"]
fn subtitle_font_selection_preserves_other_style_options() {
    use riina_core::subtitle_controls::{DEFAULT_SUBTITLE_FONT, font_from_text};
    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    harness.wait(|state| state.subtitle_font.as_deref() == Some(DEFAULT_SUBTITLE_FONT));
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
    harness
        .send(Command::AddSubtitle(
            root.join("tests/fixtures/manual-captions.srt")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    let selected = harness.wait(|state| selected_track(state, TrackKind::Sub).is_some());
    let subtitle_id = selected_track(&selected, TrackKind::Sub);
    harness
        .send(Command::Seek {
            seconds: 0.5,
            absolute: true,
        })
        .unwrap();
    let before = harness.wait(|state| {
        state
            .subtitle_text
            .as_deref()
            .is_some_and(|text| text.contains("café"))
    });
    // The custom entry is an engine preference, not proof a matching font is
    // installed. Missing families must remain eligible for libass fallback.
    for name in ["serif", "monospace", "Riina 字体 Привет", ""] {
        harness.send(font_from_text(name).unwrap()).unwrap();
        let expected = if name.is_empty() {
            DEFAULT_SUBTITLE_FONT
        } else {
            name
        };
        let changed = harness.wait(|state| state.subtitle_font.as_deref() == Some(expected));
        assert!(changed.paused);
        assert_eq!(selected_track(&changed, TrackKind::Sub), subtitle_id);
        assert_eq!(changed.subtitle_font_size, before.subtitle_font_size);
        assert_eq!(changed.subtitle_outline_size, before.subtitle_outline_size);
        assert_eq!(changed.subtitle_text_color, before.subtitle_text_color);
        assert_eq!(changed.subtitle_border_color, before.subtitle_border_color);
        assert_eq!(changed.subtitle_shadow_color, before.subtitle_shadow_color);
        assert_eq!(changed.subtitle_delay, before.subtitle_delay);
        assert_eq!(changed.error(), None);
    }
}

#[test]
#[ignore = "requires provisioned libmpv; verifies encoding reload restores both subtitle lanes"]
fn encoding_reload_preserves_both_selected_subtitle_files() {
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
    harness.send(Command::SetPaused(true)).unwrap();
    harness.wait(|state| state.paused);

    let scratch = riina_test_support::TempDir::new("subtitle-encoding").unwrap();
    let primary = scratch.path().join("primary.srt");
    let secondary = scratch.path().join("secondary.srt");
    let write = |file: &std::path::Path, text: &str| {
        std::fs::write(file, format!("1\n00:00:00,000 --> 00:00:04,000\n{text}\n")).unwrap();
    };
    let add = |file: &std::path::Path| {
        harness
            .send(Command::AddSubtitle(file.to_string_lossy().into_owned()))
            .unwrap();
    };
    write(&primary, "First primary cue");
    add(&primary);
    let first = harness.wait(|state| selected_track(state, TrackKind::Sub).is_some());
    let primary_id = selected_track(&first, TrackKind::Sub).unwrap();
    write(&secondary, "First secondary cue");
    add(&secondary);
    let second = harness
        .wait(|state| selected_track(state, TrackKind::Sub).is_some_and(|id| id != primary_id));
    let secondary_id = selected_track(&second, TrackKind::Sub).unwrap();
    harness
        .send(Command::SelectTrack {
            kind: TrackKind::Sub,
            id: Some(primary_id),
        })
        .unwrap();
    harness
        .send(Command::SelectSecondarySubtitle(Some(secondary_id)))
        .unwrap();
    let seek = |seconds| {
        harness
            .send(Command::Seek {
                seconds,
                absolute: true,
            })
            .unwrap();
    };
    let cues = |state: &Playback, primary, secondary| {
        state.subtitle_text.as_deref() == Some(primary)
            && state.secondary_subtitle_text.as_deref() == Some(secondary)
    };
    seek(0.5);
    let original = harness.wait(|state| cues(state, "First primary cue", "First secondary cue"));

    write(&primary, "Edited primary — café");
    write(&secondary, "Edited secondary — bonjour");
    harness
        .send(Command::SubtitleEncoding("UTF-8".into()))
        .unwrap();
    seek(0.8);
    let reloaded =
        harness.wait(|state| cues(state, "Edited primary — café", "Edited secondary — bonjour"));
    assert!(reloaded.paused);
    assert_eq!(reloaded.generation, original.generation);
    assert_eq!(reloaded.tracks.len(), original.tracks.len());
    assert_eq!(reloaded.error(), None);

    write(&secondary, "Secondary only — résumé");
    for command in [
        Command::SubtitleEncoding("auto".into()),
        Command::SubtitleEncoding("UTF-8".into()),
        Command::SelectTrack {
            kind: TrackKind::Sub,
            id: None,
        },
    ] {
        harness.send(command).unwrap();
    }
    seek(1.0);
    let reloaded = harness.wait(|state| {
        state.subtitle_text.as_deref().is_none_or(str::is_empty)
            && state.secondary_subtitle_text.as_deref() == Some("Secondary only — résumé")
    });
    assert_eq!(reloaded.tracks.len(), original.tracks.len());
    assert_eq!(reloaded.error(), None);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies selected ASS warning across subtitle lanes"]
fn subtitle_style_warning_follows_selected_ass_lanes() {
    let root = fixture_root();
    let fixture = |name: &str| {
        root.join("tests/fixtures")
            .join(name)
            .to_string_lossy()
            .into_owned()
    };
    let harness = Harness::start(&root).unwrap();
    harness.send(Command::Load(fixture("catalog.mkv"))).unwrap();
    harness.wait(|state| state.phase == Phase::Playing);
    harness.send(Command::TogglePause).unwrap();
    harness.wait(|state| state.paused);
    harness
        .send(Command::AddSubtitle(fixture("styled-captions.ass")))
        .unwrap();
    let styled = harness.wait(|state| state.subtitle_style_availability().ass_warning);
    let ass_id = selected_track(&styled, TrackKind::Sub).unwrap();
    assert!(styled.subtitle_style_availability().text_controls);
    harness
        .send(Command::Seek {
            seconds: 0.5,
            absolute: true,
        })
        .unwrap();
    let decoded = harness.wait(|state| {
        state
            .subtitle_text
            .as_deref()
            .is_some_and(|text| text.contains("Styled first cue"))
    });
    assert!(decoded.paused);
    assert_eq!(decoded.error(), None);
    harness
        .send(Command::AddSubtitle(fixture("manual-captions.srt")))
        .unwrap();
    let plain =
        harness.wait(|state| selected_track(state, TrackKind::Sub).is_some_and(|id| id != ass_id));
    let plain_id = selected_track(&plain, TrackKind::Sub).unwrap();
    assert!(!plain.subtitle_style_availability().ass_warning);
    harness
        .send(Command::SelectSecondarySubtitle(Some(ass_id)))
        .unwrap();
    let mixed = harness.wait(|state| state.subtitle_style_availability().ass_warning);
    assert_eq!(selected_track(&mixed, TrackKind::Sub), Some(plain_id));
    assert!(mixed.subtitle_style_availability().text_controls);
    assert!(mixed.paused);
    assert_eq!(mixed.error(), None);
    harness
        .send(Command::SelectTrack {
            kind: TrackKind::Sub,
            id: None,
        })
        .unwrap();
    let secondary_only = harness.wait(|state| selected_track(state, TrackKind::Sub).is_none());
    assert!(secondary_only.subtitle_style_availability().ass_warning);
    harness
        .send(Command::SelectSecondarySubtitle(None))
        .unwrap();
    let disabled = harness.wait(|state| !state.subtitle_style_availability().ass_warning);
    assert!(disabled.subtitle_style_availability().text_controls);
    assert!(disabled.paused);
    assert_eq!(disabled.error(), None);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies independent primary and secondary subtitle lanes"]
#[expect(
    clippy::too_many_lines,
    reason = "decomposition tracked in docs/review"
)]
fn secondary_subtitles_keep_selection_and_visibility_independent() {
    let root = fixture_root();
    let fixture = |name: &str| {
        root.join("tests/fixtures")
            .join(name)
            .to_string_lossy()
            .into_owned()
    };
    let harness = Harness::start(&root).unwrap();
    harness.send(Command::Load(fixture("catalog.mkv"))).unwrap();
    harness.wait(|state| state.phase == Phase::Playing);
    harness.send(Command::TogglePause).unwrap();
    harness.wait(|state| state.paused);
    harness
        .send(Command::AddSubtitle(fixture("manual-captions.srt")))
        .unwrap();
    let first = harness.wait(|state| selected_track(state, TrackKind::Sub).is_some());
    let primary_id = selected_track(&first, TrackKind::Sub).unwrap();
    harness
        .send(Command::AddSubtitle(fixture("secondary-captions.srt")))
        .unwrap();
    let second = harness
        .wait(|state| selected_track(state, TrackKind::Sub).is_some_and(|id| id != primary_id));
    let secondary_id = selected_track(&second, TrackKind::Sub).unwrap();
    harness
        .send(Command::SelectTrack {
            kind: TrackKind::Sub,
            id: Some(primary_id),
        })
        .unwrap();
    harness
        .send(Command::SelectSecondarySubtitle(Some(secondary_id)))
        .unwrap();
    harness
        .send(Command::Seek {
            seconds: 0.5,
            absolute: true,
        })
        .unwrap();
    let both = harness.wait(|state| {
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
    harness.send(Command::SecondarySubtitleDelay(1.0)).unwrap();
    harness.wait(|state| state.secondary_subtitle_delay == Some(1.0));
    harness
        .send(Command::Seek {
            seconds: 2.3,
            absolute: true,
        })
        .unwrap();
    let shifted = harness.wait(|state| {
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
    assert_eq!(shifted.error(), None);
    for position in [25.0, 0.0, 150.0, 100.0] {
        harness
            .send(Command::SecondarySubtitlePosition(position))
            .unwrap();
        let positioned = harness.wait(|state| state.secondary_subtitle_position == Some(position));
        assert!(positioned.paused);
        assert_eq!(positioned.subtitle_position, both.subtitle_position);
        assert_eq!(positioned.subtitle_delay, both.subtitle_delay);
        assert_eq!(positioned.secondary_subtitle_delay, Some(1.0));
        assert_eq!(
            selected_track(&positioned, TrackKind::Sub),
            Some(primary_id)
        );
        assert_eq!(positioned.error(), None);
    }
    // Exercise the opposite lane after secondary edits to catch target aliasing.
    for position in [40.0, 0.0, 150.0, 100.0] {
        harness.send(Command::SubtitlePosition(position)).unwrap();
        let positioned = harness.wait(|state| state.subtitle_position == Some(position));
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
        assert_eq!(positioned.error(), None);
    }
    // Numeric entry can exceed the visual slider's range in either lane.
    harness.send(Command::SubtitleDelay(6.125)).unwrap();
    harness.wait(|state| state.subtitle_delay == 6.125);
    harness
        .send(Command::SecondarySubtitleDelay(-7.375))
        .unwrap();
    let wide_delays = harness.wait(|state| state.secondary_subtitle_delay == Some(-7.375));
    assert_eq!(wide_delays.subtitle_delay, 6.125);
    assert!(wide_delays.paused);
    assert_eq!(
        selected_track(&wide_delays, TrackKind::Sub),
        Some(primary_id)
    );
    assert_eq!(wide_delays.error(), None);
    harness
        .send(Command::SubtitleDelay(both.subtitle_delay))
        .unwrap();
    let before_seek = harness.wait(|state| state.subtitle_delay == both.subtitle_delay);
    harness.send(Command::SecondarySubtitleDelay(0.0)).unwrap();
    harness
        .send(Command::Seek {
            seconds: 0.5,
            absolute: true,
        })
        .unwrap();
    harness.wait(|state| {
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
    harness
        .send(Command::ToggleSecondarySubtitleVisibility)
        .unwrap();
    let hidden = harness.wait(|state| state.secondary_subtitle_visible == Some(false));
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
    harness
        .send(Command::ToggleSecondarySubtitleVisibility)
        .unwrap();
    harness.wait(|state| state.secondary_subtitle_visible == Some(true));
    harness.send(Command::ToggleSubtitleVisibility).unwrap();
    let primary_hidden = harness.wait(|state| !state.subtitle_visible);
    assert_eq!(primary_hidden.secondary_subtitle_visible, Some(true));
    harness.send(Command::ToggleSubtitleVisibility).unwrap();
    harness.wait(|state| state.subtitle_visible);
    harness
        .send(Command::SelectTrack {
            kind: TrackKind::Sub,
            id: None,
        })
        .unwrap();
    let secondary_only = harness.wait(|state| {
        selected_track(state, TrackKind::Sub).is_none()
            && state.tracks.iter().any(|t| {
                t.kind == TrackKind::Sub
                    && t.id == secondary_id
                    && t.selected
                    && t.main_selection == Some(1)
            })
    });
    assert!(secondary_only.paused);
    assert_eq!(secondary_only.error(), None);
    harness
        .send(Command::SelectSecondarySubtitle(None))
        .unwrap();
    let off = harness.wait(|state| {
        !state
            .tracks
            .iter()
            .any(|t| t.kind == TrackKind::Sub && t.selected)
    });
    assert!(off.paused);
    assert_eq!(off.error(), None);
    harness.send(Command::Load(fixture("catalog.mkv"))).unwrap();
    let replaced = harness.wait(|state| {
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
    assert_eq!(replaced.error(), None);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies subtitle cue navigation with observed clock and text"]
#[expect(
    clippy::too_many_lines,
    reason = "decomposition tracked in docs/review"
)]
fn subtitle_navigation_seeks_cues_without_changing_delay() {
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
        .send(Command::AddSubtitle(
            root.join("tests/fixtures/manual-captions.srt")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    harness.wait(|state| {
        state
            .tracks
            .iter()
            .any(|t| t.kind == TrackKind::Sub && t.external && t.selected)
    });
    harness
        .send(Command::Seek {
            seconds: 0.5,
            absolute: true,
        })
        .unwrap();
    let first = harness.wait(|state| {
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
    harness.send(Command::SubtitleSeek(1)).unwrap();
    let second = harness.wait(|state| {
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
    assert_eq!(second.error(), None);
    harness.send(Command::SubtitleSeek(-1)).unwrap();
    let returned = harness.wait(|state| {
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
    assert_eq!(returned.error(), None);
    harness.send(Command::SubtitleDelay(0.25)).unwrap();
    harness.wait(|state| (state.subtitle_delay - 0.25).abs() < 0.001);
    harness
        .send(Command::Seek {
            seconds: 0.5,
            absolute: true,
        })
        .unwrap();
    let delayed_first = harness.wait(|state| {
        state.paused
            && !state.seeking
            && state.completed_seeks > returned.completed_seeks
            && state.position.is_some_and(|p| (p - 0.5).abs() < 0.05)
            && state
                .subtitle_text
                .as_deref()
                .is_some_and(|text| text.contains("Привет"))
    });
    harness.send(Command::SubtitleSeek(1)).unwrap();
    let delayed_second = harness.wait(|state| {
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
    harness
        .send(Command::Seek {
            seconds: 2.8,
            absolute: true,
        })
        .unwrap();
    let inside = harness.wait(|state| {
        state.paused
            && !state.seeking
            && state.completed_seeks > delayed_second.completed_seeks
            && state.position.is_some_and(|p| (p - 2.8).abs() < 0.05)
    });
    harness.send(Command::SubtitleSeek(0)).unwrap();
    let restarted = harness.wait(|state| {
        state.paused
            && !state.seeking
            && state.completed_seeks > inside.completed_seeks
            && state.position.is_some_and(|p| (2.15..2.4).contains(&p))
    });
    assert!((restarted.subtitle_delay - 0.25).abs() < 0.001);
    assert_eq!(restarted.error(), None);

    // Queue the burst without reading snapshots: each adjustment must use the
    // decoder's current delay rather than the UI's last observed value.
    for _ in 0..20 {
        harness.send(Command::AdjustSubtitleDelay(0.1)).unwrap();
    }
    let increased = harness.wait(|state| (state.subtitle_delay - 2.25).abs() < 0.001);
    assert!(increased.paused);
    assert_eq!(increased.error(), None);
    for _ in 0..4 {
        harness.send(Command::AdjustSubtitleDelay(-0.5)).unwrap();
    }
    let restored = harness.wait(|state| (state.subtitle_delay - 0.25).abs() < 0.001);
    assert!(restored.paused);
    harness.send(Command::SubtitleDelay(0.0)).unwrap();
    let reset = harness.wait(|state| state.subtitle_delay.abs() < 0.001);
    assert!(reset.paused);
    assert_eq!(reset.error(), None);
    harness.wait(|state| {
        state
            .subtitle_scale
            .is_some_and(|s| (s - 1.0).abs() < 0.001)
            && state.subtitle_position == Some(100.0)
    });
    // Drain neither observations nor UI updates during the bursts: native
    // relative commands must accumulate against the actual option values.
    for factor in [1.1, 0.9, 1.1] {
        harness
            .send(Command::MultiplySubtitleScale(factor))
            .unwrap();
    }
    harness.wait(|state| {
        state
            .subtitle_scale
            .is_some_and(|scale| (scale - 1.089).abs() < 0.001)
    });
    harness.send(Command::SubtitleScale(1.0)).unwrap();
    harness.wait(|state| {
        state
            .subtitle_scale
            .is_some_and(|scale| (scale - 1.0).abs() < 0.001)
    });
    for _ in 0..10 {
        harness.send(Command::AdjustSubtitleScale(0.1)).unwrap();
    }
    harness.wait(|state| {
        state
            .subtitle_scale
            .is_some_and(|s| (s - 2.0).abs() < 0.001)
    });
    for _ in 0..10 {
        harness.send(Command::AdjustSubtitleScale(-0.1)).unwrap();
    }
    harness.wait(|state| {
        state
            .subtitle_scale
            .is_some_and(|s| (s - 1.0).abs() < 0.001)
    });
    for _ in 0..15 {
        harness.send(Command::AdjustSubtitlePosition(-1.0)).unwrap();
    }
    harness.wait(|state| state.subtitle_position == Some(85.0));
    for _ in 0..15 {
        harness.send(Command::AdjustSubtitlePosition(1.0)).unwrap();
    }
    harness.wait(|state| state.subtitle_position == Some(100.0));
    for (delta, expected) in [(-100.0, 0.0), (200.0, 100.0), (-99.0, 1.0)] {
        harness.send(Command::AdjustSubtitleScale(delta)).unwrap();
        harness.wait(|state| {
            state
                .subtitle_scale
                .is_some_and(|s| (s - expected).abs() < 0.001)
        });
    }
    for size in [25.0, 75.0, 55.0] {
        harness.send(Command::SubtitleFontSize(size)).unwrap();
        let resized = harness.wait(|state| state.subtitle_font_size == Some(size));
        assert!(resized.paused);
        assert_eq!(resized.subtitle_position, Some(100.0));
        assert!(resized.subtitle_delay.abs() < 0.001);
        assert_eq!(resized.error(), None);
    }
    for width in [0.0, 0.25, 5.0, 3.0] {
        harness.send(Command::SubtitleOutlineSize(width)).unwrap();
        let outlined = harness.wait(|state| state.subtitle_outline_size == Some(width));
        assert!(outlined.paused);
        assert_eq!(outlined.subtitle_font_size, Some(55.0));
        assert_eq!(outlined.subtitle_position, Some(100.0));
        assert!(outlined.subtitle_delay.abs() < 0.001);
        assert_eq!(outlined.error(), None);
    }
    for scale in [1.0 / 6.0, 6.0, 1.0] {
        harness.send(Command::SubtitleScale(scale)).unwrap();
        let scaled = harness.wait(|state| {
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
        assert_eq!(scaled.error(), None);
    }
    for (delta, expected) in [(-1000.0, 0.0), (1000.0, 150.0), (-50.0, 100.0)] {
        harness
            .send(Command::AdjustSubtitlePosition(delta))
            .unwrap();
        let adjusted = harness.wait(|state| state.subtitle_position == Some(expected));
        assert!(adjusted.paused);
        assert_eq!(adjusted.error(), None);
        assert!((adjusted.subtitle_delay).abs() < 0.001);
        assert!(
            adjusted
                .tracks
                .iter()
                .any(|t| t.kind == TrackKind::Sub && t.external && t.selected)
        );
    }
}
