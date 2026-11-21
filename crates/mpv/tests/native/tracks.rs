use super::*;

#[test]
#[ignore = "requires provisioned libmpv; verifies real ISO and BCP 47 track metadata"]
fn multilingual_tracks_preserve_language_metadata_across_subtitle_lanes() {
    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    harness
        .send(Command::Load(
            root.join("tests/fixtures/track-languages.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    let loaded = harness.wait(|state| {
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
    harness.send(Command::TogglePause).unwrap();
    harness.wait(|state| state.paused);
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
    harness
        .send(Command::SelectTrack {
            kind: TrackKind::Sub,
            id: Some(primary),
        })
        .unwrap();
    harness.wait(|state| selected_track(state, TrackKind::Sub) == Some(primary));
    harness
        .send(Command::SelectSecondarySubtitle(Some(secondary)))
        .unwrap();
    let both = harness.wait(|state| {
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
    let root = fixture_root();
    let fixture = |name: &str| {
        root.join("tests/fixtures")
            .join(name)
            .to_string_lossy()
            .into_owned()
    };
    let harness = Harness::start(&root).unwrap();
    harness.send(Command::Load(fixture("catalog.mkv"))).unwrap();
    let loaded = harness.wait(|state| {
        state.phase == Phase::Playing
            && state
                .tracks
                .iter()
                .any(|t| t.kind == TrackKind::Audio && t.selected)
    });
    let internal_audio = selected_track(&loaded, TrackKind::Audio).unwrap();
    let video = selected_track(&loaded, TrackKind::Video).unwrap();
    harness.send(Command::TogglePause).unwrap();
    harness.wait(|state| state.paused);
    harness
        .send(Command::AddAudio(fixture("external-tone.wav")))
        .unwrap();
    let external = harness.wait(|state| {
        state
            .tracks
            .iter()
            .any(|t| t.kind == TrackKind::Audio && t.external && t.selected)
    });
    let external_audio = selected_track(&external, TrackKind::Audio).unwrap();
    assert_ne!(internal_audio, external_audio);
    for expected in [None, Some(internal_audio), Some(external_audio)] {
        harness.send(Command::CycleTrack(TrackKind::Audio)).unwrap();
        let cycled = harness.wait(|state| selected_track(state, TrackKind::Audio) == expected);
        assert!(cycled.paused);
        assert_eq!(cycled.error, None);
        assert_eq!(selected_track(&cycled, TrackKind::Video), Some(video));
    }
    harness
        .send(Command::AddSubtitle(fixture("manual-captions.srt")))
        .unwrap();
    let subtitle = harness.wait(|state| {
        state
            .tracks
            .iter()
            .any(|t| t.kind == TrackKind::Sub && t.external && t.selected)
    });
    let subtitle_id = selected_track(&subtitle, TrackKind::Sub).unwrap();
    for expected in [None, Some(subtitle_id)] {
        harness.send(Command::CycleTrack(TrackKind::Sub)).unwrap();
        let cycled = harness.wait(|state| selected_track(state, TrackKind::Sub) == expected);
        assert!(cycled.paused);
        assert_eq!(cycled.error, None);
        assert_eq!(
            selected_track(&cycled, TrackKind::Audio),
            Some(external_audio)
        );
    }
    for expected in [None, Some(video)] {
        harness.send(Command::CycleTrack(TrackKind::Video)).unwrap();
        let cycled = harness.wait(|state| selected_track(state, TrackKind::Video) == expected);
        assert!(cycled.paused);
        assert_eq!(cycled.error, None);
        assert_eq!(
            selected_track(&cycled, TrackKind::Audio),
            Some(external_audio)
        );
        assert_eq!(selected_track(&cycled, TrackKind::Sub), Some(subtitle_id));
    }
}
