use super::*;

#[test]
#[ignore = "requires provisioned libmpv; verifies live pause-on-open policy without pausing current media"]
fn pause_when_open_applies_to_replacement_and_batch_without_changing_current_playback() {
    let root = fixture_root();
    let media = root
        .join("tests/fixtures/catalog.mkv")
        .to_string_lossy()
        .into_owned();
    let harness = Harness::start(&root).unwrap();
    harness.send(Command::Load(media.clone())).unwrap();
    let playing = harness.wait(|state| state.phase == Phase::Playing && state.position.is_some());
    assert!(!playing.paused);
    assert!(!playing.pause_when_open);
    harness.send(Command::SetPauseWhenOpen(true)).unwrap();
    harness.wait(|state| state.pause_when_open && !state.paused);
    harness.send(Command::Load(media.clone())).unwrap();
    let replacement = harness.wait(|state| {
        state.generation > playing.generation
            && state.phase == Phase::Paused
            && state.paused
            && state.position.is_some()
    });
    harness
        .send(Command::OpenFiles(vec![media.clone(), media.clone()]))
        .unwrap();
    let batch = harness.wait(|state| {
        state.generation > replacement.generation
            && state.phase == Phase::Paused
            && state.paused
            && state.playlist.len() == 2
    });
    harness
        .send(Command::AppendFiles(vec![media.clone()]))
        .unwrap();
    let appended = harness.wait(|state| state.playlist.len() == 3);
    assert_eq!(appended.generation, batch.generation);
    assert!(appended.paused);
    harness.send(Command::SetPauseWhenOpen(false)).unwrap();
    harness.wait(|state| !state.pause_when_open && state.paused);
    harness.send(Command::Load(media)).unwrap();
    harness.wait(|state| {
        state.generation > batch.generation
            && state.phase == Phase::Playing
            && !state.paused
            && state.position.is_some()
    });
}

#[test]
#[ignore = "requires provisioned libmpv; verifies native watch-later save and restore policy"]
fn resume_last_position_saves_on_stop_and_restores_only_when_enabled() {
    let root = fixture_root();
    let media = root
        .join("tests/fixtures/catalog.mkv")
        .to_string_lossy()
        .into_owned();
    let scratch = std::env::temp_dir().join(format!(
        "riina-resume-native-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let enabled_watch_later = scratch.join("enabled");
    let disabled_watch_later = scratch.join("disabled");
    std::fs::create_dir_all(&enabled_watch_later).unwrap();
    std::fs::create_dir_all(&disabled_watch_later).unwrap();

    let options = |directory: &std::path::Path, enabled: bool| {
        let value = if enabled { "yes" } else { "no" }.to_owned();
        vec![
            (
                "watch-later-directory".to_owned(),
                directory.to_string_lossy().into_owned(),
            ),
            ("save-position-on-quit".to_owned(), value.clone()),
            ("resume-playback".to_owned(), value),
        ]
    };

    let Harness { player, receiver } =
        Harness::with_options(&root, options(&enabled_watch_later, true)).unwrap();
    player.send(Command::Load(media.clone())).unwrap();
    let saved = wait_for(&receiver, |state| {
        state.phase == Phase::Playing && state.position.is_some_and(|position| position >= 2.0)
    });
    player.send(Command::Stop).unwrap();
    wait_for(&receiver, |state| {
        state.phase == Phase::Stopped && state.save_position_on_quit == Some(true)
    });
    let saved_entries: Vec<_> = std::fs::read_dir(&enabled_watch_later)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert!(
        !saved_entries.is_empty(),
        "explicit Stop must write a watch-later file before the player is dropped"
    );
    drop(player);

    let Harness { player, receiver } =
        Harness::with_options(&root, options(&enabled_watch_later, true)).unwrap();
    player.send(Command::Load(media.clone())).unwrap();
    let restored = wait_for(&receiver, |state| {
        state.phase == Phase::Playing
            && state
                .position
                .is_some_and(|position| position >= saved.position.unwrap() - 0.5)
    });
    assert!(restored.position.unwrap() >= saved.position.unwrap() - 0.5);
    drop(player);

    let Harness { player, receiver } =
        Harness::with_options(&root, options(&disabled_watch_later, false)).unwrap();
    player.send(Command::Load(media.clone())).unwrap();
    wait_for(&receiver, |state| {
        state.phase == Phase::Playing && state.position.is_some_and(|position| position >= 1.0)
    });
    player.send(Command::Stop).unwrap();
    wait_for(&receiver, |state| {
        state.phase == Phase::Stopped && state.save_position_on_quit == Some(false)
    });
    assert!(
        std::fs::read_dir(&disabled_watch_later)
            .unwrap()
            .next()
            .is_none(),
        "explicit Stop must not create a watch-later file when resume is disabled"
    );
    drop(player);
    assert!(
        std::fs::read_dir(&disabled_watch_later)
            .unwrap()
            .next()
            .is_none(),
        "disabled resume must not create a watch-later file"
    );

    let Harness { player, receiver } =
        Harness::with_options(&root, options(&disabled_watch_later, false)).unwrap();
    player.send(Command::Load(media)).unwrap();
    let from_start = wait_for(&receiver, |state| {
        state.phase == Phase::Playing && state.position.is_some()
    });
    assert!(
        from_start.position.unwrap() < 0.75,
        "disabled resume must start from the beginning: {from_start:?}"
    );
    drop(player);
    std::fs::remove_dir_all(scratch).unwrap();
}

#[test]
#[ignore = "requires provisioned libmpv; verifies dropped subtitles attach after media loads"]
fn opening_dropped_media_with_subtitles_attaches_them_to_the_new_file() {
    let root = fixture_root();
    let media = root.join("tests/fixtures/catalog.mkv");
    let subtitle = root.join("tests/fixtures/manual-captions.srt");
    let harness = Harness::start(&root).unwrap();
    harness
        .send(Command::OpenFilesWithSubtitles {
            paths: vec![media.to_string_lossy().into_owned()],
            subtitles: vec![subtitle.to_string_lossy().into_owned()],
        })
        .unwrap();
    let loaded = harness.wait(|state| {
        state.error.is_some()
            || state.tracks.iter().any(|track| {
                track.kind == TrackKind::Sub
                    && track.external
                    && track.title.as_deref() == Some("manual-captions.srt")
            })
    });
    assert_eq!(loaded.error, None);
    assert!(loaded.tracks.iter().any(|track| {
        track.kind == TrackKind::Sub
            && track.external
            && track
                .external_filename
                .as_deref()
                .is_some_and(|filename| filename.ends_with("manual-captions.srt"))
    }));
}

#[test]
#[ignore = "requires provisioned libmpv; exercises the UI's multi-file open command"]
fn ui_open_files_command_starts_real_media() {
    let root = fixture_root();
    let harness = Harness::start(&root)
        .expect("Provision libmpv and set RIINA_TEST_MPV_LIBRARY to its absolute path");
    harness
        .send(Command::OpenFiles(vec![
            root.join("tests/fixtures/moving-square.avi")
                .to_string_lossy()
                .into_owned(),
        ]))
        .unwrap();

    let playing = harness.wait(|state| {
        state.phase == Phase::Playing && state.position.is_some_and(|position| position > 0.2)
    });
    assert!(
        playing
            .duration
            .is_some_and(|duration| (duration - 5.0).abs() < 0.1)
    );
    assert!(playing.audio_output.is_some(), "Audio output did not start");
    assert!(
        playing
            .tracks
            .iter()
            .any(|track| track.kind == TrackKind::Video)
    );
    assert_eq!(playing.error, None);

    let overridden = riina_core::hdr_preferences::Settings {
        target_peak_override_enabled: true,
        target_peak_nits: 1000,
        algorithm: riina_core::hdr_preferences::Algorithm::Bt2390,
        param_override_enabled: true,
        param: -0.25,
        ..Default::default()
    };
    harness.send(Command::VideoThreads(4)).unwrap();
    harness.send(Command::SetToneMapping(overridden)).unwrap();
    harness
        .send(Command::SetToneMapping(
            riina_core::hdr_preferences::Settings {
                enabled: false,
                ..Default::default()
            },
        ))
        .unwrap();
    let tone_mapping_updated = harness.wait(|state| {
        state.error.is_some()
            || state
                .position
                .is_some_and(|position| position > playing.position.unwrap_or_default() + 0.5)
    });
    assert_eq!(tone_mapping_updated.error, None);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies an invalid optional setting does not block playback"]
fn invalid_optional_startup_option_keeps_media_playable() {
    let root = fixture_root();
    let Harness { player, receiver } =
        Harness::with_options(&root, vec![("riina-invalid-option".into(), "value".into())])
            .expect("invalid user options must not fail player initialization");
    let warning = wait_for(&receiver, |state| {
        state
            .error
            .as_deref()
            .is_some_and(|error| error.contains("riina-invalid-option"))
    });
    player
        .send(Command::Load(
            root.join("tests/fixtures/moving-square.avi")
                .to_str()
                .unwrap()
                .into(),
        ))
        .unwrap();
    let playing = wait_for(&receiver, |state| {
        state.phase == Phase::Playing && state.position.is_some_and(|position| position > 0.2)
    });
    assert!(playing.audio_output.is_some());
    assert!(playing.error.is_none());
    assert!(warning.error.unwrap().contains("riina-invalid-option"));
}
