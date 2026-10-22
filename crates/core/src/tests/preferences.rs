use crate::*;

#[test]
fn unknown_root_preferences_do_not_block_launch_or_playback() {
    // A newer install may have saved additive settings before an older
    // executable is launched again; keep recognized values and defaults.
    let preferences = Preferences::parse(
        r#"{"volume":37,"unlock_window_aspect_ratio":false,"future_setting":{"mode":"auto"}}"#,
    )
    .unwrap();
    assert_eq!(preferences.volume, 37.0);
    assert!(!preferences.unlock_window_aspect_ratio);

    // Forward compatibility must not weaken validation of known controls.
    assert!(Preferences::parse(r#"{"volume":"loud","future_setting":true}"#).is_err());
}

#[test]
fn tone_mapping_preferences_keep_reference_defaults_and_round_trip() {
    let legacy = Preferences::parse("{}").unwrap();
    assert_eq!(
        legacy.hdr_tone_mapping,
        hdr_preferences::Settings::default()
    );

    let selected = Preferences {
        hdr_tone_mapping: hdr_preferences::Settings {
            enabled: false,
            target_peak_override_enabled: true,
            target_peak_nits: 1200,
            algorithm: hdr_preferences::Algorithm::Bt2390,
            param_override_enabled: true,
            param: -0.25,
        },
        ..legacy
    };
    let encoded = serde_json::to_string(&selected).unwrap();
    assert_eq!(
        Preferences::parse(&encoded).unwrap().hdr_tone_mapping,
        selected.hdr_tone_mapping
    );
    assert!(Preferences::parse(r#"{"hdr_tone_mapping":{"enabled":true,"param":1e999}}"#).is_err());
}

#[test]
fn thumbnail_preferences_default_and_round_trip_with_root_settings() {
    let legacy = Preferences::parse("{}").unwrap();
    assert_eq!(
        legacy.thumbnail_preview,
        thumbnail_preferences::Settings::default()
    );
    let selected = Preferences {
        thumbnail_preview: thumbnail_preferences::Settings {
            enabled: false,
            maximum_cache_size_mb: 0,
            enable_for_remote_files: true,
            width: 320,
        },
        ..legacy
    };
    let encoded = serde_json::to_string(&selected).unwrap();
    assert_eq!(
        Preferences::parse(&encoded).unwrap().thumbnail_preview,
        selected.thumbnail_preview
    );
    assert!(Preferences::parse(
        r#"{"thumbnail_preview":{"enabled":true,"maximum_cache_size_mb":500,"enable_for_remote_files":false,"width":0}}"#
    )
    .is_err());
}

#[test]
fn video_decoder_threads_default_to_auto_and_reach_mpv() {
    let defaults = Preferences::parse("{}").unwrap();
    assert_eq!(defaults.video_threads, 0);
    let configured = Preferences {
        video_threads: 8,
        ..defaults
    };
    assert_eq!(
        Preferences::parse(&serde_json::to_string(&configured).unwrap())
            .unwrap()
            .video_threads,
        8
    );
    assert!(Preferences::parse(r#"{"video_threads":2147483648}"#).is_err());
    assert_eq!(
        Command::VideoThreads(8)
            .plan(&Playback::default())
            .unwrap()
            .commands,
        [["set", "vd-lavc-threads", "8"]]
    );
    assert!(
        Command::VideoThreads(i32::MAX as u32 + 1)
            .plan(&Playback::default())
            .is_err()
    );
}

#[test]
fn sidebar_widths_keep_legacy_defaults_and_bound_persisted_sizes() {
    let defaults = Preferences::parse("{}").unwrap();
    assert_eq!(
        (
            defaults.leading_sidebar_width,
            defaults.trailing_sidebar_width
        ),
        (360, 360)
    );
    let bounded =
        Preferences::parse(r#"{"leading_sidebar_width":1,"trailing_sidebar_width":900}"#).unwrap();
    assert_eq!(
        (
            bounded.leading_sidebar_width,
            bounded.trailing_sidebar_width
        ),
        (300, 500)
    );
    let restored = Preferences::parse(&serde_json::to_string(&bounded).unwrap()).unwrap();
    assert_eq!(
        (
            restored.leading_sidebar_width,
            restored.trailing_sidebar_width
        ),
        (300, 500)
    );
    assert!(Preferences::parse(r#"{"leading_sidebar_width":-1}"#).is_err());
}

#[test]
fn layout_preferences_keep_reference_defaults_and_reject_unknown_controller_positions() {
    let legacy = Preferences::parse("{}").unwrap();
    assert!(!legacy.compact_ui);
    assert_eq!(legacy.osc_position, 0);
    assert_eq!(legacy.control_bar_position_horizontal, 0.5);
    assert_eq!(legacy.control_bar_position_vertical, 0.1);
    assert!(!legacy.settings_at_leading);
    assert!(legacy.playlist_at_leading);
    assert!(legacy.plugins_at_leading);
    assert!(legacy.edge_to_edge_video);
    assert!(!legacy.docked_controls);
    assert!(!legacy.unlock_window_aspect_ratio);
    assert!(legacy.dark);
    assert!(!legacy.always_float_on_top);
    assert!(!legacy.always_show_on_top_icon);
    assert_eq!(legacy.window_resize, window_resize::Settings::default());
    assert_eq!(legacy.effective_theme_material(), ThemeMaterial::Dark);
    let selected = Preferences::parse(r#"{"compact_ui":true,"osc_position":2}"#).unwrap();
    let restored = Preferences::parse(&serde_json::to_string(&selected).unwrap()).unwrap();
    assert!(restored.compact_ui);
    let positioned = Preferences::parse(
        r#"{"control_bar_position_horizontal":0.72,"control_bar_position_vertical":0.25}"#,
    )
    .unwrap();
    let restored_positioned =
        Preferences::parse(&serde_json::to_string(&positioned).unwrap()).unwrap();
    assert_eq!(restored_positioned.control_bar_position_horizontal, 0.72);
    assert_eq!(restored_positioned.control_bar_position_vertical, 0.25);
    assert!(Preferences::parse(r#"{"control_bar_position_horizontal":1.1}"#).is_err());
    assert!(Preferences::parse(r#"{"control_bar_position_vertical":-0.1}"#).is_err());
    let light = Preferences::parse(r#"{"dark":false}"#).unwrap();
    assert!(!light.dark);
    assert_eq!(light.effective_theme_material(), ThemeMaterial::Light);
    let system = Preferences::parse(r#"{"theme_material":"system","dark":false}"#).unwrap();
    assert_eq!(system.effective_theme_material(), ThemeMaterial::System);
    let restored_system = Preferences::parse(&serde_json::to_string(&system).unwrap()).unwrap();
    assert_eq!(
        restored_system.effective_theme_material(),
        ThemeMaterial::System
    );
    assert!(Preferences::parse(r#"{"theme_material":"automatic"}"#).is_err());
    let floating = Preferences::parse(r#"{"always_float_on_top":true}"#).unwrap();
    let restored_floating = Preferences::parse(&serde_json::to_string(&floating).unwrap()).unwrap();
    assert!(restored_floating.always_float_on_top);
    let with_pin_indicator = Preferences::parse(r#"{"always_show_on_top_icon":true}"#).unwrap();
    let restored_pin_indicator =
        Preferences::parse(&serde_json::to_string(&with_pin_indicator).unwrap()).unwrap();
    assert!(restored_pin_indicator.always_show_on_top_icon);
    assert_eq!(restored.osc_position, 2);
    assert!(Preferences::parse(r#"{"osc_position":3}"#).is_err());
}

#[test]
fn sidebar_positions_round_trip_without_changing_existing_layout_defaults() {
    let selected =
        Preferences::parse(
            r#"{"settings_at_leading":true,"playlist_at_leading":false,"plugins_at_leading":false,"edge_to_edge_video":false,"docked_controls":true}"#,
        )
        .unwrap();
    let restored = Preferences::parse(&serde_json::to_string(&selected).unwrap()).unwrap();
    assert!(restored.settings_at_leading);
    assert!(!restored.playlist_at_leading);
    assert!(!restored.plugins_at_leading);
    assert!(!restored.edge_to_edge_video);
    assert!(restored.docked_controls);
    assert!(!Preferences::parse("{}").unwrap().settings_at_leading);
    assert!(Preferences::parse("{}").unwrap().playlist_at_leading);
    assert!(Preferences::parse("{}").unwrap().plugins_at_leading);
}

#[test]
fn unlocked_window_aspect_persists_and_defaults_to_iina_locking() {
    let legacy = Preferences::parse("{}").unwrap();
    assert!(!legacy.unlock_window_aspect_ratio);
    let selected = Preferences::parse(r#"{"unlock_window_aspect_ratio":true}"#).unwrap();
    let restored = Preferences::parse(&serde_json::to_string(&selected).unwrap()).unwrap();
    assert!(restored.unlock_window_aspect_ratio);
}

#[test]
fn window_resize_preferences_migrate_and_round_trip() {
    let legacy = Preferences::parse(r#"{"window_resize":{}}"#).unwrap();
    assert_eq!(legacy.window_resize, window_resize::Settings::default());

    let selected = Preferences::parse(
        r#"{"window_resize":{"option":"video_size15","timing":"always","use_physical_resolution":false}}"#,
    )
    .unwrap();
    let restored = Preferences::parse(&serde_json::to_string(&selected).unwrap()).unwrap();
    assert_eq!(restored.window_resize, selected.window_resize);
    assert_eq!(
        restored.window_resize.option,
        window_resize::ResizeOption::VideoSize15
    );
    assert_eq!(
        restored.window_resize.timing,
        window_resize::ResizeTiming::Always
    );
    assert!(!restored.window_resize.use_physical_resolution);
}

#[test]
fn control_preferences_keep_iina_defaults_and_round_trip_validated_values() {
    let legacy = Preferences::parse("{}").unwrap();
    assert_eq!(legacy.controls, control_preferences::Settings::default());

    let selected = Preferences::parse(
        r#"{"controls":{"single_click_action":"pause","vertical_scroll_action":"playback_speed","seek_sensitivity":4}}"#,
    )
    .unwrap();
    assert_eq!(selected.controls.relative_seek_amount, 4);
    let restored = Preferences::parse(&serde_json::to_string(&selected).unwrap()).unwrap();
    assert_eq!(restored.controls, selected.controls);

    assert!(Preferences::parse(r#"{"controls":{"seek_sensitivity":5}}"#).is_err());
    let renamed = Preferences::parse(r#"{"controls":{"relative_seek_amount":2}}"#).unwrap();
    assert_eq!(renamed.controls.relative_seek_amount, 2);
}

#[test]
fn key_binding_preferences_keep_reference_defaults_and_round_trip() {
    let defaults = Preferences::parse("{}").unwrap().key_bindings;
    assert_eq!(defaults, key_bindings::Settings::default());
    let selected = Preferences {
        key_bindings: key_bindings::Settings {
            current_profile: "Cinema".into(),
            use_system_media_controls: false,
            display_raw_values: true,
        },
        ..Preferences::default()
    };
    assert_eq!(
        Preferences::parse(&serde_json::to_string(&selected).unwrap())
            .unwrap()
            .key_bindings,
        selected.key_bindings
    );
    assert!(Preferences::parse(r#"{"key_bindings":{"current_profile":"../outside"}}"#).is_err());
}

#[test]
fn pause_when_open_keeps_legacy_defaults_and_round_trips_enabled_policy() {
    assert!(!Preferences::parse("{}").unwrap().pause_when_open);
    assert!(!Preferences::parse("{}").unwrap().fullscreen_when_open);
    assert!(Preferences::parse("{}").unwrap().resume_last_position);
    let enabled = Preferences::parse(
        r#"{"pause_when_open":true,"fullscreen_when_open":true,"resume_last_position":false}"#,
    )
    .unwrap();
    let saved = serde_json::to_string(&enabled).unwrap();
    assert!(Preferences::parse(&saved).unwrap().pause_when_open);
    assert!(Preferences::parse(&saved).unwrap().fullscreen_when_open);
    assert!(!Preferences::parse(&saved).unwrap().resume_last_position);
}

#[test]
fn hdr_output_preference_defaults_on_and_round_trips() {
    assert!(Preferences::parse("{}").unwrap().hdr_enabled);
    let selected = Preferences {
        hdr_enabled: false,
        ..Preferences::default()
    };
    let saved = serde_json::to_string(&selected).unwrap();
    assert!(!Preferences::parse(&saved).unwrap().hdr_enabled);
}

#[test]
fn legacy_preferences_keep_osd_defaults_and_customization_round_trips() {
    let old = Preferences::parse(
        r#"{"schema_version":1,"volume":37,"volume_boost":false,"dark":true,"compact":false}"#,
    )
    .unwrap();
    assert!(old.enable_osd);
    assert!(!old.disable_osd_file_start);
    assert!(!old.disable_osd_pause_resume);
    assert!(!old.disable_osd_speed);
    assert!(!old.disable_osd_seek);
    assert_eq!(old.osd_timeout_secs, 1.0);
    assert_eq!(old.osd_text_size, 20.0);
    assert!(old.control_bar.auto_hide_enabled);
    assert_eq!(old.control_bar.auto_hide_timeout_secs, 2.5);
    let partial_control_bar =
        Preferences::parse(r#"{"control_bar":{"auto_hide_timeout_secs":1.5}}"#).unwrap();
    assert!(partial_control_bar.control_bar.auto_hide_enabled);
    assert_eq!(partial_control_bar.control_bar.auto_hide_timeout_secs, 1.5);
    assert!(!old.display_time_and_battery_in_fullscreen);
    assert!(!old.live_text_enabled);
    let configured = Preferences {
        enable_osd: false,
        live_text_enabled: true,
        control_bar: osc_preferences::Settings {
            auto_hide_enabled: false,
            auto_hide_timeout_secs: 4.25,
            toolbar_buttons: osc_preferences::Settings::TOOLBAR_PLUGINS
                | osc_preferences::Settings::TOOLBAR_QUICK_SETTINGS,
            toolbar_order: osc_preferences::Settings::default().toolbar_order,
        },
        disable_osd_speed: true,
        disable_osd_seek: true,
        osd_timeout_secs: 0.125,
        ..old
    };
    let saved = serde_json::to_string(&configured).unwrap();
    let restored = Preferences::parse(&saved).unwrap();
    assert!(!restored.enable_osd);
    assert!(restored.live_text_enabled);
    assert!(restored.disable_osd_speed);
    assert!(restored.disable_osd_seek);
    assert_eq!(restored.osd_timeout_secs, 0.125);
    assert!(!restored.control_bar.auto_hide_enabled);
    assert_eq!(restored.control_bar.auto_hide_timeout_secs, 4.25);
    assert_eq!(restored.volume, 37.0);
    for timeout in ["-1", "1e300"] {
        assert!(Preferences::parse(&format!("{{\"osd_timeout_secs\":{timeout}}}")).is_err());
    }
    for timeout in ["-0.5", "1e300"] {
        assert!(
            Preferences::parse(&format!(
                "{{\"control_bar\":{{\"auto_hide_timeout_secs\":{timeout}}}}}"
            ))
            .is_err()
        );
    }
    assert_eq!(
        Preferences::parse(r#"{"osd_timeout_secs":0}"#)
            .unwrap()
            .osd_timeout_secs,
        0.0
    );
    for value in [4.999, 500.001, f64::NAN, f64::INFINITY] {
        assert!(Preferences::parse(&format!(r#"{{"osd_text_size":{value}}}"#)).is_err());
    }
}

#[test]
fn chapter_position_markers_default_off_and_round_trip_when_enabled() {
    let legacy = Preferences::parse("{}").unwrap();
    assert!(!legacy.show_chapter_pos);

    let enabled = Preferences {
        show_chapter_pos: true,
        ..legacy
    };
    let saved = serde_json::to_string(&enabled).unwrap();
    assert!(Preferences::parse(&saved).unwrap().show_chapter_pos);
}

#[test]
fn picture_in_picture_preferences_keep_iina_defaults_and_round_trip() {
    let legacy = Preferences::parse("{}").unwrap();
    assert_eq!(
        legacy.picture_in_picture.window_behavior,
        pip_preferences::WindowBehavior::Hide
    );
    assert!(!legacy.picture_in_picture.pause_when_entering);

    let configured = Preferences {
        picture_in_picture: pip_preferences::Settings {
            window_behavior: pip_preferences::WindowBehavior::Minimize,
            pause_when_entering: true,
            toggle_by_minimizing: true,
            minimize_toggle_video_only: true,
        },
        ..legacy
    };
    let saved = serde_json::to_string(&configured).unwrap();
    assert_eq!(
        Preferences::parse(&saved).unwrap().picture_in_picture,
        configured.picture_in_picture
    );
}

#[test]
fn action_after_launch_matches_welcome_default_and_round_trips_all_choices() {
    assert_eq!(
        Preferences::parse("{} ").unwrap().action_after_launch,
        ActionAfterLaunch::WelcomeWindow
    );
    for action in [
        ActionAfterLaunch::WelcomeWindow,
        ActionAfterLaunch::OpenPanel,
        ActionAfterLaunch::None,
    ] {
        let preferences = Preferences {
            action_after_launch: action,
            ..Preferences::default()
        };
        let saved = serde_json::to_string(&preferences).unwrap();
        assert_eq!(
            Preferences::parse(&saved).unwrap().action_after_launch,
            action
        );
        assert_eq!(ActionAfterLaunch::from_index(action.index()), Some(action));
    }
    assert_eq!(ActionAfterLaunch::from_index(-1), None);
    assert_eq!(ActionAfterLaunch::from_index(3), None);
    assert!(ActionAfterLaunch::WelcomeWindow.show_welcome(false));
    assert!(!ActionAfterLaunch::OpenPanel.show_welcome(false));
    assert!(ActionAfterLaunch::OpenPanel.open_file_panel(false));
    assert!(!ActionAfterLaunch::OpenPanel.open_file_panel(true));
    assert!(!ActionAfterLaunch::None.show_welcome(false));
    assert!(!ActionAfterLaunch::None.open_file_panel(false));
}

#[test]
fn network_cache_preferences_match_reference_defaults_and_reach_native_options() {
    use network_preferences::Cache;

    let defaults = Preferences::parse("{}").unwrap().network_cache;
    assert_eq!(defaults, Cache::default());
    assert_eq!(defaults.prefetch_secs, 36_000.0);
    assert_eq!(defaults.buffer_size_kib, 153_600.0);
    assert_eq!(
        Command::NetworkCache(defaults.clone())
            .plan(&Playback::default())
            .unwrap()
            .commands,
        vec![
            vec!["set", "cache", "yes"],
            vec!["set", "cache-pause-initial", "no"],
            vec!["set", "cache-pause-wait", "1"],
            vec!["set", "cache-secs", "36000"],
            vec!["set", "demuxer-max-bytes", "153600KiB"],
        ]
    );

    let configured = Cache {
        enabled: false,
        pause_initial: true,
        pause_wait_secs: 2.5,
        prefetch_secs: 12_000.0,
        buffer_size_kib: 512_000.0,
        show_buffering_throbber: false,
        show_seeking_throbber: false,
    };
    let persisted = Preferences {
        network_cache: configured.clone(),
        ..Preferences::default()
    };
    assert_eq!(
        Preferences::parse(&serde_json::to_string(&persisted).unwrap())
            .unwrap()
            .network_cache,
        configured
    );
    assert!(
        Cache {
            pause_wait_secs: f64::NAN,
            ..defaults
        }
        .validate()
        .is_err()
    );
    assert!(
        Cache {
            prefetch_secs: 2_000_000_000.0,
            ..Cache::default()
        }
        .validate()
        .is_ok()
    );
    assert!(
        Cache {
            buffer_size_kib: 2_000_000_001.0,
            ..Cache::default()
        }
        .validate()
        .is_err()
    );
}

#[test]
fn network_connection_preferences_default_and_round_trip() {
    use network_preferences::{Connection, RtspTransport};

    let defaults = Preferences::parse("{}").unwrap().network;
    assert_eq!(defaults, Connection::default());
    let configured = Connection {
        user_agent: "player agent".into(),
        http_proxy: "http://proxy.example:8080".into(),
        rtsp_transport: RtspTransport::Http,
    };
    let saved = Preferences {
        network: configured.clone(),
        ..Preferences::default()
    };
    assert_eq!(
        Preferences::parse(&serde_json::to_string(&saved).unwrap())
            .unwrap()
            .network,
        configured
    );
    assert!(Preferences::parse(r#"{"network":{"user_agent":"bad\u0000value"}}"#).is_err());
}

#[test]
fn online_video_preferences_keep_reference_defaults_and_update_mpv() {
    use network_preferences::OnlineVideos;

    let defaults = Preferences::parse("{}").unwrap().network_online_videos;
    assert_eq!(defaults, OnlineVideos::default());
    assert!(defaults.enabled);
    assert_eq!(
        Command::NetworkOnlineVideos(defaults.clone())
            .plan(&Playback::default())
            .unwrap()
            .commands,
        vec![
            vec!["set", "ytdl", "yes"],
            vec!["set", "ytdl-raw-options", ""],
        ]
    );
    let configured = OnlineVideos {
        enabled: false,
        search_path: "C:\\apps\\yt-dlp".into(),
        raw_options: "proxy=http://proxy.example:8080".into(),
    };
    let preferences = Preferences {
        network_online_videos: configured.clone(),
        ..Preferences::default()
    };
    assert_eq!(
        Preferences::parse(&serde_json::to_string(&preferences).unwrap())
            .unwrap()
            .network_online_videos,
        configured
    );
    assert!(
        OnlineVideos {
            raw_options: "bad\0option".into(),
            ..OnlineVideos::default()
        }
        .validate()
        .is_err()
    );
}

#[test]
fn subtitle_ass_override_preferences_default_to_scale_and_round_trip_per_lane() {
    use subtitle_controls::AssOverrideLevel;

    let legacy = Preferences::parse("{}").unwrap();
    assert_eq!(legacy.subtitle_ass_override, AssOverrideLevel::Scale);
    assert_eq!(
        legacy.secondary_subtitle_ass_override,
        AssOverrideLevel::Scale
    );
    let configured = Preferences {
        subtitle_ass_override: AssOverrideLevel::Force,
        secondary_subtitle_ass_override: AssOverrideLevel::No,
        ..legacy
    };
    let restored = Preferences::parse(&serde_json::to_string(&configured).unwrap()).unwrap();
    assert_eq!(restored.subtitle_ass_override, AssOverrideLevel::Force);
    assert_eq!(
        restored.secondary_subtitle_ass_override,
        AssOverrideLevel::No
    );
}

#[test]
fn subtitle_position_preferences_keep_source_defaults_and_round_trip() {
    use subtitle_controls::{HorizontalAlignment, PositionPreferences, VerticalAlignment};

    let legacy = Preferences::parse("{}").unwrap();
    assert_eq!(legacy.subtitle_position, PositionPreferences::default());

    let configured = Preferences {
        subtitle_position: PositionPreferences {
            horizontal: HorizontalAlignment::Right,
            vertical: VerticalAlignment::Top,
            position: 84.25,
            display_in_letterbox: false,
            scale_with_window: false,
            margin_x: 31.5,
            margin_y: 18.0,
        },
        ..legacy
    };
    let restored = Preferences::parse(&serde_json::to_string(&configured).unwrap()).unwrap();
    assert_eq!(restored.subtitle_position, configured.subtitle_position);

    for invalid in [
        r#"{"subtitle_position":{"position":100.1}}"#,
        r#"{"subtitle_position":{"margin_x":-0.1}}"#,
        r#"{"subtitle_position":{"margin_y":1e300}}"#,
        r#"{"subtitle_position":{"horizontal":"leading"}}"#,
    ] {
        assert!(Preferences::parse(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn subtitle_other_preferences_preserve_the_reference_encoding_menu() {
    use subtitle_controls::{OtherPreferences, SUBTITLE_ENCODING_CHOICES};

    assert_eq!(SUBTITLE_ENCODING_CHOICES.len(), 44);
    let legacy = Preferences::parse("{}").unwrap();
    assert_eq!(legacy.subtitle_other, OtherPreferences::default());
    assert_eq!(legacy.subtitle_other.encoding_index(), Some(0));

    let configured = Preferences {
        subtitle_other: OtherPreferences {
            preferred_languages: "eng,jpn,zh-Hant".into(),
            default_encoding: "SHIFT-JIS".into(),
        },
        ..legacy
    };
    let restored = Preferences::parse(&serde_json::to_string(&configured).unwrap()).unwrap();
    assert_eq!(restored.subtitle_other, configured.subtitle_other);
    assert!(
        restored
            .subtitle_other
            .commands()
            .contains(&Command::SubtitlePreferredLanguages(
                "eng,jpn,zh-Hant".into()
            ))
    );
    assert!(
        restored
            .subtitle_other
            .commands()
            .contains(&Command::SubtitleEncoding("SHIFT-JIS".into()))
    );

    assert!(
        Preferences::parse(r#"{"subtitle_other":{"default_encoding":"invented-codepage"}}"#)
            .is_err()
    );
    assert!(
        Preferences::parse(r#"{"subtitle_other":{"preferred_languages":"eng\nfre"}}"#).is_err()
    );
}

#[test]
fn subtitle_auto_load_preferences_keep_reference_choices_and_defaults() {
    use subtitle_controls::{AutoLoadMode, AutoLoadPreferences};

    let legacy = Preferences::parse("{}").unwrap();
    assert_eq!(legacy.subtitle_auto_load, AutoLoadPreferences::default());
    assert_eq!(legacy.subtitle_auto_load.mode, AutoLoadMode::Intelligent);
    assert_eq!(legacy.subtitle_auto_load.search_paths, "./*");
    assert_eq!(AutoLoadMode::Disabled.index(), 0);
    assert_eq!(AutoLoadMode::Filename.index(), 1);
    assert_eq!(AutoLoadMode::Intelligent.index(), 2);
    assert_eq!(AutoLoadMode::from_index(3), None);
    assert_eq!(
        AutoLoadPreferences {
            mode: AutoLoadMode::Disabled,
            ..AutoLoadPreferences::default()
        }
        .mpv_matching_mode(),
        "no"
    );
    assert_eq!(
        AutoLoadPreferences {
            mode: AutoLoadMode::Filename,
            ..AutoLoadPreferences::default()
        }
        .mpv_matching_mode(),
        "fuzzy"
    );
    assert_eq!(AutoLoadPreferences::default().mpv_matching_mode(), "exact");

    let configured = Preferences {
        subtitle_auto_load: AutoLoadPreferences {
            mode: AutoLoadMode::Filename,
            priority_keywords: "forced, SDH".into(),
            search_paths: std::env::join_paths(["./subs", "../captions"])
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        },
        ..legacy
    };
    let restored = Preferences::parse(&serde_json::to_string(&configured).unwrap()).unwrap();
    assert_eq!(restored.subtitle_auto_load, configured.subtitle_auto_load);
    assert_eq!(
        Command::SubtitleAutoLoad(configured.subtitle_auto_load.clone())
            .plan(&Playback::default())
            .unwrap()
            .commands,
        [
            ["set", "sub-auto", "fuzzy"].map(str::to_owned).to_vec(),
            [
                "set",
                "sub-file-paths",
                std::env::join_paths(["./subs", "../captions"])
                    .unwrap()
                    .to_string_lossy()
                    .as_ref(),
            ]
            .map(str::to_owned)
            .to_vec(),
        ]
    );
    assert_eq!(
        AutoLoadPreferences::default().mpv_search_paths(),
        None,
        "IINA's default glob is not an mpv directory path"
    );
    for invalid in [
        r#"{"subtitle_auto_load":{"priority_keywords":"eng\nforced"}}"#,
        r#"{"subtitle_auto_load":{"search_paths":"subs\u0000"}}"#,
        r#"{"subtitle_auto_load":{"mode":"unknown"}}"#,
    ] {
        assert!(Preferences::parse(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn subtitle_text_preferences_keep_legacy_defaults_and_fractional_sizes() {
    let legacy = Preferences::parse("{}").unwrap();
    assert_eq!(
        legacy.subtitle_font,
        subtitle_controls::DEFAULT_SUBTITLE_FONT
    );
    assert_eq!(legacy.subtitle_font_size, 55.0);

    let configured = Preferences {
        subtitle_font: "Noto Sans CJK JP".into(),
        subtitle_font_size: 42.375,
        ..legacy
    };
    let restored = Preferences::parse(&serde_json::to_string(&configured).unwrap()).unwrap();
    assert_eq!(restored.subtitle_font, "Noto Sans CJK JP");
    assert_eq!(restored.subtitle_font_size, 42.375);

    for invalid in ["-1", "1e300", "NaN"] {
        assert!(Preferences::parse(&format!(r#"{{"subtitle_font_size":{invalid}}}"#)).is_err());
    }
    assert!(Preferences::parse(r#"{"subtitle_font_size":0}"#).is_err());
    assert!(Preferences::parse(r#"{"subtitle_font":"bad\nfont"}"#).is_err());
}

#[test]
fn subtitle_text_style_options_validate_and_round_trip() {
    use subtitle_controls::TextStylePreferences;

    let configured = Preferences {
        subtitle_text_style: TextStylePreferences {
            bold: true,
            italic: true,
            blur: 1.25,
            spacing: -0.5,
            outline_size: 4.5,
            shadow_offset: 2.25,
            text_color: subtitle_color::SubtitleColor::from_rgba([0.25, 0.5, 0.75, 1.0]).unwrap(),
            ..TextStylePreferences::default()
        },
        ..Preferences::default()
    };
    let restored = Preferences::parse(&serde_json::to_string(&configured).unwrap()).unwrap();
    assert_eq!(restored.subtitle_text_style, configured.subtitle_text_style);

    for invalid in [
        r#"{"subtitle_text_style":{"blur":20.001}}"#,
        r#"{"subtitle_text_style":{"spacing":"wide"}}"#,
        r#"{"subtitle_text_style":{"outline_size":-0.1}}"#,
        r#"{"subtitle_text_style":{"shadow_offset":-1}}"#,
        r#"{"subtitle_text_style":{"text_color":[2,0,0,1]}}"#,
    ] {
        assert!(Preferences::parse(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn preferences_reject_future_schema() {
    assert!(Preferences::parse(r#"{"schema_version":2}"#).is_err());
    assert_eq!(Preferences::parse("{}").unwrap().volume, 100.0);
    assert!(Preferences::parse("{}").unwrap().recent_media.is_empty());
}

#[test]
fn accessibility_motion_preference_defaults_off_and_round_trips() {
    assert!(!Preferences::parse("{}").unwrap().disable_animations);

    let configured = Preferences {
        disable_animations: true,
        ..Preferences::default()
    };
    let encoded = serde_json::to_string(&configured).unwrap();
    assert!(Preferences::parse(&encoded).unwrap().disable_animations);
}

#[test]
fn recent_media_round_trips_and_rejects_invalid_history() {
    let item = RecentMedia {
        path: r"C:\Фильмы\a.mkv".into(),
        title: "A film".into(),
        position: 87.25,
    };
    let prefs = Preferences {
        recent_media: vec![item.clone()],
        ..Preferences::default()
    };
    let encoded = serde_json::to_string(&prefs).unwrap();
    assert_eq!(Preferences::parse(&encoded).unwrap().recent_media, [item]);
    assert!(
        Preferences::parse(r#"{"recent_media":[{"path":"x","title":"x","position":-1.0}]}"#)
            .is_err()
    );
    assert!(
        Preferences::parse(
            r#"{"recent_media":[{"path":"x","title":"x","position":1.0,"secret":true}]}"#
        )
        .is_err()
    );
}

#[test]
fn playback_history_defaults_on_and_round_trips_independently_of_recent_media() {
    let old_preferences = Preferences::parse("{}").unwrap();
    assert!(old_preferences.record_playback_history);
    assert!(old_preferences.playback_history.is_empty());

    let item = playback_history::Entry {
        path: "file:///C:/media/movie.mkv".into(),
        date_ms: 1_750_000_000_000,
        progress: Some(12.5),
        duration: 90.0,
    };
    let legacy = serde_json::json!({ "playback_history": [item] });
    let parsed = Preferences::parse(&legacy.to_string()).unwrap();
    assert_eq!(parsed.playback_history, [item]);
    assert!(
        serde_json::to_value(&parsed)
            .unwrap()
            .get("playback_history")
            .is_none()
    );
    assert!(
        Preferences::parse(
            r#"{"playback_history":[{"path":"x","date_ms":1,"progress":-1.0,"duration":1.0}]}"#
        )
        .is_err()
    );
}

#[test]
fn remembered_media_moves_to_front_and_bounds_history() {
    let mut prefs = Preferences::default();
    for index in 0..12 {
        prefs.remember_media(RecentMedia {
            path: format!("film-{index}.mkv"),
            title: format!("Film {index}"),
            position: index as f64,
        });
    }
    prefs.remember_media(RecentMedia {
        path: "film-4.mkv".into(),
        title: "Film Four".into(),
        position: 44.0,
    });
    assert_eq!(prefs.recent_media.len(), 10);
    assert_eq!(prefs.recent_media[0].path, "film-4.mkv");
    assert_eq!(prefs.recent_media[0].position, 44.0);
    assert_eq!(
        prefs
            .recent_media
            .iter()
            .filter(|item| item.path == "film-4.mkv")
            .count(),
        1
    );
    assert!(
        !prefs
            .recent_media
            .iter()
            .any(|item| item.path == "film-0.mkv")
    );
}

#[test]
fn amplified_volume_round_trips_and_rejects_invalid_values() {
    for volume in [0.0, 100.0, 150.0, 200.0] {
        assert_eq!(
            Command::Volume(volume).mpv_args().unwrap()[2],
            volume.to_string()
        );
        let json = format!(r#"{{"volume":{volume},"volume_boost":true}}"#);
        assert_eq!(Preferences::parse(&json).unwrap().volume, volume);
    }
    for volume in [-1.0, 200.1, f64::NAN, f64::INFINITY] {
        assert!(Command::Volume(volume).mpv_args().is_err());
    }
    assert!(Preferences::parse(r#"{"volume":201}"#).is_err());
    assert!(!Preferences::default().volume_boost);
    assert_eq!(
        Preferences::parse(r#"{"volume":150}"#).unwrap().volume,
        100.0
    );
}
