use super::*;
use riina_core::PlaylistEntry;

fn temporary_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "riina-history-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn entries() -> Vec<riina_core::playback_history::Entry> {
    ["first", "second", "third"]
        .into_iter()
        .map(|name| riina_core::playback_history::Entry {
            path: format!("https://media.example/{name}.mkv"),
            date_ms: 1,
            progress: None,
            duration: 60.0,
        })
        .collect()
}

#[test]
fn selected_history_rows_are_removed_in_one_persisted_change() {
    let path = temporary_path("batch");
    let original = entries();
    let mut history =
        PlaybackHistoryState::with_persistence(original.clone(), Some(path.clone()), None);
    let revision = history.revision();
    assert_eq!(
        history
            .remove_paths([
                original[0].path.as_str(),
                original[2].path.as_str(),
                original[0].path.as_str(),
                "missing"
            ])
            .unwrap(),
        2
    );
    assert_eq!(history.revision(), revision + 1);
    assert_eq!(history.entries(), &original[1..2]);
    assert_eq!(
        crate::playback_history_store::load(&path, &[]).unwrap(),
        history.entries()
    );
    assert_eq!(
        history.pending_progress_paths,
        HashSet::from([original[1].path.clone()])
    );
    assert_eq!(
        history.remove_paths([original[0].path.as_str()]).unwrap(),
        0
    );
    assert_eq!(history.revision(), revision + 1);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn a_failed_history_batch_keeps_every_row_and_revision() {
    let blocked_parent = temporary_path("blocked");
    std::fs::write(&blocked_parent, "not a directory").unwrap();
    let original = entries();
    let mut history = PlaybackHistoryState::with_persistence(
        original.clone(),
        Some(blocked_parent.join("history.json")),
        None,
    );
    let revision = history.revision();
    let pending = history.pending_progress_paths.clone();
    assert!(
        history
            .remove_paths(original.iter().map(|entry| entry.path.as_str()))
            .is_err()
    );
    assert_eq!(history.entries(), original);
    assert_eq!(history.revision(), revision);
    assert_eq!(history.pending_progress_paths, pending);
    std::fs::remove_file(blocked_parent).unwrap();
}

#[test]
fn playback_history_records_a_loaded_media_once_per_generation() {
    let mut state = PlaybackHistoryState::default();
    let media = "https://media.example/sample.mp4";
    let mut playback = Playback {
        generation: 1,
        phase: Phase::Playing,
        position: Some(15.0),
        duration: Some(80.0),
        ..Playback::default()
    };
    playback.playlist.push(PlaylistEntry {
        id: 1,
        filename: media.into(),
        title: None,
        current: true,
        playing: true,
    });

    state.observe(1, &playback, true);
    let first = state.entries().first().unwrap().clone();
    assert_eq!(first.path, media);
    assert_eq!(first.progress, None);
    assert_eq!(first.duration, 80.0);
    assert!(first.date_ms > 0);

    playback.position = Some(42.0);
    state.observe(1, &playback, true);
    assert_eq!(state.entries(), std::slice::from_ref(&first));

    playback.generation = 2;
    state.observe(1, &playback, true);
    assert_eq!(state.entries().len(), 1);
    assert_eq!(state.entries()[0].path, media);
    assert!(state.entries()[0].date_ms >= first.date_ms);
}

#[test]
fn playback_history_is_saved_when_a_media_generation_is_first_observed() {
    let path = std::env::temp_dir().join(format!(
        "riina-history-observed-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut state = PlaybackHistoryState::with_persistence(Vec::new(), Some(path.clone()), None);
    let mut playback = Playback {
        generation: 1,
        phase: Phase::Playing,
        duration: Some(90.0),
        ..Playback::default()
    };
    playback.playlist.push(PlaylistEntry {
        id: 1,
        filename: "C:/media/movie.mkv".into(),
        title: None,
        current: true,
        playing: true,
    });

    state.observe(1, &playback, true);

    let saved = crate::playback_history_store::load(&path, &[]).unwrap();
    assert_eq!(saved, state.entries());
    assert_eq!(saved.len(), 1);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn playback_history_fills_late_duration_without_resetting_visit_or_progress() {
    let path = std::env::temp_dir().join(format!("riina-history-late-{}.json", std::process::id()));
    let mut history = PlaybackHistoryState::with_persistence(Vec::new(), Some(path.clone()), None);
    let mut playback = Playback {
        generation: 1,
        phase: Phase::Paused,
        playlist: vec![PlaylistEntry {
            id: 1,
            filename: "C:/media/late.wav".into(),
            title: None,
            current: true,
            playing: true,
        }],
        ..Playback::default()
    };
    history.observe(1, &playback, true);
    assert_eq!(history.entries()[0].duration, 0.0);
    history.entries[0].progress = Some(30.0);
    let date = history.entries()[0].date_ms;
    let revision = history.revision();
    playback.duration = Some(60.0);
    history.observe(1, &playback, true);
    assert_eq!(history.entries().len(), 1);
    assert_eq!(history.entries()[0].duration, 60.0);
    assert_eq!(history.entries()[0].progress, Some(30.0));
    assert_eq!(history.entries()[0].date_ms, date);
    assert_eq!(history.revision(), revision + 1);
    assert_eq!(
        crate::playback_history_store::load(&path, &[]).unwrap(),
        history.entries()
    );
    for duration in [60.0, 0.0, -1.0, f64::NAN, f64::INFINITY] {
        playback.duration = Some(duration);
        history.observe(1, &playback, true);
        assert_eq!(history.revision(), revision + 1);
        assert_eq!(history.entries()[0].duration, 60.0);
    }
    playback.duration = Some(90.0);
    history.observe(1, &playback, false);
    assert_eq!(history.revision(), revision + 1);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn restored_playback_history_reads_progress_from_mpv_watch_later() {
    let root = std::env::temp_dir().join(format!(
        "riina-history-watch-later-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let watch_later = root.join("watch-later");
    std::fs::create_dir_all(&watch_later).unwrap();
    let archive_path = root.join("playback-history.json");
    let media = "https://media.example/movie.mkv";
    let entry = riina_core::playback_history::Entry {
        path: media.into(),
        date_ms: 1,
        progress: None,
        duration: 90.0,
    };
    crate::playback_history_store::save(&archive_path, std::slice::from_ref(&entry)).unwrap();
    let key = crate::playback_history_store::md5_hex(media.as_bytes());
    std::fs::write(watch_later.join(key), "start=42.25\n").unwrap();
    let mut state = PlaybackHistoryState::with_persistence(
        vec![entry],
        Some(archive_path.clone()),
        Some(watch_later),
    );
    let mut playback = Playback {
        generation: 1,
        phase: Phase::Playing,
        ignore_path_in_watch_later_config: Some(false),
        ..Playback::default()
    };
    playback.playlist.push(PlaylistEntry {
        id: 1,
        filename: media.into(),
        title: None,
        current: true,
        playing: true,
    });

    state.observe(1, &playback, false);

    assert_eq!(state.entries()[0].progress, Some(42.25));
    assert_eq!(
        crate::playback_history_store::load(&archive_path, &[]).unwrap()[0].progress,
        Some(42.25)
    );
    std::fs::remove_dir_all(root).unwrap();
}
