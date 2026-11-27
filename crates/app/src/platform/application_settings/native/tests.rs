use super::*;
use riina_core::{
    ActionAfterLaunch, ThemeMaterial, screenshot::Format, subtitle_controls::AssOverrideLevel,
};
use riina_ui::{GeneralSettingsState, KeyBindingSettingsState, NetworkSettingsState};
use std::cell::Cell;

fn credentials() -> Credentials {
    Credentials {
        open_subtitles: |_| panic!("Unexpected password store call"),
        assrt: |_| panic!("Unexpected Assrt password store call"),
    }
}

fn temp() -> (riina_test_support::TempDir, PathBuf) {
    i_slint_backend_testing::init_no_event_loop();
    let directory = riina_test_support::TempDir::new("application-settings").unwrap();
    let path = directory.path().join("preferences.json");
    (directory, path)
}

#[test]
fn settings_changes_before_playback_merge_with_an_independent_save() {
    let (_directory, path) = temp();
    let mut original = Preferences {
        video_threads: 6,
        ..Preferences::default()
    };
    original.screenshot.encoding.format = Format::Avif;
    original.screenshot.encoding.template = "%F-%P".into();
    crate::preference_store::Writer::new(path.clone(), original.clone())
        .save(&original)
        .unwrap();
    let owner = Rc::new(ApplicationSettings::default());
    let commits = Rc::new(Cell::new(0));
    let gain = riina_core::audio_preferences::ReplayGain::Album;
    let counter = commits.clone();
    let window = owner
        .create(
            &path,
            original.clone(),
            credentials(),
            || Ok(0),
            move |changes| {
                assert!(changes.controller.is_none());
                assert_eq!(changes.replay_gain.unwrap().mode, gain);
                assert_eq!(changes.subtitle_font.as_deref(), Some("Serif"));
                assert_eq!(
                    changes.key_bindings.unwrap().current_profile,
                    "Movist Default"
                );
                counter.set(counter.get() + 1);
            },
        )
        .unwrap();
    profile_window::connect_actions(&window, None, &path, || {});
    window.show().unwrap();
    let selected = window.as_weak();
    owner.window.replace(Some(window.clone_strong()));
    assert!(owner.is_visible());
    assert!(owner.show_visible(Some(5)).unwrap());
    assert_eq!(window.get_selected_pane(), 5);
    assert_eq!(selected.upgrade().unwrap().get_selected_pane(), 5);

    crate::preference_store::update(&path, &original, |latest| {
        latest.record_recent_files = false;
        latest.advanced.logging_enabled = true;
        latest.screenshot.destinations.copy_to_clipboard = true;
        Ok(())
    })
    .unwrap();
    let general = window.global::<GeneralSettingsState>();
    general.set_launch_action_index(ActionAfterLaunch::OpenPanel.index());
    general.set_show_preview(false);
    general.set_pause_when_open(true);
    window.set_video_threads_text("8".into());
    window
        .global::<riina_ui::AudioSettingsState>()
        .set_audio_replay_gain_index(2);
    window.set_appearance_index(ThemeMaterial::Dark.index());
    let subtitles = window.global::<crate::SubtitleSettingsState>();
    subtitles.set_subtitle_font("Serif".into());
    subtitles.set_subtitle_ass_override_primary_index(AssOverrideLevel::Force.slider_index());
    window
        .global::<KeyBindingSettingsState>()
        .set_key_binding_current_profile("Movist Default".into());
    window.invoke_settings_changed();
    let saved = crate::preference_load::read_valid(&path).unwrap().unwrap();
    assert!(
        window.get_profile_status().is_empty(),
        "{}",
        window.get_profile_status()
    );
    assert_eq!(commits.get(), 1);
    assert_eq!(saved.action_after_launch, ActionAfterLaunch::OpenPanel);
    assert!(saved.pause_when_open);
    assert_eq!(saved.video_threads, 8);
    assert_eq!(saved.audio.replay_gain_options().mode, gain);
    assert!(!saved.screenshot.show_preview);
    assert!(saved.screenshot.destinations.copy_to_clipboard);
    assert_eq!(saved.screenshot.encoding, original.screenshot.encoding);
    assert!(!saved.record_recent_files);
    assert!(saved.advanced.logging_enabled);
    assert_eq!(saved.subtitle_font, "Serif");
    assert_eq!(saved.subtitle_ass_override, AssOverrideLevel::Force);
    assert_eq!(saved.key_bindings.current_profile, "Movist Default");
    assert_eq!(saved.theme_material, ThemeMaterial::Dark);
    owner.hide();
    assert!(!window.window().is_visible());
    assert!(!owner.is_visible());
    assert!(!owner.show_visible(None).unwrap());
}

#[test]
fn invalid_edit_restores_the_form_and_keeps_the_existing_profile() {
    let (_directory, path) = temp();
    let owner = Rc::new(ApplicationSettings::default());
    let window = owner
        .create(
            &path,
            Preferences::default(),
            credentials(),
            || Ok(0),
            |_| panic!("Invalid settings cannot commit"),
        )
        .unwrap();
    let threads = window.get_video_threads_text();
    let network = window.global::<NetworkSettingsState>();
    let enabled = network.get_network_cache_enabled();
    network.set_network_cache_enabled(!enabled);
    window.set_video_threads_text("invalid".into());
    window.invoke_settings_changed();
    assert!(!path.exists());
    assert_eq!(window.get_video_threads_text(), threads);
    assert_eq!(network.get_network_cache_enabled(), enabled);
    assert!(
        window
            .get_profile_status()
            .contains("Video decoder threads")
    );
}

#[test]
fn credential_store_failure_keeps_the_previous_key_and_saves_other_changes() {
    let (_directory, path) = temp();
    let mut saved = Preferences::default();
    saved.online_subtitles.api_key = "existing-test-key".into();
    let calls = Credentials {
        open_subtitles: |_| Err("Keyring is unavailable".into()),
        ..credentials()
    };
    let owner = Rc::new(ApplicationSettings::default());
    let window = owner.create(&path, saved, calls, || Ok(0), |_| {}).unwrap();
    let subtitles = window.global::<crate::SubtitleSettingsState>();
    subtitles.set_open_subtitles_api_key("new-test-key".into());
    window.set_load_icc_profile(false);
    window.invoke_settings_changed();
    assert_eq!(subtitles.get_open_subtitles_api_key(), "existing-test-key");
    assert_eq!(window.get_profile_status(), "Keyring is unavailable");
    let json = std::fs::read_to_string(&path).unwrap();
    assert!(!json.contains("test-key"));
    assert!(!Preferences::parse(&json).unwrap().load_icc_profile);
}
