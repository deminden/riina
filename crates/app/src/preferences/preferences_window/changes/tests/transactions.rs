use super::*;
use crate::player_ui::glue_tests::Harness;
use riina_ui::PlayerPreferences;
use slint::ComponentHandle;

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
        crate::preference_store::Writer::new(profile.file.clone(), initial.clone())
            .save(&initial)
            .unwrap();
        let player = Harness::start(profile.file.clone(), false).unwrap();
        let window =
            crate::preferences_window::profile_window::create(&profile.file, &initial, &[], || {
                Ok(0)
            })
            .unwrap();
        refresh(&window, &player.app);
        connect_with_credentials(
            &player.app,
            &window,
            player.sender.clone(),
            player.session.preference_model.clone(),
            crate::PlaybackStore::default(),
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
        std::fs::read(&self.profile.file).unwrap()
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
    ($($test:ident => $([$form:ty])? $window_get:ident, $window_set:ident,
      $([$projected:ty])? $app_get:ident, $saved:expr;)+) => {
        $(
            #[test]
            fn $test() {
                let transaction = Transaction::new();
                let form = &transaction.window;
                $(let form = form.global::<$form>();)?
                let target = !form.$window_get();
                form.$window_set(target);
                assert_eq!(transaction.apply(), [transaction.expected_subtitle_reload()]);
                assert_eq!(($saved)(&transaction.profile.read()), target);
                let projected = &transaction.player.app;
                $(let projected = projected.global::<$projected>();)?
                assert_eq!(projected.$app_get(), target);
            }
        )+
    };
}

checkbox_cases! {
    fullscreen_intent_persists_and_projects =>
        [riina_ui::GeneralSettingsState]
        get_fullscreen_when_open, set_fullscreen_when_open, [PlayerPreferences] get_fullscreen_when_open,
        |p: &Preferences| p.fullscreen_when_open;
    fullscreen_entry_resume_policy_persists_and_projects =>
        [riina_ui::GeneralSettingsState]
        get_play_when_entering_fullscreen, set_play_when_entering_fullscreen,
        [PlayerPreferences] get_play_when_entering_fullscreen, |p: &Preferences| p.play_when_entering_fullscreen;
    fullscreen_exit_pause_policy_persists_and_projects =>
        [riina_ui::GeneralSettingsState]
        get_pause_when_leaving_fullscreen, set_pause_when_leaving_fullscreen,
        [PlayerPreferences] get_pause_when_leaving_fullscreen, |p: &Preferences| p.pause_when_leaving_fullscreen;
    blackout_preference_persists_and_projects =>
        [riina_ui::GeneralSettingsState]
        get_black_out_other_monitors, set_black_out_other_monitors,
        [PlayerPreferences] get_black_out_other_monitors, |p: &Preferences| p.black_out_other_monitors;
    legacy_fullscreen_selection_persists_and_projects =>
        [riina_ui::GeneralSettingsState]
        get_use_legacy_fullscreen, set_use_legacy_fullscreen,
        [PlayerPreferences] get_use_legacy_fullscreen, |p: &Preferences| p.use_legacy_fullscreen;
    inactive_pause_policy_persists_and_projects =>
        [riina_ui::GeneralSettingsState]
        get_pause_when_inactive, set_pause_when_inactive,
        [PlayerPreferences] get_pause_when_inactive, |p: &Preferences| p.pause_when_inactive;
    minimized_pause_policy_persists_and_projects =>
        [riina_ui::GeneralSettingsState]
        get_pause_when_minimized, set_pause_when_minimized,
        [PlayerPreferences] get_pause_when_minimized, |p: &Preferences| p.pause_when_minimized;
    sleep_pause_policy_persists_and_projects =>
        [riina_ui::GeneralSettingsState]
        get_pause_when_goes_to_sleep, set_pause_when_goes_to_sleep,
        [PlayerPreferences] get_pause_when_goes_to_sleep, |p: &Preferences| p.pause_when_goes_to_sleep;
    quitting_last_player_policy_persists_and_projects =>
        [riina_ui::GeneralSettingsState]
        get_quit_when_no_opened_window, set_quit_when_no_opened_window,
        [PlayerPreferences] get_quit_when_no_opened_window, |p: &Preferences| p.quit_when_no_opened_window;
    recent_files_policy_persists_and_projects =>
        [riina_ui::GeneralSettingsState]
        get_record_recent_files, set_record_recent_files,
        [PlayerPreferences] get_record_recent_files, |p: &Preferences| p.record_recent_files;
    open_menu_tracking_policy_persists_and_projects =>
        [riina_ui::GeneralSettingsState]
        get_track_all_files_in_recent_open_menu, set_track_all_files_in_recent_open_menu,
        [PlayerPreferences] get_track_all_files_in_recent_open_menu,
        |p: &Preferences| p.track_all_files_in_recent_open_menu;
    playlist_auto_add_persists_and_projects =>
        [riina_ui::GeneralSettingsState]
        get_playlist_auto_add, set_playlist_auto_add, [PlayerPreferences] get_playlist_auto_add,
        |p: &Preferences| p.playlist_auto_add;
    playlist_metadata_persists_and_projects =>
        [riina_ui::GeneralSettingsState]
        get_playlist_show_metadata, set_playlist_show_metadata, [PlayerPreferences] get_playlist_show_metadata,
        |p: &Preferences| p.playlist_show_metadata;
    music_playlist_metadata_persists_and_projects =>
        [riina_ui::GeneralSettingsState]
        get_playlist_show_metadata_in_music_mode, set_playlist_show_metadata_in_music_mode,
        [PlayerPreferences] get_playlist_show_metadata_in_music_mode,
        |p: &Preferences| p.playlist_show_metadata_in_music_mode;
    repeat_policy_persists_and_projects =>
        [riina_ui::GeneralSettingsState]
        get_auto_repeat, set_auto_repeat, [PlayerPreferences] get_auto_repeat, |p: &Preferences| p.auto_repeat;
    chapter_markers_persist_and_project =>
        get_show_chapter_pos, set_show_chapter_pos, [PlayerPreferences] get_show_chapter_pos,
        |p: &Preferences| p.show_chapter_pos;
    animation_preference_persists_and_projects =>
        get_disable_animations, set_disable_animations, [PlayerPreferences] get_disable_animations,
        |p: &Preferences| p.disable_animations;
    always_show_on_top_icon_persists_and_projects =>
        get_always_show_on_top_icon, set_always_show_on_top_icon,
        [riina_ui::PlayerLayoutState] get_always_show_on_top_icon,
        |p: &Preferences| p.always_show_on_top_icon;
    osd_enabled_persists_and_projects =>
        get_osd_enabled, set_osd_enabled, [PlayerPreferences] get_osd_enabled, |p: &Preferences| p.enable_osd;
    osd_pause_suppression_persists_and_projects =>
        get_disable_osd_pause_resume, set_disable_osd_pause_resume, [PlayerPreferences] get_disable_osd_pause_resume,
        |p: &Preferences| p.disable_osd_pause_resume;
    thumbnails_enabled_persists_and_projects =>
        get_thumbnail_preview_enabled, set_thumbnail_preview_enabled, [PlayerPreferences] get_thumbnail_preview_enabled,
        |p: &Preferences| p.thumbnail_preview.enabled;
    remote_thumbnail_policy_persists_and_projects =>
        get_thumbnail_remote_enabled, set_thumbnail_remote_enabled, [PlayerPreferences] get_thumbnail_remote_enabled,
        |p: &Preferences| p.thumbnail_preview.enable_for_remote_files;
    pip_pause_policy_persists_and_projects =>
        get_pip_pause_when_entering, set_pip_pause_when_entering, [PlayerPreferences] get_pip_pause_when_entering,
        |p: &Preferences| p.picture_in_picture.pause_when_entering;
    pip_minimize_policy_persists_and_projects =>
        get_pip_toggle_by_minimizing, set_pip_toggle_by_minimizing, [PlayerPreferences] get_pip_toggle_by_minimizing,
        |p: &Preferences| p.picture_in_picture.toggle_by_minimizing;
    floating_window_policy_persists_and_projects =>
        get_always_float_on_top, set_always_float_on_top, [riina_ui::PlayerLayoutState] get_always_float_on_top,
        |p: &Preferences| p.always_float_on_top;
    screenshot_preview_persists_and_projects =>
        [riina_ui::GeneralSettingsState]
        get_show_preview, set_show_preview, [PlayerPreferences] get_screenshot_show_preview,
        |p: &Preferences| p.screenshot.show_preview;
    screenshot_subtitle_policy_persists_and_projects =>
        [riina_ui::GeneralSettingsState]
        get_include_subtitles, set_include_subtitles, [PlayerPreferences] get_screenshot_include_subtitles,
        |p: &Preferences| p.screenshot.include_subtitles;
}

macro_rules! invalid_values {
    ($($test:ident => $([$form:ty])? $get:ident, $set:ident, $value:expr;)+) => {
        $(
            #[test]
            fn $test() {
                let transaction = Transaction::new();
                let before = transaction.bytes();
                let form = &transaction.window;
                $(let form = form.global::<$form>();)?
                let original = form.$get();
                form.$set($value);
                assert!(transaction.apply().is_empty());
                assert_eq!(transaction.bytes(), before);
                assert_eq!(form.$get(), original);
            }
        )+
    };
}

invalid_values! {
    invalid_repeat_choice_leaves_settings_untouched =>
        [riina_ui::GeneralSettingsState]
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
        [crate::SubtitleSettingsState]
        get_subtitle_font_size_text, set_subtitle_font_size_text, "abc".into();
    invalid_network_cache_size_leaves_settings_untouched =>
        [crate::NetworkSettingsState]
        get_network_cache_size_kib_text, set_network_cache_size_kib_text, "abc".into();
    invalid_network_prefetch_leaves_settings_untouched =>
        [crate::NetworkSettingsState]
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
    assert!(
        transaction
            .player
            .app
            .global::<PlayerPreferences>()
            .get_volume_boost()
    );
}

#[test]
fn pause_when_open_callback_observes_the_persisted_edit_and_fires_once() {
    let transaction = Transaction::new();
    let path = transaction.profile.file.clone();
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
    transaction
        .window
        .global::<riina_ui::GeneralSettingsState>()
        .set_pause_when_open(true);
    assert_eq!(
        transaction.apply(),
        [transaction.expected_subtitle_reload()]
    );
    assert!(
        transaction
            .player
            .app
            .global::<PlayerPreferences>()
            .get_pause_when_open()
    );
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
    let target = !transaction
        .window
        .global::<riina_ui::GeneralSettingsState>()
        .get_resume_last_position();
    transaction
        .window
        .global::<riina_ui::GeneralSettingsState>()
        .set_resume_last_position(target);
    assert_eq!(
        transaction.apply(),
        [
            transaction.expected_subtitle_reload(),
            Command::SetResumeLastPosition(target)
        ]
    );
    assert_eq!(transaction.profile.read().resume_last_position, target);
    assert_eq!(
        transaction
            .player
            .app
            .global::<PlayerPreferences>()
            .get_resume_last_position(),
        target
    );
    assert_eq!(
        transaction.apply(),
        [transaction.expected_subtitle_reload()]
    );
}

#[test]
fn record_history_callback_fires_after_the_file_changes_and_only_once() {
    let transaction = Transaction::new();
    let path = transaction.profile.file.clone();
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
    transaction
        .window
        .global::<riina_ui::GeneralSettingsState>()
        .set_record_playback_history(false);
    assert_eq!(
        transaction.apply(),
        [transaction.expected_subtitle_reload()]
    );
    assert!(
        !transaction
            .player
            .app
            .global::<PlayerPreferences>()
            .get_record_playback_history()
    );
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
    transaction
        .window
        .global::<crate::AdvancedSettingsState>()
        .set_advanced_enabled(true);
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
    transaction
        .window
        .global::<crate::AudioSettingsState>()
        .set_audio_threads_text("3".into());
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
    assert!(
        !transaction
            .player
            .app
            .global::<PlayerPreferences>()
            .get_volume_boost()
    );
}

#[test]
fn rejected_subtitle_reload_restores_an_independent_live_setting() {
    let transaction = Transaction::new();
    let before = transaction.bytes();
    let queued = transaction.saturate();
    transaction
        .window
        .global::<riina_ui::GeneralSettingsState>()
        .set_fullscreen_when_open(true);
    assert_eq!(transaction.apply(), queued);
    assert_eq!(transaction.bytes(), before);
    assert!(
        !transaction
            .window
            .global::<riina_ui::GeneralSettingsState>()
            .get_fullscreen_when_open()
    );
    assert!(
        !transaction
            .player
            .app
            .global::<PlayerPreferences>()
            .get_fullscreen_when_open()
    );
}

#[test]
fn music_mode_callback_reads_the_updated_preference_file() {
    let transaction = Transaction::new();
    i_slint_backend_testing::mock_elapsed_time(std::time::Duration::ZERO);
    let path = transaction.profile.file.clone();
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
    let target = !transaction
        .window
        .global::<riina_ui::GeneralSettingsState>()
        .get_auto_switch_to_music_mode();
    transaction
        .window
        .global::<riina_ui::GeneralSettingsState>()
        .set_auto_switch_to_music_mode(target);
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
    app.global::<PlayerPreferences>()
        .set_open_subtitles_api_key("existing-api-key".into());
    transaction
        .window
        .global::<crate::SubtitleSettingsState>()
        .set_open_subtitles_api_key("candidate-api-key".into());
    assert_eq!(
        transaction.apply(),
        [transaction.expected_subtitle_reload()]
    );
    assert_eq!(
        app.global::<PlayerPreferences>()
            .get_open_subtitles_api_key(),
        "existing-api-key"
    );
    assert_eq!(
        transaction
            .window
            .global::<crate::SubtitleSettingsState>()
            .get_open_subtitles_api_key(),
        "existing-api-key"
    );
    assert_eq!(app.get_status(), "credential store rejected the key");
    let recorded = transaction.player.session.preference_model.snapshot();
    assert_eq!(recorded.online_subtitles.api_key, "existing-api-key");
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
    app.global::<PlayerPreferences>()
        .set_assrt_token("existing-assrt-token".into());
    transaction
        .window
        .global::<crate::SubtitleSettingsState>()
        .set_assrt_token("candidate-assrt-token".into());
    assert_eq!(
        transaction.apply(),
        [transaction.expected_subtitle_reload()]
    );
    assert_eq!(
        app.global::<PlayerPreferences>().get_assrt_token(),
        "existing-assrt-token"
    );
    assert_eq!(
        transaction
            .window
            .global::<crate::SubtitleSettingsState>()
            .get_assrt_token(),
        "existing-assrt-token"
    );
    assert_eq!(app.get_status(), "credential store rejected the token");
    let recorded = transaction.player.session.preference_model.snapshot();
    assert_eq!(
        recorded.online_subtitles.assrt_token,
        "existing-assrt-token"
    );
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
fn cache_edits_apply_the_saved_policy_after_subtitle_reload() {
    let transaction = Transaction::new();
    let enabled = !transaction
        .window
        .global::<crate::NetworkSettingsState>()
        .get_network_cache_enabled();
    transaction
        .window
        .global::<crate::NetworkSettingsState>()
        .set_network_cache_enabled(enabled);
    let commands = transaction.apply();
    let saved = transaction.profile.read();
    assert_eq!(saved.network_cache.enabled, enabled);
    assert_eq!(
        transaction
            .player
            .app
            .global::<PlayerPreferences>()
            .get_network_cache_enabled(),
        enabled
    );
    assert_eq!(
        commands,
        [
            Command::SubtitleAutoLoad(saved.subtitle_auto_load),
            Command::NetworkCache(saved.network_cache)
        ]
    );
}

#[test]
fn mixed_invalid_edit_reverses_the_live_volume_command_before_returning() {
    let transaction = Transaction::new();
    let before = transaction.bytes();
    transaction.window.set_volume_boost(true);
    transaction
        .window
        .global::<crate::SubtitleSettingsState>()
        .set_subtitle_font_size_text("abc".into());
    assert_eq!(
        transaction.apply(),
        [
            Command::SetVolumeBoost(true),
            Command::SetVolumeBoost(false)
        ]
    );
    assert_eq!(transaction.bytes(), before);
    assert!(
        !transaction
            .player
            .app
            .global::<PlayerPreferences>()
            .get_volume_boost()
    );
    assert!(!transaction.window.get_volume_boost());
}

#[test]
fn an_invalid_launch_action_cannot_trigger_a_live_decoder_edit() {
    let transaction = Transaction::new();
    let before = transaction.bytes();
    transaction.window.set_volume_boost(true);
    let general = transaction
        .window
        .global::<riina_ui::GeneralSettingsState>();
    general.set_launch_action_index(-1);

    assert!(transaction.apply().is_empty());
    assert_eq!(transaction.bytes(), before);
    assert_eq!(
        general.get_launch_action_index(),
        Preferences::default().action_after_launch.index()
    );
    assert!(!transaction.window.get_volume_boost());
}

#[test]
fn screenshot_edits_preserve_the_configured_format_and_template_outside_the_form() {
    use riina_core::screenshot::Format;

    let transaction = Transaction::new();
    let app = &transaction.player.app;
    let mut screenshot = crate::screenshot_preferences::current(app).unwrap();
    screenshot.encoding.format = Format::Avif;
    screenshot.encoding.template = "%F-%P".into();
    crate::screenshot_preferences::install(app, &screenshot);
    refresh(&transaction.window, app);
    let general = transaction
        .window
        .global::<riina_ui::GeneralSettingsState>();
    assert_eq!(general.get_format_index(), -1);
    general.set_show_preview(false);

    assert_eq!(
        transaction.apply(),
        [transaction.expected_subtitle_reload()]
    );
    screenshot.show_preview = false;
    assert_eq!(transaction.profile.read().screenshot, screenshot);
    assert_eq!(
        crate::screenshot_preferences::current(app).unwrap(),
        screenshot
    );
}

#[test]
fn saving_a_session_retains_committed_preferences_and_updates_runtime_volume() {
    let transaction = Transaction::new();
    transaction
        .window
        .global::<crate::AudioSettingsState>()
        .set_audio_threads_text("3".into());
    transaction.window.set_thumbnail_preview_enabled(false);
    assert_eq!(
        transaction.apply(),
        [transaction.expected_subtitle_reload()]
    );
    transaction
        .player
        .app
        .global::<riina_ui::PlaybackAudioState>()
        .set_volume(72.0);
    transaction
        .player
        .session
        .save(&transaction.player.app)
        .unwrap();
    let saved = transaction.profile.read();
    assert_eq!(saved.audio.audio_threads, 3);
    assert!(!saved.thumbnail_preview.enabled);
    assert_eq!(saved.volume, 72.0);
}

#[test]
fn a_session_save_keeps_a_different_writer_s_newer_engine_settings() {
    let transaction = Transaction::new();
    let baseline = transaction.profile.read();
    let writer =
        crate::preference_store::Writer::new(transaction.profile.file.clone(), baseline.clone());
    let mut foreign = baseline;
    foreign.audio.audio_threads = 5;
    writer.save(&foreign).unwrap();
    transaction
        .player
        .app
        .global::<riina_ui::PlaybackAudioState>()
        .set_volume(65.0);
    transaction
        .player
        .session
        .save(&transaction.player.app)
        .unwrap();
    let saved = transaction.profile.read();
    assert_eq!(saved.audio.audio_threads, 5);
    assert_eq!(saved.volume, 65.0);
}

#[test]
fn session_save_captures_controls_edited_directly_in_the_player() {
    let transaction = Transaction::new();
    let app = &transaction.player.app;
    app.global::<riina_ui::PlayerMusicModeState>()
        .set_music_mode_show_album_art(false);
    app.global::<riina_ui::PlayerMusicModeState>()
        .set_music_mode_show_playlist(true);
    app.global::<riina_ui::PlayerLayoutState>()
        .set_control_bar_position_horizontal(0.25);
    app.global::<riina_ui::PlayerSidebarState>()
        .set_trailing_sidebar_width(320.0);
    app.global::<PlayerPreferences>()
        .set_subtitle_font_size_preference(38.0);
    transaction.player.session.save(app).unwrap();
    let saved = transaction.profile.read();
    // Session saves must leave explicit Music Mode profile choices alone.
    assert!(saved.music_mode_show_album_art);
    assert!(!saved.music_mode_show_playlist);
    assert_eq!(saved.control_bar_position_horizontal, 0.25);
    assert_eq!(saved.trailing_sidebar_width, 320);
    assert_eq!(saved.subtitle_font_size, 38.0);
}
