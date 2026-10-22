//! Refresh controls from the owning player.
use crate::PlayerWindow;

pub(super) fn refresh(window: &crate::PreferencesWindow, app: &PlayerWindow) {
    window.set_surface_material_title(crate::platform::surface_material_title().into());
    if let Ok(settings) = crate::online_subtitles_preferences::current_player(app) {
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
    window.set_load_icc_profile(app.get_load_icc_profile());
    window.set_default_hdr_mode_enabled(app.get_default_hdr_mode_enabled());
    window.set_disable_animations(app.get_disable_animations());
    window.set_platform_macos(cfg!(target_os = "macos"));
    window.set_system_reduce_motion(crate::platform::system_reduce_motion_enabled());
    window.set_live_text_available(app.get_live_text_available());
    if let Ok(controls) = crate::control_preferences::current_player(app) {
        crate::control_preferences::install_window(window, &controls);
    }
    if let Ok(settings) = crate::key_binding_preferences::current_player(app) {
        let _ = crate::key_binding_preferences::refresh(window, &settings);
    }
    window.set_subtitle_encoding_options(crate::subtitle_other_preferences::encoding_options());
    window
        .set_subtitle_auto_load_mode_options(crate::subtitle_auto_load_preferences::mode_options());
    window.set_subtitle_font(app.get_subtitle_font());
    window.set_subtitle_font_size_text(crate::subtitle_preferences::format_number(
        app.get_subtitle_font_size_preference(),
    ));
    if let Ok(style) = crate::subtitle_preferences::current_style(app) {
        crate::subtitle_preferences::install_window_style(window, style);
    }
    window.set_pause_when_open(app.get_pause_when_open());
    window.set_play_when_entering_fullscreen(app.get_play_when_entering_fullscreen());
    window.set_pause_when_leaving_fullscreen(app.get_pause_when_leaving_fullscreen());
    window.set_pause_when_inactive(app.get_pause_when_inactive());
    window.set_pause_when_minimized(app.get_pause_when_minimized());
    window.set_pause_when_goes_to_sleep(app.get_pause_when_goes_to_sleep());
    window.set_pause_policies_available(cfg!(target_os = "macos"));
    window.set_quit_when_no_opened_window(app.get_quit_when_no_opened_window());
    window.set_keep_open_on_file_end(app.get_keep_open_on_file_end());
    window.set_playlist_auto_play_next(app.get_playlist_auto_play_next());
    window.set_playlist_auto_add(app.get_playlist_auto_add());
    window.set_playlist_show_metadata(app.get_playlist_show_metadata());
    window.set_playlist_show_metadata_in_music_mode(app.get_playlist_show_metadata_in_music_mode());
    window.set_auto_repeat(app.get_auto_repeat());
    window.set_default_repeat_mode_index(app.get_default_repeat_mode_index());
    window.set_fullscreen_when_open(app.get_fullscreen_when_open());
    window.set_resume_last_position(app.get_resume_last_position());
    window.set_record_playback_history(app.get_record_playback_history());
    window.set_record_recent_files(app.get_record_recent_files());
    window.set_track_all_files_in_recent_open_menu(app.get_track_all_files_in_recent_open_menu());
    window.set_launch_action_index(app.get_launch_action_index());
    window.set_auto_switch_to_music_mode(app.get_auto_switch_to_music_mode());
    window.set_music_mode_show_playlist(app.get_music_mode_show_playlist());
    window.set_music_mode_show_album_art(app.get_music_mode_show_album_art());
    window.set_osc_position(app.get_osc_position());
    window.set_show_chapter_pos(app.get_show_chapter_pos());
    if let Ok(settings) = crate::pip_preferences::current_player(app) {
        crate::pip_preferences::install_preferences(window, settings);
    }
    window.set_hardware_decoder_index(app.get_hardware_decoder_index());
    window.set_subtitle_ass_override_primary_index(app.get_subtitle_ass_override_index());
    window
        .set_subtitle_ass_override_secondary_index(app.get_secondary_subtitle_ass_override_index());
    if let Ok(position) = crate::subtitle_position_preferences::current_player(app) {
        crate::subtitle_position_preferences::install_window(window, position);
    }
    if let Ok(other) = crate::subtitle_other_preferences::current_player(app) {
        let _ = crate::subtitle_other_preferences::install_window(window, &other);
    }
    if let Ok(auto_load) = crate::subtitle_auto_load_preferences::current_player(app) {
        crate::subtitle_auto_load_preferences::install_window(window, &auto_load);
    }
    window.set_volume_boost(app.get_volume_boost());
    window.set_appearance_index(app.get_appearance_mode());
    window.set_always_float_on_top(app.get_always_float_on_top());
    window.set_always_show_on_top_icon(app.get_always_show_on_top_icon());
    if let Ok(settings) = crate::window_resize_preferences::from_player(app) {
        crate::window_resize_preferences::install_preferences(window, settings);
    }
    window.set_compact_ui(app.get_compact_ui());
    window.set_docked_controls(app.get_docked_controls());
    window.set_edge_to_edge_video(app.get_edge_to_edge_video());
    let index = ["png", "jpg", "jpeg", "webp", "jxl"]
        .iter()
        .position(|format| *format == app.get_screenshot_format().as_str())
        .map_or(-1, |index| index as i32);
    window.set_format_index(index);
    window.set_include_subtitles(app.get_screenshot_include_subtitles());
    window.set_show_preview(app.get_screenshot_show_preview());
    window.set_save_to_file(app.get_screenshot_save_to_file());
    window.set_copy_to_clipboard(app.get_screenshot_copy_to_clipboard());
    window.set_custom_directory(app.get_screenshot_custom_directory());
    window.set_directory(if app.get_screenshot_custom_directory() {
        app.get_screenshot_directory()
    } else {
        dirs::picture_dir()
            .map(|path| path.join("Screenshots").to_string_lossy().into_owned())
            .unwrap_or_default()
            .into()
    });
}
