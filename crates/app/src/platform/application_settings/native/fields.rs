//! Read an edit to the saved profile when there is no playback engine.
use crate::preferences_window::{general_policy, profile_window};
use crate::{PreferencesWindow, SubtitleSettingsState, preferences::model::PreferenceSection};
use riina_core::{
    Preferences, ThemeMaterial, advanced_preferences, audio_preferences, control_preferences,
    hdr_preferences, key_bindings, liquid_glass, network_preferences, online_subtitles,
    osc_preferences, pip_preferences, subtitle_controls, thumbnail_preferences, window_resize,
};
use riina_ui::GeneralSettingsState;
use slint::ComponentHandle;

pub(super) fn read(
    window: &PreferencesWindow,
    previous: &Preferences,
) -> Result<Preferences, String> {
    let mut candidate = previous.clone();
    read_general(window, &mut candidate)?;
    read_display(window, &mut candidate)?;
    read_subtitles(window, &mut candidate)?;
    read_playback(window, &mut candidate)?;
    Ok(candidate)
}

fn read_general(window: &PreferencesWindow, prefs: &mut Preferences) -> Result<(), String> {
    general_policy::read_profile(window, prefs)?;
    let general = window.global::<GeneralSettingsState>();
    prefs.allow_multiple_windows = general.get_allow_multiple_windows();
    prefs.group_simultaneous_opens_in_playlist = general.get_group_simultaneous_opens_in_playlist();
    prefs.allow_duplicate_players = general.get_allow_duplicate_players();
    prefs.record_recent_files = general.get_record_recent_files();
    prefs.track_all_files_in_recent_open_menu = general.get_track_all_files_in_recent_open_menu();
    prefs.auto_switch_to_music_mode = general.get_auto_switch_to_music_mode();
    prefs.music_mode_show_playlist = general.get_music_mode_show_playlist();
    prefs.music_mode_show_album_art = general.get_music_mode_show_album_art();
    prefs.screenshot = crate::screenshot_preferences::read_window(window, &prefs.screenshot)?;
    Ok(())
}

fn read_display(window: &PreferencesWindow, prefs: &mut Preferences) -> Result<(), String> {
    prefs.theme_material = ThemeMaterial::from_index(window.get_appearance_index())
        .ok_or("Choose a valid appearance")?;
    prefs.always_float_on_top = window.get_always_float_on_top();
    prefs.always_show_on_top_icon = window.get_always_show_on_top_icon();
    prefs.compact_ui = window.get_compact_ui();
    if !prefs.edge_to_edge_video {
        prefs.docked_controls = window.get_docked_controls();
    }
    prefs.osc_position = window.get_osc_position().clamp(0, 2) as u8;
    prefs.show_chapter_pos = window.get_show_chapter_pos();
    prefs.disable_animations = window.get_disable_animations();
    prefs.live_text_enabled = window.get_live_text_enabled();
    read_section::<crate::osd_preferences::Settings>(window, prefs)?;
    read_section::<osc_preferences::Settings>(window, prefs)?;
    read_section::<liquid_glass::SurfaceSettings>(window, prefs)?;
    read_section::<pip_preferences::Settings>(window, prefs)?;
    read_section::<thumbnail_preferences::Settings>(window, prefs)?;
    read_section::<window_resize::Settings>(window, prefs)
}

fn read_subtitles(window: &PreferencesWindow, prefs: &mut Preferences) -> Result<(), String> {
    let subtitles = window.global::<SubtitleSettingsState>();
    let font = subtitles.get_subtitle_font();
    if !subtitle_controls::valid_font_name(font.trim()) {
        return Err("Invalid subtitle font".into());
    }
    prefs.subtitle_font = font.trim().into();
    prefs.subtitle_font_size =
        crate::subtitle_preferences::font_size(subtitles.get_subtitle_font_size_text().as_str())?;
    prefs.subtitle_text_style = crate::subtitle_preferences::current_window_style(window)?;
    prefs.subtitle_ass_override = subtitle_controls::AssOverrideLevel::from_slider_index(
        subtitles.get_subtitle_ass_override_primary_index(),
    )
    .ok_or("Invalid primary ASS subtitle override")?;
    prefs.secondary_subtitle_ass_override = subtitle_controls::AssOverrideLevel::from_slider_index(
        subtitles.get_subtitle_ass_override_secondary_index(),
    )
    .ok_or("Invalid secondary ASS subtitle override")?;
    read_section::<subtitle_controls::PositionPreferences>(window, prefs)?;
    read_section::<subtitle_controls::OtherPreferences>(window, prefs)?;
    read_section::<subtitle_controls::AutoLoadPreferences>(window, prefs)?;
    read_section::<online_subtitles::Preferences>(window, prefs)
}

fn read_playback(window: &PreferencesWindow, prefs: &mut Preferences) -> Result<(), String> {
    prefs.load_icc_profile = window.get_load_icc_profile();
    prefs.hdr_enabled = window.get_default_hdr_mode_enabled();
    prefs.hardware_decoder =
        crate::codec_preferences::hardware_decoder(window.get_hardware_decoder_index())
            .ok_or("Invalid hardware decoder selection")?;
    prefs.video_threads = profile_window::video_threads(window)?;
    prefs.volume_boost = window.get_volume_boost();
    if !prefs.volume_boost {
        prefs.volume = prefs.volume.min(100.0);
    }
    read_section::<hdr_preferences::Settings>(window, prefs)?;
    read_section::<audio_preferences::Settings>(window, prefs)?;
    read_section::<network_preferences::Connection>(window, prefs)?;
    read_section::<network_preferences::Cache>(window, prefs)?;
    read_section::<network_preferences::OnlineVideos>(window, prefs)?;
    read_section::<advanced_preferences::Settings>(window, prefs)?;
    read_section::<control_preferences::Settings>(window, prefs)?;
    read_section::<key_bindings::Settings>(window, prefs)
}

fn read_section<S: PreferenceSection>(
    window: &PreferencesWindow,
    prefs: &mut Preferences,
) -> Result<(), String> {
    S::read_window(window)
        .map_err(|error| error.into_message())?
        .put(prefs);
    Ok(())
}
