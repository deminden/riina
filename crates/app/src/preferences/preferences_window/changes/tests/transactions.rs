use super::*;
use crate::player_ui::glue_tests::Harness;

struct Transaction {
    player: Harness,
    window: PreferencesWindow,
    profile: TestPath,
}

impl Transaction {
    fn new() -> Self {
        Self::with_credentials(CredentialStore {
            open_subtitles: |_| panic!("Unexpected OpenSubtitles credential write"),
            assrt: |_| panic!("Unexpected Assrt credential write"),
        })
    }

    fn with_credentials(credentials: CredentialStore) -> Self {
        i_slint_backend_testing::init_no_event_loop();
        let profile = TestPath::new();
        let initial = Preferences::default();
        crate::preference_store::Writer::new(profile.0.clone(), initial.clone())
            .save(&initial)
            .unwrap();
        let player = Harness::start(profile.0.clone(), false).unwrap();
        let window = PreferencesWindow::new().unwrap();

        crate::advanced_preferences::install_window(&window, &initial.advanced);
        crate::audio_preferences::install_window(&window, &initial.audio, &[]);
        crate::tone_mapping_preferences::install_window(&window, &initial.hdr_tone_mapping);
        window.set_video_threads_text(initial.video_threads.to_string().into());
        crate::network_connection_preferences::install_window(&window, &initial.network);
        window.set_allow_multiple_windows(initial.allow_multiple_windows);
        window
            .set_group_simultaneous_opens_in_playlist(initial.group_simultaneous_opens_in_playlist);
        window.set_allow_duplicate_players(initial.allow_duplicate_players);
        refresh(&window, &player.app);
        connect_with_credentials(
            &player.app,
            &window,
            player.sender.clone(),
            profile.0.clone(),
            initial,
            Arc::new(Mutex::new(riina_core::Playback::default())),
            credentials,
        );
        Self {
            player,
            window,
            profile,
        }
    }

    fn apply(&self) -> Vec<Command> {
        self.window.invoke_settings_changed();
        self.player.engine.commands.try_iter().collect()
    }

    fn bytes(&self) -> Vec<u8> {
        std::fs::read(&self.profile.0).unwrap()
    }

    fn expected_subtitle_reload(&self) -> Command {
        Command::SubtitleAutoLoad(self.profile.read().subtitle_auto_load)
    }

    fn saturate(&self) -> Vec<Command> {
        let queued = std::iter::from_fn(|| {
            self.player
                .sender
                .send(Command::ToggleMute)
                .ok()
                .map(|()| Command::ToggleMute)
        })
        .take(65)
        .collect::<Vec<_>>();
        assert_eq!(queued.len(), 64);
        queued
    }
}

macro_rules! checkbox_cases {
    ($($test:ident => $window_get:ident, $window_set:ident, $app_get:ident, $saved:expr;)+) => {
        $(
            #[test]
            fn $test() {
                let transaction = Transaction::new();
                let target = !transaction.window.$window_get();
                transaction.window.$window_set(target);
                assert_eq!(transaction.apply(), [transaction.expected_subtitle_reload()]);
                assert_eq!(($saved)(&transaction.profile.read()), target);
                assert_eq!(transaction.player.app.$app_get(), target);
            }
        )+
    };
}

checkbox_cases! {
    fullscreen_intent_persists_and_projects =>
        get_fullscreen_when_open, set_fullscreen_when_open, get_fullscreen_when_open,
        |p: &Preferences| p.fullscreen_when_open;
    fullscreen_entry_resume_policy_persists_and_projects =>
        get_play_when_entering_fullscreen, set_play_when_entering_fullscreen,
        get_play_when_entering_fullscreen, |p: &Preferences| p.play_when_entering_fullscreen;
    fullscreen_exit_pause_policy_persists_and_projects =>
        get_pause_when_leaving_fullscreen, set_pause_when_leaving_fullscreen,
        get_pause_when_leaving_fullscreen, |p: &Preferences| p.pause_when_leaving_fullscreen;
    inactive_pause_policy_persists_and_projects =>
        get_pause_when_inactive, set_pause_when_inactive,
        get_pause_when_inactive, |p: &Preferences| p.pause_when_inactive;
    minimized_pause_policy_persists_and_projects =>
        get_pause_when_minimized, set_pause_when_minimized,
        get_pause_when_minimized, |p: &Preferences| p.pause_when_minimized;
    sleep_pause_policy_persists_and_projects =>
        get_pause_when_goes_to_sleep, set_pause_when_goes_to_sleep,
        get_pause_when_goes_to_sleep, |p: &Preferences| p.pause_when_goes_to_sleep;
    quitting_last_player_policy_persists_and_projects =>
        get_quit_when_no_opened_window, set_quit_when_no_opened_window,
        get_quit_when_no_opened_window, |p: &Preferences| p.quit_when_no_opened_window;
    recent_files_policy_persists_and_projects =>
        get_record_recent_files, set_record_recent_files,
        get_record_recent_files, |p: &Preferences| p.record_recent_files;
    open_menu_tracking_policy_persists_and_projects =>
        get_track_all_files_in_recent_open_menu, set_track_all_files_in_recent_open_menu,
        get_track_all_files_in_recent_open_menu, |p: &Preferences| p.track_all_files_in_recent_open_menu;
    playlist_auto_add_persists_and_projects =>
        get_playlist_auto_add, set_playlist_auto_add, get_playlist_auto_add,
        |p: &Preferences| p.playlist_auto_add;
    playlist_metadata_persists_and_projects =>
        get_playlist_show_metadata, set_playlist_show_metadata, get_playlist_show_metadata,
        |p: &Preferences| p.playlist_show_metadata;
    music_playlist_metadata_persists_and_projects =>
        get_playlist_show_metadata_in_music_mode, set_playlist_show_metadata_in_music_mode,
        get_playlist_show_metadata_in_music_mode, |p: &Preferences| p.playlist_show_metadata_in_music_mode;
    repeat_policy_persists_and_projects =>
        get_auto_repeat, set_auto_repeat, get_auto_repeat, |p: &Preferences| p.auto_repeat;
    chapter_markers_persist_and_project =>
        get_show_chapter_pos, set_show_chapter_pos, get_show_chapter_pos,
        |p: &Preferences| p.show_chapter_pos;
    animation_preference_persists_and_projects =>
        get_disable_animations, set_disable_animations, get_disable_animations,
        |p: &Preferences| p.disable_animations;
    always_show_on_top_icon_persists_and_projects =>
        get_always_show_on_top_icon, set_always_show_on_top_icon, get_always_show_on_top_icon,
        |p: &Preferences| p.always_show_on_top_icon;
    osd_enabled_persists_and_projects =>
        get_osd_enabled, set_osd_enabled, get_osd_enabled, |p: &Preferences| p.enable_osd;
    osd_pause_suppression_persists_and_projects =>
        get_disable_osd_pause_resume, set_disable_osd_pause_resume, get_disable_osd_pause_resume,
        |p: &Preferences| p.disable_osd_pause_resume;
    thumbnails_enabled_persists_and_projects =>
        get_thumbnail_preview_enabled, set_thumbnail_preview_enabled, get_thumbnail_preview_enabled,
        |p: &Preferences| p.thumbnail_preview.enabled;
    remote_thumbnail_policy_persists_and_projects =>
        get_thumbnail_remote_enabled, set_thumbnail_remote_enabled, get_thumbnail_remote_enabled,
        |p: &Preferences| p.thumbnail_preview.enable_for_remote_files;
    pip_pause_policy_persists_and_projects =>
        get_pip_pause_when_entering, set_pip_pause_when_entering, get_pip_pause_when_entering,
        |p: &Preferences| p.picture_in_picture.pause_when_entering;
    pip_minimize_policy_persists_and_projects =>
        get_pip_toggle_by_minimizing, set_pip_toggle_by_minimizing, get_pip_toggle_by_minimizing,
        |p: &Preferences| p.picture_in_picture.toggle_by_minimizing;
    floating_window_policy_persists_and_projects =>
        get_always_float_on_top, set_always_float_on_top, get_always_float_on_top,
        |p: &Preferences| p.always_float_on_top;
    screenshot_preview_persists_and_projects =>
        get_show_preview, set_show_preview, get_screenshot_show_preview,
        |p: &Preferences| p.screenshot.show_preview;
    screenshot_subtitle_policy_persists_and_projects =>
        get_include_subtitles, set_include_subtitles, get_screenshot_include_subtitles,
        |p: &Preferences| p.screenshot.include_subtitles;
}

macro_rules! invalid_values {
    ($($test:ident => $get:ident, $set:ident, $value:expr;)+) => {
        $(
            #[test]
            fn $test() {
                let transaction = Transaction::new();
                let before = transaction.bytes();
                let original = transaction.window.$get();
                transaction.window.$set($value);
                assert!(transaction.apply().is_empty());
                assert_eq!(transaction.bytes(), before);
                assert_eq!(transaction.window.$get(), original);
            }
        )+
    };
}

invalid_values! {
    invalid_repeat_choice_leaves_settings_untouched =>
        get_default_repeat_mode_index, set_default_repeat_mode_index, -1;
    invalid_appearance_leaves_settings_untouched =>
        get_appearance_index, set_appearance_index, -1;
    invalid_thumbnail_cache_leaves_settings_untouched =>
        get_thumbnail_cache_size_text, set_thumbnail_cache_size_text, "abc".into();
    invalid_thumbnail_width_leaves_settings_untouched =>
        get_thumbnail_width_text, set_thumbnail_width_text, "abc".into();
    invalid_osd_timeout_leaves_settings_untouched =>
        get_osd_timeout_text, set_osd_timeout_text, "abc".into();
    invalid_osd_text_size_leaves_settings_untouched =>
        get_osd_text_size_text, set_osd_text_size_text, "abc".into();
    invalid_pip_behavior_leaves_settings_untouched =>
        get_pip_window_behavior_index, set_pip_window_behavior_index, -1;
    invalid_resize_timing_leaves_settings_untouched =>
        get_window_resize_timing_index, set_window_resize_timing_index, -1;
    invalid_resize_option_leaves_settings_untouched =>
        get_window_resize_option_index, set_window_resize_option_index, -1;
    invalid_decoder_leaves_settings_untouched =>
        get_hardware_decoder_index, set_hardware_decoder_index, -1;
    invalid_decoder_thread_count_leaves_settings_untouched =>
        get_video_threads_text, set_video_threads_text, "-1".into();
    invalid_subtitle_font_size_leaves_settings_untouched =>
        get_subtitle_font_size_text, set_subtitle_font_size_text, "abc".into();
    invalid_network_cache_size_leaves_settings_untouched =>
        get_network_cache_size_kib_text, set_network_cache_size_kib_text, "abc".into();
    invalid_network_prefetch_leaves_settings_untouched =>
        get_network_cache_prefetch_secs_text, set_network_cache_prefetch_secs_text, "abc".into();
}

#[test]
fn volume_boost_sends_its_command_before_subtitle_reload() {
    let transaction = Transaction::new();
    transaction.window.set_volume_boost(true);
    assert_eq!(
        transaction.apply(),
        [
            Command::SetVolumeBoost(true),
            transaction.expected_subtitle_reload()
        ]
    );
    assert!(transaction.profile.read().volume_boost);
    assert!(transaction.player.app.get_volume_boost());
}

#[test]
fn pause_when_open_callback_observes_the_persisted_edit_and_fires_once() {
    let transaction = Transaction::new();
    let path = transaction.profile.0.clone();
    let observed = std::rc::Rc::new(std::cell::Cell::new(0));
    let calls = observed.clone();
    transaction
        .player
        .app
        .on_pause_when_open_changed(move |enabled| {
            let saved = Preferences::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
            assert_eq!(saved.pause_when_open, enabled);
            calls.set(calls.get() + 1);
        });
    transaction.window.set_pause_when_open(true);
    assert_eq!(
        transaction.apply(),
        [transaction.expected_subtitle_reload()]
    );
    assert!(transaction.player.app.get_pause_when_open());
    assert_eq!(observed.get(), 1);
    assert_eq!(
        transaction.apply(),
        [transaction.expected_subtitle_reload()]
    );
    assert_eq!(observed.get(), 1);
}

#[test]
fn resume_policy_sends_its_command_only_when_changed() {
    let transaction = Transaction::new();
    let target = !transaction.window.get_resume_last_position();
    transaction.window.set_resume_last_position(target);
    assert_eq!(
        transaction.apply(),
        [
            transaction.expected_subtitle_reload(),
            Command::SetResumeLastPosition(target)
        ]
    );
    assert_eq!(transaction.profile.read().resume_last_position, target);
    assert_eq!(transaction.player.app.get_resume_last_position(), target);
    assert_eq!(
        transaction.apply(),
        [transaction.expected_subtitle_reload()]
    );
}

#[test]
fn record_history_callback_fires_after_the_file_changes_and_only_once() {
    let transaction = Transaction::new();
    let path = transaction.profile.0.clone();
    let observed = std::rc::Rc::new(std::cell::Cell::new(0));
    let calls = observed.clone();
    transaction
        .player
        .app
        .on_record_playback_history_changed(move |enabled| {
            let saved = Preferences::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
            assert_eq!(saved.record_playback_history, enabled);
            calls.set(calls.get() + 1);
        });
    transaction.window.set_record_playback_history(false);
    assert_eq!(
        transaction.apply(),
        [transaction.expected_subtitle_reload()]
    );
    assert!(!transaction.player.app.get_record_playback_history());
    assert_eq!(observed.get(), 1);
    assert_eq!(
        transaction.apply(),
        [transaction.expected_subtitle_reload()]
    );
    assert_eq!(observed.get(), 1);
}

#[test]
fn advanced_edits_report_that_the_player_must_restart() {
    let transaction = Transaction::new();
    transaction.window.set_advanced_enabled(true);
    assert_eq!(
        transaction.apply(),
        [transaction.expected_subtitle_reload()]
    );
    assert!(transaction.profile.read().advanced.enabled);
    assert_eq!(
        transaction.player.app.get_status(),
        "Advanced settings apply after restarting Riina"
    );
}

#[test]
fn audio_engine_edits_report_that_the_player_must_restart() {
    let transaction = Transaction::new();
    transaction.window.set_audio_threads_text("3".into());
    assert_eq!(
        transaction.apply(),
        [transaction.expected_subtitle_reload()]
    );
    assert_eq!(transaction.profile.read().audio.audio_threads, 3);
    assert_eq!(
        transaction.player.app.get_status(),
        "Audio engine preferences apply after restarting Riina"
    );
}

#[test]
fn rejected_volume_boost_restores_the_setting_without_writing() {
    let transaction = Transaction::new();
    let before = transaction.bytes();
    let queued = transaction.saturate();
    transaction.window.set_volume_boost(true);
    assert_eq!(transaction.apply(), queued);
    assert_eq!(transaction.bytes(), before);
    assert!(!transaction.window.get_volume_boost());
    assert!(!transaction.player.app.get_volume_boost());
}

#[test]
fn rejected_subtitle_reload_restores_an_independent_live_setting() {
    let transaction = Transaction::new();
    let before = transaction.bytes();
    let queued = transaction.saturate();
    transaction.window.set_fullscreen_when_open(true);
    assert_eq!(transaction.apply(), queued);
    assert_eq!(transaction.bytes(), before);
    assert!(!transaction.window.get_fullscreen_when_open());
    assert!(!transaction.player.app.get_fullscreen_when_open());
}

#[test]
fn music_mode_callback_reads_the_updated_preference_file() {
    let transaction = Transaction::new();
    i_slint_backend_testing::mock_elapsed_time(std::time::Duration::ZERO);
    let path = transaction.profile.0.clone();
    let observed = std::rc::Rc::new(std::cell::Cell::new(None));
    let value = observed.clone();
    transaction
        .player
        .app
        .on_auto_switch_music_mode_changed(move |enabled| {
            let saved = Preferences::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
            assert_eq!(saved.auto_switch_to_music_mode, enabled);
            value.set(Some(enabled));
        });
    let target = !transaction.window.get_auto_switch_to_music_mode();
    transaction.window.set_auto_switch_to_music_mode(target);
    assert_eq!(
        transaction.apply(),
        [transaction.expected_subtitle_reload()]
    );
    i_slint_backend_testing::mock_elapsed_time(std::time::Duration::ZERO);
    assert_eq!(observed.get(), Some(target));
}

#[test]
fn opensubtitles_storage_failure_keeps_the_prior_key_and_omits_secrets_from_json() {
    let transaction = Transaction::with_credentials(CredentialStore {
        open_subtitles: |key| {
            assert_eq!(key, "candidate-api-key");
            Err("credential store rejected the key".into())
        },
        assrt: |_| panic!("Unexpected Assrt credential write"),
    });
    let app = &transaction.player.app;
    app.set_open_subtitles_api_key("existing-api-key".into());
    transaction
        .window
        .set_open_subtitles_api_key("candidate-api-key".into());
    assert_eq!(
        transaction.apply(),
        [transaction.expected_subtitle_reload()]
    );
    assert_eq!(app.get_open_subtitles_api_key(), "existing-api-key");
    assert_eq!(
        transaction.window.get_open_subtitles_api_key(),
        "existing-api-key"
    );
    assert_eq!(app.get_status(), "credential store rejected the key");
    assert!(
        transaction
            .profile
            .read()
            .online_subtitles
            .api_key
            .is_empty()
    );
}

#[test]
fn assrt_storage_failure_keeps_the_prior_token_and_omits_secrets_from_json() {
    let transaction = Transaction::with_credentials(CredentialStore {
        open_subtitles: |_| panic!("Unexpected OpenSubtitles credential write"),
        assrt: |token| {
            assert_eq!(token, "candidate-assrt-token");
            Err("credential store rejected the token".into())
        },
    });
    let app = &transaction.player.app;
    app.set_assrt_token("existing-assrt-token".into());
    transaction
        .window
        .set_assrt_token("candidate-assrt-token".into());
    assert_eq!(
        transaction.apply(),
        [transaction.expected_subtitle_reload()]
    );
    assert_eq!(app.get_assrt_token(), "existing-assrt-token");
    assert_eq!(transaction.window.get_assrt_token(), "existing-assrt-token");
    assert_eq!(app.get_status(), "credential store rejected the token");
    assert!(
        transaction
            .profile
            .read()
            .online_subtitles
            .assrt_token
            .is_empty()
    );
}

#[test]
fn valid_video_thread_count_reaches_the_engine_and_file_once() {
    let transaction = Transaction::new();
    transaction.window.set_video_threads_text("3".into());
    assert_eq!(
        transaction.apply(),
        [
            transaction.expected_subtitle_reload(),
            Command::VideoThreads(3)
        ]
    );
    assert_eq!(transaction.profile.read().video_threads, 3);
    assert_eq!(
        transaction.apply(),
        [transaction.expected_subtitle_reload()]
    );
}

#[test]
fn mixed_invalid_edit_reverses_the_live_volume_command_before_returning() {
    let transaction = Transaction::new();
    let before = transaction.bytes();
    transaction.window.set_volume_boost(true);
    transaction.window.set_subtitle_font_size_text("abc".into());
    assert_eq!(
        transaction.apply(),
        [
            Command::SetVolumeBoost(true),
            Command::SetVolumeBoost(false)
        ]
    );
    assert_eq!(transaction.bytes(), before);
    assert!(!transaction.player.app.get_volume_boost());
    assert!(!transaction.window.get_volume_boost());
}
