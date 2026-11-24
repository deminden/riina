//! Refresh controls from the owning player.
use super::general_policy::Policy;
use crate::{PlayerWindow, preferences::model::PreferenceSection};
use slint::ComponentHandle;

pub(super) fn refresh(window: &crate::PreferencesWindow, app: &PlayerWindow) {
    let player_preferences = app.global::<riina_ui::PlayerPreferences>();
    let layout = app.global::<riina_ui::PlayerLayoutState>();
    let appearance = app.global::<riina_ui::PlayerAppearance>();
    let music_mode = app.global::<riina_ui::PlayerMusicModeState>();
    let general = window.global::<riina_ui::GeneralSettingsState>();
    let subtitles = window.global::<crate::SubtitleSettingsState>();
    window.set_surface_material_title(crate::platform::surface_material_title().into());
    if let Ok(settings) = crate::online_subtitles_preferences::current_player(app) {
        crate::online_subtitles_preferences::install_window_catalog(window, app);
        crate::online_subtitles_preferences::install_window(window, &settings);
    }
    crate::osd_preferences::current_player(app).install_preferences(window);
    crate::osc_preferences::install_preferences(
        window,
        crate::osc_preferences::current_player(app),
    );
    crate::liquid_glass_preferences::install_preferences(
        window,
        crate::liquid_glass_preferences::current_player(app),
    );
    crate::network_cache_preferences::refresh_window(window, app);
    crate::network_ytdl_preferences::refresh_window(window, app);
    if let Ok(settings) = crate::thumbnail_preferences::current_player(app) {
        crate::thumbnail_preferences::install_window(window, &settings);
    }
    window.set_live_text_enabled(app.get_live_text_enabled());
    window.set_load_icc_profile(player_preferences.get_load_icc_profile());
    window.set_default_hdr_mode_enabled(player_preferences.get_default_hdr_mode_enabled());
    window.set_disable_animations(player_preferences.get_disable_animations());
    window.set_platform_macos(cfg!(target_os = "macos"));
    window.set_system_reduce_motion(crate::platform::system_reduce_motion_enabled());
    window.set_live_text_available(app.get_live_text_available());
    if let Ok(controls) = crate::control_preferences::current_player(app) {
        crate::control_preferences::install_window(window, &controls);
    }
    if let Ok(settings) = crate::key_binding_preferences::current_player(app) {
        settings.write_window(window);
    }
    subtitles
        .set_subtitle_encoding_options(crate::subtitle_other_preferences::encoding_options(window));
    subtitles.set_subtitle_font(
        app.global::<riina_ui::PlaybackSubtitleState>()
            .get_subtitle_font(),
    );
    subtitles.set_subtitle_font_size_text(crate::subtitle_preferences::format_number(
        player_preferences.get_subtitle_font_size_preference(),
    ));
    if let Ok(style) = crate::subtitle_preferences::current_style(app) {
        crate::subtitle_preferences::install_window_style(window, style);
    }
    Policy::from_player(app).install_window(window);
    general.set_record_recent_files(player_preferences.get_record_recent_files());
    general.set_track_all_files_in_recent_open_menu(
        player_preferences.get_track_all_files_in_recent_open_menu(),
    );
    general.set_auto_switch_to_music_mode(music_mode.get_auto_switch_to_music_mode());
    general.set_music_mode_show_playlist(music_mode.get_music_mode_show_playlist());
    general.set_music_mode_show_album_art(music_mode.get_music_mode_show_album_art());
    window.set_osc_position(layout.get_osc_position());
    window.set_show_chapter_pos(player_preferences.get_show_chapter_pos());
    if let Ok(settings) = crate::pip_preferences::current_player(app) {
        crate::pip_preferences::install_preferences(window, settings);
    }
    window.set_hardware_decoder_index(player_preferences.get_hardware_decoder_index());
    subtitles.set_subtitle_ass_override_primary_index(
        player_preferences.get_subtitle_ass_override_index(),
    );
    subtitles.set_subtitle_ass_override_secondary_index(
        player_preferences.get_secondary_subtitle_ass_override_index(),
    );
    if let Ok(position) = crate::subtitle_position_preferences::current_player(app) {
        crate::subtitle_position_preferences::install_window(window, position);
    }
    if let Ok(other) = crate::subtitle_other_preferences::current_player(app) {
        other.write_window(window);
    }
    if let Ok(auto_load) = crate::subtitle_auto_load_preferences::current_player(app) {
        crate::subtitle_auto_load_preferences::install_window(window, &auto_load);
    }
    window.set_volume_boost(player_preferences.get_volume_boost());
    window.set_appearance_index(appearance.get_appearance_mode());
    window.set_always_float_on_top(layout.get_always_float_on_top());
    window.set_always_show_on_top_icon(layout.get_always_show_on_top_icon());
    if let Ok(settings) = crate::window_resize_preferences::from_player(app) {
        crate::window_resize_preferences::install_preferences(window, settings);
    }
    window.set_compact_ui(appearance.get_compact_ui());
    window.set_docked_controls(layout.get_docked_controls());
    window.set_edge_to_edge_video(layout.get_edge_to_edge_video());
    if let Ok(settings) = crate::screenshot_preferences::current(app) {
        crate::screenshot_preferences::install_window(window, &settings);
    }
}
