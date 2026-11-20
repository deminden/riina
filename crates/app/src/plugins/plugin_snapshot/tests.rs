use super::*;
use crate::plugin_files::PluginFiles;
use riina_core::{Chapter, PlaylistEntry, Track};

#[test]
fn playback_observation_releases_history_before_waiting_for_recent_media() {
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };

    let recent = Arc::new(Mutex::new(Vec::new()));
    let history = Arc::new(Mutex::new(
        crate::playback_history::PlaybackHistoryState::default(),
    ));
    let recent_guard = recent.lock().unwrap();
    let worker_recent = recent.clone();
    let worker_history = history.clone();
    let worker = std::thread::spawn(move || {
        let playback = Playback {
            generation: 1,
            phase: Phase::Playing,
            playlist: vec![PlaylistEntry {
                id: 1,
                filename: "https://media.example/observed.mkv".into(),
                title: None,
                current: true,
                playing: true,
            }],
            ..Playback::default()
        };
        observe_playback(&playback, &worker_recent, &worker_history, true, 1)
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut history_available = false;
    while Instant::now() < deadline {
        if let Ok(state) = history.try_lock()
            && state.entries().len() == 1
        {
            history_available = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    drop(recent_guard);
    let (context, error) = worker.join().unwrap();
    assert!(
        history_available,
        "History remained coupled to the locked recent-media list"
    );
    assert!(error.is_none());
    let files = PluginFiles::temporary("org.example.observed-history");
    files.set_player_context(context);
    assert_eq!(files.core_playback_history().unwrap().len(), 1);
}

#[test]
fn playback_history_api_values_use_iina_field_names() {
    let entries = playback_history_values(&[riina_core::playback_history::Entry {
        path: "https://media.example/sample.mp4".into(),
        date_ms: 1_750_000_000_000,
        progress: Some(12.5),
        duration: 80.0,
    }]);
    assert_eq!(
        entries,
        [serde_json::json!({
            "name": "sample.mp4",
            "url": "https://media.example/sample.mp4",
            "date": 1_750_000_000_000_u64,
            "progress": 12.5,
            "duration": 80.0
        })]
    );
}

#[test]
fn recent_documents_preserve_recency_names_and_absolute_urls() {
    let path = std::env::temp_dir().join("Riina Recent Clip.mp4");
    let expected_url = url::Url::from_file_path(&path).unwrap().to_string();
    let entries = recent_document_values(&[
        riina_core::RecentMedia {
            path: path.to_string_lossy().into_owned(),
            title: "Recent Clip".into(),
            position: 0.0,
        },
        riina_core::RecentMedia {
            path: "https://media.example/second.mkv".into(),
            title: "Second".into(),
            position: 0.0,
        },
    ]);
    assert_eq!(
        entries[0],
        serde_json::json!({"name":"Riina Recent Clip.mp4","url":expected_url})
    );
    assert_eq!(
        entries[1],
        serde_json::json!({"name":"second.mkv","url":"https://media.example/second.mkv"})
    );
}

#[test]
fn playback_snapshots_supply_core_status_and_chapters() {
    let mut playback = Playback {
        phase: Phase::Playing,
        mpv_version: Some("mpv 0.41.0-git".into()),
        position: Some(12.5),
        duration: Some(80.0),
        paused: true,
        speed: 1.5,
        title: "Sample title".into(),
        volume: 82.0,
        audio_delay: 0.125,
        subtitle_delay: -0.25,
        muted: true,
        tracks: vec![Track {
            id: 4,
            kind: TrackKind::Audio,
            title: Some("Commentary".into()),
            lang: Some("en".into()),
            codec: Some("aac".into()),
            selected: true,
            main_selection: Some(0),
            external: false,
            albumart: false,
            external_filename: None,
            is_default: true,
            forced: false,
            channels: Some(2),
            channel_layout: Some("stereo".into()),
            sample_rate: Some(48_000),
            width: None,
            height: None,
            frame_rate: None,
        }],
        ..Playback::default()
    };
    playback.playlist.push(PlaylistEntry {
        id: 1,
        filename: "https://media.example/sample.mp4".into(),
        title: None,
        current: true,
        playing: true,
    });
    playback.chapters.push(Chapter {
        time: 0.0,
        title: Some("Opening".into()),
    });
    let files = PluginFiles::temporary("org.example.snapshot-source");
    files.set_player_context(player_file_context(&playback, &mut Vec::new(), &[]));
    let version = files.core_version().unwrap();
    assert_eq!(version["iina"], env!("CARGO_PKG_VERSION"));
    assert!(
        version["build"]
            .as_str()
            .is_some_and(|build| !build.is_empty())
    );
    assert_eq!(version["mpv"], "mpv 0.41.0-git");
    assert_eq!(
        files.core_recent_documents().unwrap(),
        [serde_json::json!({
            "name": "sample.mp4",
            "url": "https://media.example/sample.mp4"
        })]
    );

    assert_eq!(
        files.core_status_property("paused").unwrap(),
        Some(serde_json::json!(true))
    );
    assert_eq!(
        files.core_status_property("position").unwrap(),
        Some(serde_json::json!(12.5))
    );
    assert_eq!(
        files.core_status_property("duration").unwrap(),
        Some(serde_json::json!(80.0))
    );
    assert_eq!(
        files.core_status_property("isNetworkResource").unwrap(),
        Some(serde_json::json!(true))
    );
    assert_eq!(
        files.core_status_property("idle").unwrap(),
        Some(serde_json::json!(false))
    );
    assert_eq!(
        files.core_status_property("audio.id").unwrap(),
        Some(serde_json::json!(4))
    );
    assert_eq!(
        files.core_status_property("audio.delay").unwrap(),
        Some(serde_json::json!(0.125))
    );
    assert_eq!(
        files.core_status_property("audio.volume").unwrap(),
        Some(serde_json::json!(82.0))
    );
    assert_eq!(
        files.core_status_property("audio.muted").unwrap(),
        Some(serde_json::json!(true))
    );
    assert_eq!(
        files.core_status_property("audio.currentTrack").unwrap(),
        Some(serde_json::json!({
            "id": 4,
            "title": "Commentary",
            "formattedTitle": "#4 [English] Commentary (aac, 2ch, 48kHz, Default)",
            "lang": "en",
            "codec": "aac",
            "isDefault": true,
            "isForced": false,
            "isSelected": true,
            "isExternal": false,
            "demuxW": null,
            "demuxH": null,
            "demuxChannelCount": 2,
            "demuxChannels": "stereo",
            "demuxSamplerate": 48000,
            "demuxFPS": null,
        }))
    );
    assert_eq!(
        files.core_status_property("audio.tracks").unwrap(),
        Some(serde_json::json!([{
            "id": 4,
            "title": "Commentary",
            "formattedTitle": "#4 [English] Commentary (aac, 2ch, 48kHz, Default)",
            "lang": "en",
            "codec": "aac",
            "isDefault": true,
            "isForced": false,
            "isSelected": true,
            "isExternal": false,
            "demuxW": null,
            "demuxH": null,
            "demuxChannelCount": 2,
            "demuxChannels": "stereo",
            "demuxSamplerate": 48000,
            "demuxFPS": null,
        }]))
    );
    assert_eq!(
        files.core_chapters().unwrap(),
        [serde_json::json!({"title":"Opening", "start":0.0})]
    );
}
