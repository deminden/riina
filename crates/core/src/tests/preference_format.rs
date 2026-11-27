use crate::*;

#[test]
fn preference_objects_fill_default_fields_and_reject_positional_settings() {
    let prefs = Preferences::parse(r#"{"pause_when_goes_to_sleep":false}"#).unwrap();
    assert!(prefs.playback_power.prevent_screen_saver);
    assert!(!prefs.playback_power.allow_screen_saver_for_audio);
    assert!(!prefs.pause_when_goes_to_sleep);

    let error = Preferences::parse("[]").err().unwrap();
    assert_eq!(error.key, "$");
    assert_eq!(error.reason, "Expected a JSON object for preferences");
}

fn configured_playback() -> Preferences {
    Preferences {
        video_threads: 8,
        disable_animations: true,
        hdr_tone_mapping: hdr_preferences::Settings {
            enabled: false,
            target_peak_override_enabled: true,
            target_peak_nits: 1200,
            algorithm: hdr_preferences::Algorithm::Bt2390,
            param_override_enabled: true,
            param: -0.25,
        },
        thumbnail_preview: thumbnail_preferences::Settings {
            enabled: false,
            maximum_cache_size_mb: 0,
            enable_for_remote_files: true,
            width: 320,
        },
        ..Preferences::default()
    }
}

fn configured_network_and_recent_media() -> Preferences {
    Preferences {
        network: network_preferences::Connection {
            user_agent: "player agent".into(),
            http_proxy: "http://proxy.example:8080".into(),
            rtsp_transport: network_preferences::RtspTransport::Http,
        },
        network_cache: network_preferences::Cache {
            enabled: false,
            pause_initial: true,
            pause_wait_secs: 2.5,
            prefetch_secs: 12_000.0,
            buffer_size_kib: 512_000.0,
            show_buffering_throbber: false,
            show_seeking_throbber: false,
        },
        recent_media: vec![RecentMedia {
            path: r"C:\Фильмы\a.mkv".into(),
            title: "A film".into(),
            position: 87.25,
        }],
        ..Preferences::default()
    }
}

#[test]
fn schema_v1_preferences_keep_their_pretty_serialized_bytes() {
    for (name, settings, expected) in [
        (
            "defaults",
            Preferences::default(),
            include_str!("fixtures/preferences-default-v1.json"),
        ),
        (
            "playback",
            configured_playback(),
            include_str!("fixtures/preferences-playback-v1.json"),
        ),
        (
            "network and recent media",
            configured_network_and_recent_media(),
            include_str!("fixtures/preferences-network-v1.json"),
        ),
    ] {
        let encoded = serde_json::to_string_pretty(&settings).unwrap();
        assert_eq!(encoded, expected, "{name}: serialization");
        let decoded = Preferences::parse(expected).unwrap();
        assert_eq!(
            serde_json::to_string_pretty(&decoded).unwrap(),
            expected,
            "{name}: round trip",
        );
    }
}
