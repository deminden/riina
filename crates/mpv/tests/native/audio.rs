use super::*;

#[test]
#[ignore = "requires provisioned libmpv; observes audio chapter metadata through native playback"]
fn chapter_metadata_follows_the_playhead_and_clears_on_replacement() {
    let root = fixture_root();
    let fixture = riina_test_support::TempDir::new("audio-chapters").unwrap();
    let wave = fixture.path().join("session.wav");
    std::fs::copy(root.join("tests/fixtures/external-tone.wav"), &wave).unwrap();
    let cue = fixture.path().join("session.cue");
    std::fs::write(
        &cue,
        r#"TITLE "Recorded session"
FILE "session.wav" WAVE
  TRACK 01 AUDIO
    TITLE "Introduction"
    PERFORMER "First performer"
    INDEX 01 00:00:00
  TRACK 02 AUDIO
    TITLE "Main set"
    PERFORMER "Second performer"
    INDEX 01 00:01:00
"#,
    )
    .unwrap();
    let harness = Harness::with_options(&root, vec![("ao".into(), "null".into())]).unwrap();
    harness.send(Command::SetPauseWhenOpen(true)).unwrap();
    harness
        .send(Command::Load(cue.to_str().unwrap().into()))
        .unwrap();
    let first = harness.wait(|state| {
        state.paused
            && state.current_chapter_index() == Some(0)
            && state.chapter_performer.as_deref() == Some("First performer")
    });
    assert_eq!(first.music_metadata().title, "Introduction");
    harness.send(Command::AdjustChapter(1)).unwrap();
    let second = harness.wait(|state| {
        state.current_chapter_index() == Some(1)
            && state.chapter_performer.as_deref() == Some("Second performer")
    });
    assert!(second.paused);
    assert_eq!(second.music_metadata().title, "Main set");
    assert_eq!(second.file_metadata(), first.file_metadata());
    harness
        .send(Command::Load(wave.to_str().unwrap().into()))
        .unwrap();
    let replacement =
        harness.wait(|state| state.generation > second.generation && state.phase == Phase::Paused);
    assert_eq!(replacement.current_chapter_index(), None);
    assert_eq!(replacement.chapter_performer, None);
    assert_eq!(replacement.music_metadata(), replacement.file_metadata());
}

#[test]
#[ignore = "requires provisioned libmpv; verifies disconnected preferred audio device at startup"]
fn missing_preferred_device_keeps_automatic_audio_before_first_playback() {
    let root = fixture_root();
    let harness = Harness::with_options(
        &root,
        riina_mpv::StartupOptions {
            mpv: vec![("ao".into(), "null".into())],
            preferred_audio_device: Some("unavailable/riina-test-endpoint".into()),
        },
    )
    .unwrap();
    harness.send(Command::SetPauseWhenOpen(true)).unwrap();
    harness
        .send(Command::Load(
            root.join("tests/fixtures/moving-square.avi")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    let loaded = harness.wait(|state| {
        state.phase == Phase::Failed
            || (state.paused
                && state.position.is_some()
                && state.audio_output.is_some()
                && state
                    .tracks
                    .iter()
                    .any(|track| track.kind == TrackKind::Audio && track.selected))
    });
    assert_eq!(loaded.error(), None);
    assert_eq!(loaded.audio_device.as_deref(), Some("auto"));
    assert!(
        loaded
            .tracks
            .iter()
            .any(|track| track.kind == TrackKind::Audio && track.selected)
    );
}

#[test]
#[ignore = "requires provisioned libmpv; verifies live ReplayGain values during paused playback"]
fn replay_gain_changes_all_options_without_reloading_playback() {
    use riina_core::audio_preferences::{ReplayGain, ReplayGainOptions};

    let root = fixture_root();
    let harness = Harness::with_options(&root, vec![("ao".into(), "null".into())]).unwrap();
    harness.send(Command::SetPauseWhenOpen(true)).unwrap();
    harness
        .send(Command::Load(
            root.join("tests/fixtures/moving-square.avi")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    let loaded = harness.wait(|state| {
        state.paused
            && state.position.is_some()
            && state.audio_output.is_some()
            && state
                .tracks
                .iter()
                .any(|track| track.kind == TrackKind::Audio && track.selected)
    });
    let properties = [
        ("replaygain", serde_json::json!("album")),
        ("replaygain-preamp", serde_json::json!(-2.5)),
        ("replaygain-clip", serde_json::json!(true)),
        ("replaygain-fallback", serde_json::json!(-4.0)),
    ];
    for (name, _) in &properties {
        harness
            .send(Command::ObserveMpvProperty((*name).into()))
            .unwrap();
    }
    harness
        .send(Command::SetReplayGain(ReplayGainOptions {
            mode: ReplayGain::Album,
            preamp: -2.5,
            clip: true,
            fallback: -4.0,
        }))
        .unwrap();
    let applied = harness.wait(|state| {
        state.error().is_some()
            || properties
                .iter()
                .all(|(name, expected)| state.observed_mpv_properties.get(*name) == Some(expected))
    });
    assert_eq!(applied.error(), None);
    assert!(applied.paused);
    assert_eq!(applied.generation, loaded.generation);
    assert_eq!(applied.position, loaded.position);
    assert_eq!(applied.volume, loaded.volume);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies audio-device selection through observed property state"]
fn audio_device_selection_updates_the_live_engine() {
    let root = fixture_root();
    let harness = Harness::with_options(&root, vec![("ao".into(), "null".into())]).unwrap();
    let initial = harness.wait(|state| state.audio_device.is_some());
    assert_eq!(initial.error(), None);
    eprintln!(
        "audio output driver: {:?}; observed devices: {}",
        initial.audio_output,
        initial.audio_devices.len()
    );
    if initial.audio_devices.is_empty() {
        // Some headless Windows sessions expose the AO but no endpoint list.
        drop(harness);
        return;
    }
    assert!(
        initial
            .audio_devices
            .iter()
            .any(|device| { Some(device.name.as_str()) == initial.audio_device.as_deref() })
    );
    let alternative = initial
        .audio_devices
        .iter()
        .find(|device| Some(device.name.as_str()) != initial.audio_device.as_deref());
    eprintln!(
        "observed {} audio device(s); alternate selection available: {}",
        initial.audio_devices.len(),
        alternative.is_some()
    );
    if let Some(alternative) = alternative {
        let selected = alternative.name.clone();
        harness
            .send(Command::AudioDevice(selected.clone()))
            .unwrap();
        let changed =
            harness.wait(|state| state.audio_device.as_deref() == Some(selected.as_str()));
        assert_eq!(changed.error(), None);
    }
    drop(harness);
    if let Some(alternative) = alternative {
        let preferred = Harness::with_options(
            &root,
            riina_mpv::StartupOptions {
                mpv: vec![("ao".into(), "null".into())],
                preferred_audio_device: Some(alternative.name.clone()),
            },
        )
        .unwrap();
        let observed = preferred.wait(|state| state.audio_device.is_some());
        assert_eq!(
            observed.audio_device.as_deref(),
            Some(alternative.name.as_str())
        );
        assert_eq!(observed.error(), None);
    }
}

#[test]
#[ignore = "requires provisioned libmpv; verifies live equalizer filter replacement and observation"]
fn audio_equalizer_applies_updates_and_returns_to_flat() {
    let root = fixture_root();
    let harness = Harness::with_options(&root, vec![("ao".into(), "null".into())]).unwrap();
    harness
        .send(Command::Load(
            root.join("tests/fixtures/external-tone.wav")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    let loaded = harness.wait(|state| {
        state.error().is_some()
            || (state.phase == Phase::Playing && state.audio_equalizer.is_some())
    });
    assert_eq!(loaded.error(), None);
    assert_eq!(loaded.audio_equalizer, Some([0.; 10]));

    let mut gains = [0.; 10];
    gains[4] = 4.5;
    harness.send(Command::AudioEqualizer(gains)).unwrap();
    let applied = harness.wait(|state| {
        state.error().is_some()
            || state.audio_equalizer.is_some_and(|observed| {
                observed
                    .iter()
                    .zip(gains)
                    .all(|(actual, expected)| (actual - expected).abs() < 0.01)
            })
    });
    assert_eq!(applied.error(), None);

    harness.send(Command::AudioEqualizer([0.; 10])).unwrap();
    let reset =
        harness.wait(|state| state.error().is_some() || state.audio_equalizer == Some([0.; 10]));
    assert_eq!(reset.error(), None);
    drop(harness);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies remembered user volume and temporary restoration"]
fn remembered_volume_tracks_native_user_adjustments() {
    let root = fixture_root();
    let harness = Harness::with_options(&root, vec![("ao".into(), "null".into())]).unwrap();
    harness.send(Command::RestoreVolume(41.0)).unwrap();
    let restored = harness.wait(|state| state.volume == 41.0);
    assert_eq!(restored.remembered_volume, None);

    harness
        .send(Command::AdjustVolume {
            delta: 6.0,
            boost: false,
        })
        .unwrap();
    let adjusted = harness.wait(|state| state.remembered_volume.is_some());
    let remembered = adjusted.remembered_volume.unwrap();
    assert_eq!(remembered.level, 47.0);
    harness.send(Command::RestoreVolume(31.0)).unwrap();
    let restored = harness.wait(|state| state.volume == 31.0);
    assert_eq!(restored.remembered_volume, Some(remembered));

    harness.send(Command::Volume(150.0)).unwrap();
    let capped = harness.wait(|state| {
        state
            .remembered_volume
            .is_some_and(|volume| volume.revision > remembered.revision)
    });
    assert_eq!(capped.remembered_volume.unwrap().level, 100.0);
}
