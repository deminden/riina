use super::*;

#[test]
#[ignore = "requires provisioned libmpv; verifies live pause-on-open policy without pausing current media"]
fn pause_when_open_applies_to_replacement_and_batch_without_changing_current_playback() {
    let root = fixture_root();
    let media = root
        .join("tests/fixtures/catalog.mkv")
        .to_string_lossy()
        .into_owned();
    let audio = root.join("tests/fixtures/external-tone.wav");
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
        .send(Command::OpenPlaylist(vec![media.clone(), media.clone()]))
        .unwrap();
    let batch = harness.wait(|state| {
        state.generation > replacement.generation
            && state.phase == Phase::Paused
            && state.paused
            && state.playlist.len() == 2
    });
    harness
        .send(Command::InsertPlaylistFilesAroundCurrent {
            before: vec![
                root.join("tests/fixtures/moving-square.avi")
                    .to_string_lossy()
                    .into_owned(),
            ],
            after: Vec::new(),
        })
        .unwrap();
    harness
        .send(Command::AppendFiles(vec![
            audio.to_string_lossy().into_owned(),
        ]))
        .unwrap();
    let appended = harness.wait(|state| {
        state
            .playlist
            .iter()
            .any(|entry| std::path::Path::new(&entry.filename).ends_with("external-tone.wav"))
    });
    assert_eq!(appended.generation, batch.generation);
    assert_eq!(appended.playlist.len(), 3);
    assert!(appended.paused);
    assert!(appended.explicit_playlist);
    harness.send(Command::PlaylistNext).unwrap();
    let next = harness.wait(|state| {
        state.generation > appended.generation
            && state.phase == Phase::Paused
            && state.paused
            && state.position.is_some()
            && state.playlist.get(1).is_some_and(|entry| entry.current)
    });
    harness.send(Command::SetPauseWhenOpen(false)).unwrap();
    let updated = harness.wait(|state| !state.pause_when_open);
    assert!(
        updated.paused,
        "Changing the preference resumed the current file"
    );
    harness.send(Command::PlaylistPrevious).unwrap();
    let previous = harness.wait(|state| {
        state.generation > next.generation
            && state.phase == Phase::Playing
            && !state.paused
            && state.playlist.first().is_some_and(|entry| entry.current)
    });
    harness.send(Command::SelectPlaylistEntry(2)).unwrap();
    let audio_playing = harness.wait(|state| {
        state.generation > previous.generation
            && state.phase == Phase::Playing
            && !state.paused
            && state.playlist.get(2).is_some_and(|entry| entry.current)
            && state.video.is_none()
            && state.position.is_some()
    });
    for resumed in [&previous, &audio_playing] {
        assert!(resumed.explicit_playlist);
        assert!(
            resumed
                .mpv_events
                .iter()
                .rev()
                .take_while(|event| event.name.as_ref() != "start-file")
                .any(|event| event.name.as_ref() == "playback-restart")
        );
    }
    harness.send(Command::Load(media)).unwrap();
    let ordinary_open = harness.wait(|state| {
        state.generation > audio_playing.generation
            && state.phase == Phase::Playing
            && !state.paused
    });
    assert!(!ordinary_open.explicit_playlist);
}

#[test]
#[ignore = "requires provisioned libmpv; checks pause policy across automatic playlist transitions"]
fn pause_when_open_also_applies_to_automatic_advancement() {
    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    let paths = ["moving-square.avi", "external-tone.wav", "catalog.mkv"].map(|name| {
        root.join("tests/fixtures")
            .join(name)
            .to_string_lossy()
            .into_owned()
    });
    harness.send(Command::SetPauseWhenOpen(true)).unwrap();
    harness.send(Command::OpenPlaylist(paths.into())).unwrap();
    let first = harness
        .wait(|state| state.phase == Phase::Paused && state.seekable && state.duration.is_some());
    let advance = |current: &Playback| {
        harness
            .send(Command::Seek {
                seconds: current.duration.unwrap() - 0.25,
                absolute: true,
            })
            .unwrap();
        harness.wait(|state| state.completed_seeks > current.completed_seeks && !state.seeking);
        harness.send(Command::SetPaused(false)).unwrap();
    };
    advance(&first);
    let audio = harness.wait(|state| {
        state.generation > first.generation
            && state.seekable
            && state.duration.is_some()
            && state.playlist.get(1).is_some_and(|row| row.current)
            && state
                .mpv_events
                .iter()
                .rev()
                .take_while(|event| event.name.as_ref() != "start-file")
                .any(|event| event.name.as_ref() == "playback-restart")
    });
    assert!(audio.paused);
    assert!(audio.position.is_some_and(|position| position < 0.15));
    assert!(audio.video.is_none());

    harness.send(Command::SetPauseWhenOpen(false)).unwrap();
    assert!(harness.wait(|state| !state.pause_when_open).paused);
    advance(&audio);
    let third = harness.wait(|state| {
        state.generation > audio.generation
            && state.playlist.get(2).is_some_and(|row| row.current)
            && state.position.is_some_and(|position| position > 0.12)
    });
    assert_eq!(third.phase, Phase::Playing);
    assert!(!third.paused);
    assert_eq!(third.error(), None);
}

#[test]
#[ignore = "requires provisioned libmpv; checks folder discovery ownership and playlist edits"]
fn folder_discovery_only_edits_the_entry_created_by_a_single_file_open() {
    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    let commands = harness.player.command_sender();
    let path = |name| {
        root.join("tests/fixtures")
            .join(name)
            .to_str()
            .unwrap()
            .to_owned()
    };
    let current = path("catalog.mkv");
    let before = path("moving-square.avi");
    let audio = path("external-tone.wav");
    let after = path("hdr-hlg.mkv");
    harness.send(Command::SetPauseWhenOpen(true)).unwrap();
    harness
        .send(Command::OpenFiles(vec![current.clone()]))
        .unwrap();
    let initial =
        harness.wait(|state| state.phase == Phase::Paused && state.folder_discovery_allowed);
    harness
        .send(Command::AppendFiles(vec![audio.clone()]))
        .unwrap();
    harness.wait(|state| state.playlist.len() == 2);
    let scan_result = Command::InsertPlaylistFilesAroundCurrent {
        before: vec![audio.clone(), before.clone()],
        after: vec![before.clone(), after.clone()],
    };
    commands
        .send_for_media(initial.generation, scan_result.clone())
        .unwrap();
    let expanded = harness.wait(|state| state.playlist.len() == 4);
    assert!(
        expanded
            .playlist
            .iter()
            .map(|entry| std::path::Path::new(&entry.filename).file_name().unwrap())
            .eq([
                "moving-square.avi",
                "catalog.mkv",
                "external-tone.wav",
                "hdr-hlg.mkv"
            ])
    );
    assert!(expanded.playlist[1].current && expanded.paused);

    harness
        .send(Command::RemovePlaylistId(expanded.playlist[0].id))
        .unwrap();
    let adjusted = harness.wait(|state| state.playlist.len() == 3);
    harness
        .send(Command::SelectPlaylistId(adjusted.playlist[1].id))
        .unwrap();
    let navigated =
        harness.wait(|state| state.generation > initial.generation && state.phase == Phase::Paused);
    assert!(!navigated.folder_discovery_allowed);
    commands
        .send_for_media(initial.generation, scan_result.clone())
        .unwrap();
    commands
        .send_for_media(navigated.generation, scan_result)
        .unwrap();
    harness.send(Command::Volume(59.0)).unwrap();
    let settled = harness.wait(|state| state.volume == 59.0);
    assert_eq!(settled.playlist.len(), 3);
    assert!(settled.playlist[1].current);
    assert_eq!(settled.error(), None);

    harness
        .send(Command::OpenFiles(vec![current, audio]))
        .unwrap();
    let batch = harness
        .wait(|state| state.generation > navigated.generation && state.phase == Phase::Paused);
    assert!(!batch.folder_discovery_allowed);
    assert_eq!(batch.playlist.len(), 2);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies native watch-later save and restore policy"]
fn resume_last_position_saves_on_stop_and_restores_only_when_enabled() {
    let root = fixture_root();
    let media = root
        .join("tests/fixtures/catalog.mkv")
        .to_string_lossy()
        .into_owned();
    let scratch = riina_test_support::TempDir::new("resume-native").unwrap();
    let enabled_watch_later = scratch.path().join("enabled");
    let disabled_watch_later = scratch.path().join("disabled");
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
        state.error().is_some()
            || state.tracks.iter().any(|track| {
                track.kind == TrackKind::Sub
                    && track.external
                    && track.title.as_deref() == Some("manual-captions.srt")
            })
    });
    assert_eq!(loaded.error(), None);
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
    assert_eq!(playing.error(), None);

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
        state.error().is_some()
            || state
                .position
                .is_some_and(|position| position > playing.position.unwrap_or_default() + 0.5)
    });
    assert_eq!(tone_mapping_updated.error(), None);
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
            .error()
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
    assert!(playing.error().is_none());
    assert!(warning.error().unwrap().contains("riina-invalid-option"));
}
