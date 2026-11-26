use super::*;

#[test]
#[ignore = "requires provisioned libmpv; verifies media-scoped chapter actions"]
fn chapter_actions_resume_and_only_menu_selection_confirms() {
    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    harness.send(Command::SetPauseWhenOpen(true)).unwrap();
    let fixture = root.join("tests/fixtures/catalog.mkv");
    harness
        .send(Command::Load(fixture.to_string_lossy().into_owned()))
        .unwrap();
    let loaded = harness.wait(|state| state.paused && state.chapters.len() == 2);
    let select = |index, start, notify| Command::ActivateChapter {
        index,
        start,
        notify,
    };
    harness
        .send(select(1, loaded.chapters[1].time + 1.0, true))
        .unwrap();
    harness.send(Command::SubtitleDelay(0.25)).unwrap();
    let stale = harness.wait(|state| (state.subtitle_delay - 0.25).abs() < 0.001);
    assert!(stale.paused);
    assert!(stale.chapter_feedback.is_none());
    assert_eq!(stale.error(), None);

    harness
        .send(select(1, loaded.chapters[1].time, false))
        .unwrap();
    let sidebar = harness.wait(|state| !state.paused && state.current_chapter_index() == Some(1));
    assert!(sidebar.chapter_feedback.is_none());
    harness.send(Command::SetPaused(true)).unwrap();
    harness.wait(|state| state.paused);
    harness
        .send(select(0, loaded.chapters[0].time, true))
        .unwrap();
    let menu = harness.wait(|state| {
        !state.paused
            && state.current_chapter_index() == Some(0)
            && state.chapter_feedback.is_some()
    });
    assert_eq!(
        menu.chapter_feedback.as_ref().unwrap().title.as_ref(),
        "Opening"
    );

    harness
        .send(Command::Load(fixture.to_string_lossy().into_owned()))
        .unwrap();
    let next =
        harness.wait(|state| state.generation > loaded.generation && state.chapters.len() == 2);
    assert!(next.chapter_feedback.is_none());
}

#[test]
#[ignore = "requires provisioned libmpv; checks silent playlist duration probing"]
fn playlist_duration_probe_reads_local_video_and_audio() {
    let root = fixture_root();
    let library = test_library(&root);
    let fixtures = root.join("tests/fixtures");
    let video = fixtures.join("moving-square.avi");
    let audio = fixtures.join("external-tone.wav");
    assert!(
        riina_mpv::read_local_duration(&library, &video, || false)
            .is_some_and(|duration| (duration - 5.0).abs() < 0.1)
    );
    assert!(
        riina_mpv::read_local_duration(&library, &audio, || false)
            .is_some_and(|duration| duration > 0.5)
    );
    assert_eq!(
        riina_mpv::read_local_duration(&library, &video, || true),
        None
    );
    assert_eq!(
        riina_mpv::read_local_duration(&library, &fixtures.join("missing.avi"), || false),
        None
    );
}

#[test]
#[ignore = "requires provisioned libmpv; verifies a stable playlist move during playback"]
fn stable_playlist_move_resolves_both_rows_after_an_insertion() {
    use riina_core::PlaylistChange;
    use std::num::NonZeroUsize;
    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    let path = |name| {
        root.join("tests/fixtures")
            .join(name)
            .to_string_lossy()
            .into_owned()
    };
    harness
        .send(Command::Load(path("moving-square.avi")))
        .unwrap();
    harness.wait(|state| state.phase == Phase::Playing);
    harness.send(Command::SetPaused(true)).unwrap();
    let sender = harness.player.command_sender();
    let two_added = PlaylistChange::Added(NonZeroUsize::new(2).unwrap());
    sender
        .send_playlist_edit(Command::AppendFiles(vec![
            path("catalog.mkv"),
            path("external-tone.wav"),
        ]))
        .unwrap();
    let loaded = harness.wait(|state| {
        state.paused
            && state.playlist.len() == 3
            && state
                .playlist_feedback
                .is_some_and(|feedback| feedback.change == two_added)
    });
    let appended = loaded.playlist_feedback.unwrap();
    assert_eq!(appended.generation, loaded.generation);
    let ids = [
        loaded.playlist[0].id,
        loaded.playlist[1].id,
        loaded.playlist[2].id,
    ];
    sender
        .send_playlist_edit(Command::InsertPlaylistFilesBefore {
            paths: vec![path("hdr-hlg.mkv")],
            before_id: Some(ids[1]),
        })
        .unwrap();
    let inserted = harness.wait(|state| {
        state.playlist.len() == 4
            && state.playlist_feedback.is_some_and(|feedback| {
                feedback.sequence > appended.sequence
                    && feedback.change == PlaylistChange::Added(NonZeroUsize::MIN)
            })
            && std::path::Path::new(&state.playlist[1].filename)
                .file_name()
                .is_some_and(|name| name == "hdr-hlg.mkv")
    });
    let inserted_id = inserted.playlist[1].id;
    harness
        .send(Command::MovePlaylistIdBefore {
            id: ids[0],
            before_id: ids[2],
        })
        .unwrap();
    let moved = harness.wait(|state| {
        state
            .playlist
            .iter()
            .map(|entry| entry.id)
            .eq([inserted_id, ids[1], ids[0], ids[2]])
    });
    assert_eq!(moved.generation, loaded.generation);
    assert!(moved.paused && moved.playlist[2].current);
    assert_eq!(moved.error(), None);

    harness
        .send(Command::QueuePlaylistIdsNext(vec![
            ids[0],
            ids[2],
            inserted_id,
        ]))
        .unwrap();
    let queued = harness.wait(|state| {
        state
            .playlist
            .iter()
            .map(|entry| entry.id)
            .eq([ids[1], ids[0], inserted_id, ids[2]])
    });
    assert_eq!(queued.generation, loaded.generation);
    assert!(queued.paused && queued.playlist[1].current);

    harness
        .send(Command::MovePlaylistIdsBefore {
            ids: vec![ids[2], ids[1]],
            before_id: Some(inserted_id),
        })
        .unwrap();
    let dropped = harness.wait(|state| {
        state
            .playlist
            .iter()
            .map(|entry| entry.id)
            .eq([ids[0], ids[1], ids[2], inserted_id])
    });
    assert_eq!(dropped.generation, loaded.generation);
    assert!(dropped.paused && dropped.playlist[0].current);

    sender.send_playlist_edit(Command::ClearPlaylist).unwrap();
    let cleared = harness.wait(|state| {
        state.playlist.len() == 1
            && state
                .playlist_feedback
                .is_some_and(|feedback| feedback.change == PlaylistChange::Cleared)
    });
    assert_eq!(cleared.generation, loaded.generation);
    assert_eq!(cleared.position, dropped.position);
    assert_eq!(cleared.playlist[0].id, ids[0]);
    assert!(cleared.paused && cleared.playlist[0].current);
    assert_eq!(cleared.error(), None);
    assert!(!cleared.folder_discovery_allowed);
    sender
        .send_for_media(
            cleared.generation,
            Command::InsertPlaylistFilesAroundCurrent {
                before: vec![path("catalog.mkv")],
                after: vec![path("external-tone.wav")],
            },
        )
        .unwrap();
    sender.send(Command::SetMuted(true)).unwrap();
    let after_delayed_discovery = harness.wait(|state| state.muted);
    assert_eq!(after_delayed_discovery.playlist, cleared.playlist);

    sender
        .send_playlist_edit(Command::InsertPlaylistFilesBefore {
            paths: vec![path("catalog.mkv")],
            before_id: Some(ids[1]),
        })
        .unwrap();
    let rejected = harness.wait(|state| state.error().is_some());
    assert_eq!(rejected.playlist_feedback, cleared.playlist_feedback);
    assert_eq!(rejected.playlist, cleared.playlist);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies Play restarts media after EOF"]
fn play_restarts_media_after_end_of_file() {
    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();

    harness
        .send(Command::Load(
            root.join("tests/fixtures/catalog.mkv")
                .to_string_lossy()
                .into_owned(),
        ))
        .unwrap();
    let ended = harness.wait(|state| state.phase == Phase::Ended);
    assert!(
        ended.video.is_some(),
        "keep-open-on-file-end must retain the final video presentation"
    );
    assert!(
        ended.playlist.iter().any(|entry| entry.current),
        "the ended media must remain the current playlist item"
    );

    harness.send(Command::TogglePause).unwrap();
    let restarted = harness.wait(|state| {
        state.phase == Phase::Playing && !state.paused && state.position.is_some_and(|p| p < 1.0)
    });
    assert_eq!(restarted.error(), None);
}

#[test]
#[ignore = "requires provisioned libmpv; verifies file repeat playback and independent playlist repeat"]
fn file_repeat_wraps_and_preserves_playlist_repeat() {
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
    harness.send(Command::CycleLoop).unwrap();
    harness.wait(|state| state.loop_file.as_deref() == Some("inf"));
    harness.send(Command::CycleLoop).unwrap();
    let playlist_repeat = harness.wait(|state| {
        state.loop_file.as_deref() == Some("no") && state.loop_playlist.as_deref() == Some("inf")
    });
    assert!(playlist_repeat.paused);
    harness.send(Command::ToggleFileLoop).unwrap();
    let both = harness.wait(|state| {
        state.loop_file.as_deref() == Some("inf") && state.loop_playlist.as_deref() == Some("inf")
    });
    assert!(both.paused);
    assert_eq!(both.loop_mode(), riina_core::LoopMode::File);
    harness
        .send(Command::Seek {
            seconds: 4.8,
            absolute: true,
        })
        .unwrap();
    let near_end = harness.wait(|state| {
        state.paused
            && !state.seeking
            && state.completed_seeks > paused.completed_seeks
            && state.position.is_some_and(|p| (4.75..4.95).contains(&p))
    });
    harness.send(Command::TogglePause).unwrap();
    let wrapped = harness.wait(|state| {
        !state.paused
            && state.phase == Phase::Playing
            && state.position.is_some_and(|p| p < 0.5)
            && state.generation == near_end.generation
    });
    assert_eq!(wrapped.error(), None);
    assert_eq!(wrapped.loop_file.as_deref(), Some("inf"));
    assert_eq!(wrapped.loop_playlist.as_deref(), Some("inf"));
    harness.send(Command::TogglePause).unwrap();
    harness.wait(|state| state.paused);
    harness.send(Command::ToggleFileLoop).unwrap();
    let file_off = harness.wait(|state| {
        state.loop_file.as_deref() == Some("no") && state.loop_playlist.as_deref() == Some("inf")
    });
    assert!(file_off.paused);
    assert_eq!(file_off.loop_mode(), riina_core::LoopMode::Playlist);
    assert_eq!(file_off.error(), None);
    harness.send(Command::CycleLoop).unwrap();
    harness.wait(|state| state.loop_mode() == riina_core::LoopMode::Off);
    harness.send(Command::ToggleFileLoop).unwrap();
    let file_only = harness.wait(|state| {
        state.loop_file.as_deref() == Some("inf") && state.loop_playlist.as_deref() == Some("no")
    });
    assert!(file_only.paused);
    harness.send(Command::ToggleFileLoop).unwrap();
    let all_off = harness.wait(|state| {
        state.loop_file.as_deref() == Some("no") && state.loop_playlist.as_deref() == Some("no")
    });
    assert!(all_off.paused);
    assert_eq!(all_off.error(), None);

    // The menu toggles the effective mode. A profile shortcut still owns the
    // raw file-repeat property and can reveal the saved playlist repeat again.
    use riina_core::LoopMode;
    harness
        .send(Command::MpvInputAction("set loop-playlist 2".into()))
        .unwrap();
    harness.wait(|state| {
        state.loop_playlist.as_deref() == Some("2") && state.loop_mode() == LoopMode::Playlist
    });
    harness
        .send(Command::ToggleLoopMode(LoopMode::File))
        .unwrap();
    harness.wait(|state| {
        state.loop_mode() == LoopMode::File && state.loop_playlist.as_deref() == Some("2")
    });
    harness
        .send(Command::ToggleLoopMode(LoopMode::File))
        .unwrap();
    harness.wait(|state| {
        state.loop_mode() == LoopMode::Off && state.loop_playlist.as_deref() == Some("no")
    });
    harness
        .send(Command::ToggleLoopMode(LoopMode::Playlist))
        .unwrap();
    harness.wait(|state| {
        state.loop_mode() == LoopMode::Playlist && state.loop_playlist.as_deref() == Some("inf")
    });
    harness
        .send(Command::ToggleLoopMode(LoopMode::Playlist))
        .unwrap();
    let off = harness.wait(|state| state.loop_mode() == LoopMode::Off);
    assert!(off.paused);
    assert_eq!(off.error(), None);
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
    let harness = Harness::start(&root).unwrap();
    harness.send(Command::Load(fixture("catalog.mkv"))).unwrap();
    harness.wait(|state| state.position.is_some_and(|p| p > 0.1));
    harness.send(Command::TogglePause).unwrap();
    let old = harness.wait(|state| state.phase == Phase::Paused);
    let paths = vec![
        fixture("moving-square.avi"),
        fixture("hdr-hlg.mkv"),
        fixture("external-tone.wav"),
    ];
    harness.send(Command::OpenFiles(paths.clone())).unwrap();
    let opened = harness.wait(|state| {
        state.playlist.len() == 3
            && state.generation > old.generation
            && state.playlist[0].filename == paths[0]
            && state.playlist[0].current
            && state.playlist_feedback.is_some_and(|feedback| {
                matches!(feedback.change, riina_core::PlaylistChange::Added(count) if count.get() == 3)
                    && feedback.generation == state.generation
            })
            && state
                .duration
                .is_some_and(|duration| (duration - 5.0).abs() < 0.1)
            && state.position.is_some_and(|p| p > 0.1)
            && !state.paused
    });
    assert_eq!(opened.error(), None);
    assert_eq!(
        opened
            .playlist
            .iter()
            .map(|entry| &entry.filename)
            .collect::<Vec<_>>(),
        paths.iter().collect::<Vec<_>>()
    );
    harness.send(Command::PlaylistNext).unwrap();
    let next = harness.wait(|state| {
        state.playlist.get(1).is_some_and(|entry| entry.current)
            && state
                .video
                .as_ref()
                .is_some_and(|video| video.gamma.as_deref() == Some("hlg"))
            && state.position.is_some_and(|p| p > 0.1)
    });
    assert_eq!(next.error(), None);
    assert!(!next.paused);
    drop(harness);
}

#[test]
#[ignore = "requires provisioned libmpv; checks startup playlist order"]
fn shuffled_open_starts_at_the_first_shuffled_entry_and_keeps_pause() {
    let root = fixture_root();
    let harness = Harness::start(&root).unwrap();
    let fixture = |name| {
        root.join("tests/fixtures")
            .join(name)
            .to_str()
            .unwrap()
            .trim_start_matches(r"\\?\")
            .to_owned()
    };
    let mut paths = ["moving-square.avi", "catalog.mkv", "external-tone.wav"].map(fixture);
    harness.send(Command::SetPauseWhenOpen(true)).unwrap();
    harness
        .send(Command::OpenShuffledPlaylist(paths.to_vec()))
        .unwrap();
    let opened = harness.wait(|state| {
        state.phase == Phase::Paused
            && state.playlist_shuffled
            && state.playlist.len() == paths.len()
            && state.playlist[0].current
    });
    assert!(opened.explicit_playlist);
    assert_eq!(opened.error(), None);
    let mut observed = opened
        .playlist
        .iter()
        .map(|entry| entry.filename.clone())
        .collect::<Vec<_>>();
    observed.sort_unstable();
    paths.sort_unstable();
    assert_eq!(observed, paths);
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
    let harness = Harness::start(&root).unwrap();
    harness.send(Command::Load(fixture("catalog.mkv"))).unwrap();
    let loaded = harness.wait(|state| state.phase == Phase::Playing && state.chapters.len() == 2);
    assert_eq!(loaded.chapters[0].title.as_deref(), Some("Opening"));
    assert_eq!(loaded.chapters[1].title.as_deref(), Some("Moving square"));
    assert!((loaded.chapters[1].time - 2.0).abs() < 0.001);
    harness.send(Command::TogglePause).unwrap();
    harness.wait(|state| state.phase == Phase::Paused);
    harness
        .send(Command::AddSubtitle(fixture("manual-captions.srt")))
        .unwrap();
    let subs = harness.wait(|state| {
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
            .is_some_and(|text| text.contains("café") && text.contains("Привет"))
    });
    harness
        .send(Command::AddSubtitle(fixture("manual-captions.srt")))
        .unwrap();
    // Reload can temporarily remove its catalog entry. Delay observation alone
    // orders commands but does not prove the independent catalog has settled.
    // Wait for its selected external entry as well, without a timing sleep.
    harness.send(Command::SubtitleDelay(0.25)).unwrap();
    let reloaded_subs = harness.wait(|state| {
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
    harness.send(Command::ToggleSubtitleVisibility).unwrap();
    harness.wait(|state| !state.subtitle_visible);
    harness.send(Command::ToggleSubtitleVisibility).unwrap();
    harness.wait(|state| state.subtitle_visible);
    harness
        .send(Command::AddAudio(fixture("external-tone.wav")))
        .unwrap();
    harness.wait(|state| {
        state
            .tracks
            .iter()
            .any(|track| track.kind == TrackKind::Audio && track.external && track.selected)
    });
    harness.send(Command::AudioDelay(-0.2)).unwrap();
    harness.wait(|state| (state.audio_delay + 0.2).abs() < 0.001);
    // Native relative changes must accumulate even when no UI snapshots are
    // consumed between commands, and must leave the playback clock paused.
    for _ in 0..12 {
        harness.send(Command::AdjustAudioDelay(0.1)).unwrap();
    }
    let delayed = harness.wait(|state| (state.audio_delay - 1.0).abs() < 0.001);
    assert!(delayed.paused);
    assert_eq!(delayed.error(), None);
    for _ in 0..2 {
        harness.send(Command::AdjustAudioDelay(-0.5)).unwrap();
    }
    let restored = harness.wait(|state| state.audio_delay.abs() < 0.001);
    assert!(restored.paused);
    harness.send(Command::AdjustAudioDelay(-0.1)).unwrap();
    harness.wait(|state| (state.audio_delay + 0.1).abs() < 0.001);
    harness.send(Command::AudioDelay(0.0)).unwrap();
    let reset = harness.wait(|state| state.audio_delay.abs() < 0.001);
    assert!(reset.paused);
    assert_eq!(reset.error(), None);
    harness.send(Command::SelectChapter(1)).unwrap();
    let selected_chapter = harness.wait(|state| {
        !state.paused
            && state
                .position
                .is_some_and(|position| (2.0..2.5).contains(&position))
    });
    harness.send(Command::TogglePause).unwrap();
    harness.wait(|state| state.phase == Phase::Paused);
    harness.send(Command::AdjustChapter(-1)).unwrap();
    let previous_chapter = harness.wait(|state| {
        state.paused
            && !state.seeking
            && state.completed_seeks > selected_chapter.completed_seeks
            && state.position.is_some_and(|p| p < 0.1)
    });
    assert_eq!(previous_chapter.error(), None);
    harness.send(Command::AdjustChapter(1)).unwrap();
    let next_chapter = harness.wait(|state| {
        state.paused
            && !state.seeking
            && state.completed_seeks > previous_chapter.completed_seeks
            && state.position.is_some_and(|p| (p - 2.0).abs() < 0.05)
    });
    assert_eq!(next_chapter.error(), None);
    harness.send(Command::AdjustChapter(-1)).unwrap();
    let first_chapter = harness.wait(|state| {
        state.paused
            && !state.seeking
            && state.completed_seeks > next_chapter.completed_seeks
            && state.position.is_some_and(|p| p < 0.1)
    });
    harness
        .send(Command::Seek {
            seconds: 0.5,
            absolute: true,
        })
        .unwrap();
    let inside_first = harness.wait(|state| {
        state.paused
            && !state.seeking
            && state.completed_seeks > first_chapter.completed_seeks
            && state.position.is_some_and(|p| (p - 0.5).abs() < 0.05)
    });
    harness.send(Command::AdjustChapter(-1)).unwrap();
    let boundary = harness.wait(|state| {
        state.paused
            && !state.seeking
            && state.completed_seeks > inside_first.completed_seeks
            && state.position.is_some_and(|p| p < 0.1)
    });
    assert_eq!(boundary.generation, loaded.generation);
    assert_eq!(boundary.error(), None);
    harness
        .send(Command::AppendFiles(vec![
            fixture("moving-square.avi"),
            fixture("external-tone.wav"),
        ]))
        .unwrap();
    let appended = harness.wait(|state| state.playlist.len() == 3);
    assert_eq!(
        appended.generation, loaded.generation,
        "Appending must not replace playback"
    );
    let avi_id = appended.playlist[1].id;
    let wav_id = appended.playlist[2].id;
    harness
        .send(Command::AppendFiles(vec![fixture("external-tone.wav")]))
        .unwrap();
    let expanded = harness.wait(|state| state.playlist.len() == 4);
    let trailing_wav_id = expanded.playlist[3].id;
    harness
        .send(Command::MovePlaylistId {
            id: wav_id,
            direction: -1,
        })
        .unwrap();
    harness.wait(|state| state.playlist.len() == 4 && state.playlist[2].id == avi_id);
    harness
        .send(Command::MovePlaylistId {
            id: wav_id,
            direction: 1,
        })
        .unwrap();
    harness.wait(|state| state.playlist.len() == 4 && state.playlist[1].id == avi_id);
    harness
        .send(Command::MovePlaylistId {
            id: wav_id,
            direction: -1,
        })
        .unwrap();
    harness.wait(|state| state.playlist.len() == 4 && state.playlist[2].id == avi_id);
    harness
        .send(Command::RemovePlaylistIds(vec![trailing_wav_id, wav_id]))
        .unwrap();
    harness.wait(|state| state.playlist.len() == 2 && state.playlist[1].id == avi_id);
    harness.send(Command::SelectPlaylistId(avi_id)).unwrap();
    let advanced = harness.wait(|state| {
        state.generation > loaded.generation
            && state.phase == Phase::Playing
            && !state.paused
            && state
                .playlist
                .iter()
                .any(|entry| entry.id == avi_id && entry.current)
    });
    harness.send(Command::SelectPlaylistEntry(1)).unwrap();
    let same_entry = harness.wait(|state| {
        state.generation > advanced.generation && state.phase == Phase::Playing && !state.paused
    });
    assert!(same_entry.position.is_some_and(|position| position < 1.0));
    harness.send(Command::CycleLoop).unwrap();
    harness.wait(|state| state.loop_mode() == riina_core::LoopMode::File);
    harness.send(Command::CycleLoop).unwrap();
    harness.wait(|state| state.loop_mode() == riina_core::LoopMode::Playlist);
    harness.send(Command::CycleLoop).unwrap();
    harness.wait(|state| state.loop_mode() == riina_core::LoopMode::Off);
    harness.send(Command::ClearPlaylist).unwrap();
    let cleared = harness.wait(|state| state.playlist.len() == 1);
    assert_eq!(
        cleared.playlist[0].id, avi_id,
        "Clear keeps the playing entry, as in IINA"
    );
    assert!(
        cleared.error().is_none(),
        "Unexpected native error: {:?}",
        cleared.error()
    );
    drop(harness);
}
