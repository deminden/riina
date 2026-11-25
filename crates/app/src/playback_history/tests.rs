use super::*;
use riina_core::PlaylistEntry;

use riina_test_support::TempDir;

fn entries() -> Vec<riina_core::playback_history::Entry> {
    ["first", "second", "third"]
        .into_iter()
        .map(|name| riina_core::playback_history::Entry {
            path: format!("https://media.example/{name}.mkv"),
            title: None,
            date_ms: 1,
            progress: None,
            duration: 60.0,
        })
        .collect()
}

#[test]
fn selected_history_rows_are_removed_in_one_persisted_change() {
    let temp = TempDir::new("riina-history-batch").unwrap();
    let path = temp.path().join("history.json");
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
        history.remove_paths([original[0].path.as_str()]).unwrap(),
        0
    );
    assert_eq!(history.revision(), revision + 1);
}

#[test]
fn a_failed_history_batch_keeps_every_row_and_revision() {
    let temp = TempDir::new("riina-history-blocked").unwrap();
    let blocked_parent = temp.path().join("blocked");
    std::fs::write(&blocked_parent, "not a directory").unwrap();
    let original = entries();
    let mut history = PlaybackHistoryState::with_persistence(
        original.clone(),
        Some(blocked_parent.join("history.json")),
        None,
    );
    let revision = history.revision();
    assert!(
        history
            .remove_paths(original.iter().map(|entry| entry.path.as_str()))
            .is_err()
    );
    assert_eq!(history.entries(), original);
    assert_eq!(history.revision(), revision);
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
    std::sync::Arc::make_mut(&mut playback.playlist).push(PlaylistEntry {
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
    let temp = TempDir::new("riina-history-observed").unwrap();
    let path = temp.path().join("history.json");
    let mut state = PlaybackHistoryState::with_persistence(Vec::new(), Some(path.clone()), None);
    let mut playback = Playback {
        generation: 1,
        phase: Phase::Playing,
        duration: Some(90.0),
        ..Playback::default()
    };
    std::sync::Arc::make_mut(&mut playback.playlist).push(PlaylistEntry {
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
}

#[test]
fn playback_history_fills_late_metadata_without_resetting_visit_or_progress() {
    let temp = TempDir::new("riina-history-late").unwrap();
    let path = temp.path().join("history.json");
    let mut history = PlaybackHistoryState::with_persistence(Vec::new(), Some(path.clone()), None);
    let mut playback = Playback {
        generation: 1,
        phase: Phase::Paused,
        title: "late.wav".into(),
        playlist: vec![PlaylistEntry {
            id: 1,
            filename: "C:/media/late.wav".into(),
            title: None,
            current: true,
            playing: true,
        }]
        .into(),
        ..Playback::default()
    };
    history.observe(1, &playback, true);
    assert_eq!(history.entries()[0].duration, 0.0);
    assert_eq!(history.entries()[0].title, None);
    history.entries[0].progress = Some(30.0);
    let date = history.entries()[0].date_ms;
    let revision = history.revision();
    playback.duration = Some(60.0);
    playback.title = "L'été à Paris".into();
    history.observe(1, &playback, true);
    assert_eq!(history.entries().len(), 1);
    assert_eq!(history.entries()[0].duration, 60.0);
    assert_eq!(history.entries()[0].title.as_deref(), Some("L'été à Paris"));
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
}
