use crate::Preferences;

#[test]
fn invalid_sections_reset_independently_and_preserve_every_other_value() {
    let too_many_recents = serde_json::to_string(&vec![
        serde_json::json!({"path": "film.mkv", "title": "Film", "position": 10});
        11
    ])
    .unwrap();
    let cases = [
        ("window_resize", "false"),
        ("control_bar", "false"),
        (
            "advanced",
            r#"{"use_config_directory":true,"config_directory":" "}"#,
        ),
        ("audio", "false"),
        ("audio_equalizer", "false"),
        ("video_filters", "false"),
        ("audio_filters", "false"),
        ("hdr_tone_mapping", r#"{"param":1e999}"#),
        ("controls", "false"),
        ("key_bindings", "false"),
        ("liquid_glass", "false"),
        ("subtitle_position", r#"{"position":101}"#),
        ("subtitle_other", r#"{"default_encoding":"bad"}"#),
        ("subtitle_auto_load", r#"{"search_paths":"bad\u0000"}"#),
        ("online_subtitles", r#"{"auto_search_threshold_minutes":0}"#),
        ("subtitle_text_style", r#"{"blur":-1}"#),
        ("picture_in_picture", "false"),
        ("screenshot", r#"{"directory":"bad\u0000"}"#),
        ("network", r#"{"user_agent":"bad\u0000"}"#),
        ("network_cache", r#"{"prefetch_secs":-1}"#),
        ("network_online_videos", "false"),
        ("thumbnail_preview", r#"{"width":0}"#),
        ("recent_media", too_many_recents.as_str()),
        (
            "recent_open_documents",
            r#"[{"path":"","title":"no path","position":0}]"#,
        ),
        (
            "playback_history",
            r#"[{"path":"","date_ms":0,"duration":0}]"#,
        ),
        ("schema_version", "19"),
        ("volume", "1e999"),
        ("volume", "201"),
        ("volume_boost", "0"),
        ("video_threads", "2147483648"),
        ("subtitle_font", r#""bad\u0000""#),
        ("subtitle_font_size", "0"),
        ("control_bar_position_horizontal", "-1"),
        ("control_bar_position_vertical", "2"),
        ("osc_position", "3"),
        ("osd_text_size", "501"),
        ("osd_timeout_secs", "1e18"),
    ];
    for (key, bad_value) in cases {
        let mut valid = serde_json::json!({
            "leading_sidebar_width": 7,
            "trailing_sidebar_width": 800,
            "volume": 149,
            "volume_boost": true,
            "video_threads": 8,
            "dark": false,
            "record_recent_files": false,
            "future_settings": {"value":true},
        });
        valid.as_object_mut().unwrap().remove(key);
        let valid = valid.to_string();
        let expected = Preferences::parse(&valid).unwrap();
        let input = format!(
            r#"{},"{key}":{bad_value}}}"#,
            valid.strip_suffix('}').unwrap()
        );
        assert!(Preferences::parse(&input).is_err(), "{key}: {bad_value}");

        let (prefs, rejected) = Preferences::parse_lenient(&input);
        assert_eq!(rejected.len(), 1, "{key}: {rejected:?}");
        assert_eq!(rejected[0].key, key);
        if key == "schema_version" {
            assert!(rejected[0].reason.contains("19"));
        }
        assert_strict_round_trip(&prefs, &expected);
    }
}

#[test]
fn malformed_documents_use_defaults_and_report_one_document_rejection() {
    for input in [
        "",
        "{",
        r#"{"dark":false,"volume":"#,
        "[]",
        "[1, 2]",
        "null",
    ] {
        let (prefs, rejected) = Preferences::parse_lenient(input);
        assert_eq!(rejected.len(), 1, "{input:?}");
        assert_eq!(rejected[0].key, "$");
        assert_strict_round_trip(&prefs, &Preferences::default());
    }
}

#[test]
fn recovery_keeps_legacy_playback_history_for_migration() {
    let (prefs, rejected) = Preferences::parse_lenient(
        r#"{"volume":"bad","playback_history":[{"path":"film.mkv","date_ms":10,"duration":2}]}"#,
    );
    assert_eq!(rejected.len(), 1);
    assert_eq!(rejected[0].key, "volume");
    assert_eq!(prefs.playback_history[0].path, "film.mkv");
    assert_strict_round_trip(&prefs, &Preferences::parse("{}").unwrap());
}

#[test]
fn invalid_explicit_theme_resets_while_missing_theme_keeps_legacy_dark_bit() {
    let (prefs, rejected) =
        Preferences::parse_lenient(r#"{"dark":false,"theme_material":"custom"}"#);
    assert_eq!(rejected.len(), 1);
    assert_eq!(rejected[0].key, "theme_material");
    assert_eq!(prefs.theme_material, Preferences::default().theme_material);
    assert!(!prefs.dark);
    let expected = Preferences {
        dark: false,
        ..Preferences::default()
    };
    assert_strict_round_trip(&prefs, &expected);

    let (legacy, rejected) = Preferences::parse_lenient(r#"{"dark":false}"#);
    assert!(rejected.is_empty());
    assert_eq!(legacy.theme_material, None);
    assert_strict_round_trip(&legacy, &Preferences::parse(r#"{"dark":false}"#).unwrap());
}

fn assert_strict_round_trip(actual: &Preferences, expected: &Preferences) {
    let serialized = serde_json::to_string_pretty(actual).unwrap();
    assert_eq!(serialized, serde_json::to_string_pretty(expected).unwrap());
    let reparsed = Preferences::parse(&serialized).unwrap();
    assert_eq!(serialized, serde_json::to_string_pretty(&reparsed).unwrap());
}
