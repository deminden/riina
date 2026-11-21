use super::*;

#[test]
#[ignore = "requires provisioned libmpv; verifies Play restarts media after EOF"]
fn play_restarts_media_after_end_of_file() {
    let root = fixture_root();
    let (sender, receiver) = mpsc::channel();
    let player = start_test_player(&test_library(&root), true, move |state| {
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
    let ended = wait_for(&receiver, |state| state.phase == Phase::Ended);
    assert!(
        ended.video.is_some(),
        "keep-open-on-file-end must retain the final video presentation"
    );
    assert!(
        ended.playlist.iter().any(|entry| entry.current),
        "the ended media must remain the current playlist item"
    );

    player.send(Command::TogglePause).unwrap();
    let restarted = wait_for(&receiver, |state| {
        state.phase == Phase::Playing && !state.paused && state.position.is_some_and(|p| p < 1.0)
    });
    assert_eq!(restarted.error, None);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies file repeat playback and independent playlist repeat"]
fn file_repeat_wraps_and_preserves_playlist_repeat() {
    let root = fixture_root();
    let (sender, receiver) = mpsc::channel();
    let player = start_test_player(&test_library(&root), true, move |state| {
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
#[ignore = "requires provisioned libmpv; verifies ordered multi-file replacement and resume"]
fn multi_file_open_replaces_old_playlist_and_resumes() {
    let root = fixture_root();
    let fixture = |name: &str| {
        root.join("tests/fixtures")
            .join(name)
            .to_str()
            .unwrap()
            .trim_start_matches(r"\\?\")
            .to_owned()
    };
    let (sender, receiver) = mpsc::channel();
    let player = start_test_player(&test_library(&root), true, move |state| {
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

#[test]
#[ignore = "requires provisioned libmpv; exercises real catalog workflows"]
#[expect(
    clippy::too_many_lines,
    reason = "decomposition tracked in docs/review"
)]
fn chapters_external_tracks_and_playlist_editing() {
    let root = fixture_root();
    let fixture = |name: &str| {
        root.join("tests/fixtures")
            .join(name)
            .to_str()
            .unwrap()
            .to_owned()
    };
    let (sender, receiver) = mpsc::channel();
    let player = start_test_player(&test_library(&root), true, move |state| {
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
        .send(Command::AppendFiles(vec![fixture("external-tone.wav")]))
        .unwrap();
    let expanded = wait_for(&receiver, |state| state.playlist.len() == 4);
    let trailing_wav_id = expanded.playlist[3].id;
    player
        .send(Command::MovePlaylistId {
            id: wav_id,
            direction: -1,
        })
        .unwrap();
    wait_for(&receiver, |state| {
        state.playlist.len() == 4 && state.playlist[2].id == avi_id
    });
    player
        .send(Command::MovePlaylistId {
            id: wav_id,
            direction: 1,
        })
        .unwrap();
    wait_for(&receiver, |state| {
        state.playlist.len() == 4 && state.playlist[1].id == avi_id
    });
    player
        .send(Command::MovePlaylistId {
            id: wav_id,
            direction: -1,
        })
        .unwrap();
    wait_for(&receiver, |state| {
        state.playlist.len() == 4 && state.playlist[2].id == avi_id
    });
    player
        .send(Command::RemovePlaylistIds(vec![trailing_wav_id, wav_id]))
        .unwrap();
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
