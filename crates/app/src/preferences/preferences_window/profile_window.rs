//! Construct the Settings view from the persisted profile before binding a player.

use super::{general_policy::Policy, owner::Owner};
use crate::{PlayerWindow, PreferencesWindow, preferences::model::PreferenceSection};
use riina_core::{Preferences, audio_preferences::OutputDevice};
use slint::ComponentHandle;
use std::path::Path;

pub(super) fn video_threads(window: &PreferencesWindow) -> Result<u32, &'static str> {
    window
        .get_video_threads_text()
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|threads| *threads <= i32::MAX as u32)
        .ok_or("Video decoder threads must be between 0 and 2,147,483,647")
}

pub(super) fn connect_actions(
    window: &PreferencesWindow,
    app: Option<&PlayerWindow>,
    path: &Path,
    open_log_viewer: impl Fn() + 'static,
) {
    let owner = Owner::new(window, app);
    let assrt = owner.clone();
    window.on_open_assrt_token_page(move || {
        if let Err(error) = crate::platform::open_http_url_in_browser("https://2.assrt.net/api/doc")
        {
            assrt.report(format!("Could not open Assrt API instructions: {error}"));
        }
    });
    super::key_bindings::connect(window, &owner);
    super::folder::register(window, &owner);
    let color = owner.clone();
    crate::platform::subtitle_color_panel::connect_preferences(window, move |error| {
        color.report(error);
    });
    crate::advanced_preferences::connect_actions(window, path, open_log_viewer, move |error| {
        owner.report(error);
    });
}

pub(super) fn create(
    path: &Path,
    baseline: &Preferences,
    available_devices: &[OutputDevice],
    clear_history: impl Fn() -> Result<usize, String> + 'static,
) -> Result<PreferencesWindow, String> {
    let window = PreferencesWindow::new().map_err(|error| error.to_string())?;
    crate::appearance_preferences::install_system_typography(&window);
    crate::platform::enable_preferences_mica(&window);
    crate::settings_search::connect(&window);
    crate::language_token_preferences::connect(&window);
    crate::osc_preferences::connect_preferences(&window);
    crate::utilities_preferences::connect(&window, path, clear_history);
    install_profile_controls(&window, baseline, available_devices)?;
    let advanced_settings_window = window.as_weak();
    window.on_advanced_settings_changed(move || {
        if let Some(window) = advanced_settings_window.upgrade() {
            window.invoke_settings_changed();
        }
    });
    Ok(window)
}

pub(super) fn install_profile_controls(
    window: &PreferencesWindow,
    baseline: &Preferences,
    available_devices: &[OutputDevice],
) -> Result<(), String> {
    install_general(window, baseline);
    install_display(window, baseline);
    install_subtitles(window, baseline);
    baseline.advanced.write_window(window);
    crate::audio_preferences::install_window(window, &baseline.audio, available_devices);
    baseline.hdr_tone_mapping.write_window(window);
    window.set_load_icc_profile(baseline.load_icc_profile);
    window.set_default_hdr_mode_enabled(baseline.hdr_enabled);
    window.set_hardware_decoder_index(crate::codec_preferences::index(baseline.hardware_decoder));
    window.set_video_threads_text(baseline.video_threads.to_string().into());
    window.set_volume_boost(baseline.volume_boost);
    baseline.controls.write_window(window);
    crate::key_binding_preferences::install_window(window, &baseline.key_bindings)?;
    baseline.network.write_window(window);
    baseline.network_cache.write_window(window);
    baseline.network_online_videos.write_window(window);
    Ok(())
}

fn install_general(window: &PreferencesWindow, saved: &Preferences) {
    Policy::from_profile(saved).install_window(window);
    super::general_policy::install_open_policy(window, saved);
    let general = window.global::<riina_ui::GeneralSettingsState>();
    general.set_record_recent_files(saved.record_recent_files);
    general.set_track_all_files_in_recent_open_menu(saved.track_all_files_in_recent_open_menu);
    general.set_auto_switch_to_music_mode(saved.auto_switch_to_music_mode);
    general.set_music_mode_show_playlist(saved.music_mode_show_playlist);
    general.set_music_mode_show_album_art(saved.music_mode_show_album_art);
    crate::screenshot_preferences::install_window(window, &saved.screenshot);
}

fn install_display(window: &PreferencesWindow, saved: &Preferences) {
    window.set_surface_material_title(crate::platform::surface_material_title().into());
    window.set_appearance_index(saved.effective_theme_material().index());
    window.set_always_float_on_top(saved.always_float_on_top);
    window.set_always_show_on_top_icon(saved.always_show_on_top_icon);
    window.set_compact_ui(saved.compact_ui);
    window.set_docked_controls(saved.docked_controls);
    window.set_edge_to_edge_video(saved.edge_to_edge_video);
    window.set_osc_position(i32::from(saved.osc_position));
    window.set_show_chapter_pos(saved.show_chapter_pos);
    window.set_disable_animations(saved.disable_animations);
    window.set_platform_macos(cfg!(target_os = "macos"));
    window.set_system_reduce_motion(crate::platform::system_reduce_motion_enabled());
    window.set_live_text_enabled(saved.live_text_enabled);
    crate::osd_preferences::Settings::get(saved).write_window(window);
    saved.control_bar.write_window(window);
    saved.liquid_glass.write_window(window);
    saved.picture_in_picture.write_window(window);
    saved.thumbnail_preview.write_window(window);
    saved.window_resize.write_window(window);
}

fn install_subtitles(window: &PreferencesWindow, saved: &Preferences) {
    let subtitles = window.global::<crate::SubtitleSettingsState>();
    subtitles
        .set_subtitle_encoding_options(crate::subtitle_other_preferences::encoding_options(window));
    subtitles.set_subtitle_font(saved.subtitle_font.clone().into());
    subtitles.set_subtitle_font_size_text(crate::subtitle_preferences::format_number(
        saved.subtitle_font_size as f32,
    ));
    subtitles.set_subtitle_ass_override_primary_index(saved.subtitle_ass_override.slider_index());
    subtitles.set_subtitle_ass_override_secondary_index(
        saved.secondary_subtitle_ass_override.slider_index(),
    );
    crate::subtitle_preferences::install_window_style(window, saved.subtitle_text_style);
    saved.subtitle_position.write_window(window);
    saved.subtitle_other.write_window(window);
    saved.subtitle_auto_load.write_window(window);
    saved.online_subtitles.write_window(window);
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_core::{
        ActionAfterLaunch, ThemeMaterial, network_preferences::Cache, screenshot::Format,
        subtitle_controls::AssOverrideLevel,
    };
    use riina_ui::{GeneralSettingsState, UtilityAction};
    use std::{cell::Cell, rc::Rc};

    #[test]
    fn settings_projects_profile_values_and_routes_utilities_without_a_player()
    -> Result<(), Box<dyn std::error::Error>> {
        i_slint_backend_testing::init_no_event_loop();
        let profile = riina_test_support::TempDir::new("profile-settings-window")?;
        let path = profile.path().join("preferences.json");
        let mut prefs = Preferences {
            video_threads: 6,
            action_after_launch: ActionAfterLaunch::OpenPanel,
            allow_multiple_windows: false,
            pause_when_open: true,
            playlist_auto_add: false,
            theme_material: Some(ThemeMaterial::Light),
            always_float_on_top: true,
            subtitle_ass_override: AssOverrideLevel::Force,
            subtitle_font: "Custom Font".into(),
            ..Preferences::default()
        };
        prefs.screenshot.encoding.format = Format::Webp;
        prefs.network_cache.enabled = !prefs.network_cache.enabled;
        let writer = crate::preference_store::Writer::new(path.clone(), prefs.clone());
        writer.save(&prefs)?;
        let saved = crate::preference_load::read_valid(&path)?.unwrap();
        let clears = Rc::new(Cell::new(0));
        let observed = clears.clone();
        let settings = create(&path, &saved, &[], move || {
            observed.set(observed.get() + 1);
            Ok(2)
        })?;
        assert_eq!(settings.get_video_threads_text(), "6");
        assert_eq!(
            settings.get_appearance_index(),
            ThemeMaterial::Light.index()
        );
        assert!(settings.get_always_float_on_top());
        let general = settings.global::<GeneralSettingsState>();
        assert!(!general.get_allow_multiple_windows());
        assert!(!general.get_playlist_auto_add());
        assert!(general.get_pause_when_open());
        assert_eq!(
            general.get_launch_action_index(),
            ActionAfterLaunch::OpenPanel.index()
        );
        assert_eq!(
            crate::screenshot_preferences::read_window(&settings, &saved.screenshot)?,
            saved.screenshot,
        );
        assert_eq!(
            Cache::read_window(&settings).map_err(|error| error.into_message())?,
            saved.network_cache,
        );
        let subtitles = settings.global::<crate::SubtitleSettingsState>();
        assert_eq!(subtitles.get_subtitle_font(), "Custom Font");
        assert_eq!(
            subtitles.get_subtitle_ass_override_primary_index(),
            AssOverrideLevel::Force.slider_index()
        );
        assert_eq!(clears.get(), 0);
        assert!(
            settings
                .invoke_utility_confirmed(UtilityAction::ClearHistory)
                .contains("2")
        );
        assert_eq!(clears.get(), 1);
        Ok(())
    }
}
