//! Menu callbacks retain only weak UI handles; UiSession remains the engine owner.
use crate::{NativeVideoHost, PlayerWindow};
use slint::ComponentHandle;

macro_rules! forward {
    ($host:expr, $app:ident, $callback:ident, || $body:block) => {
        forward!($host, $app, $callback, | | $body);
    };
    ($host:expr, $app:ident, $callback:ident, |$($arg:ident),*| $body:block) => {
        let weak = $app.as_weak();
        $host.$callback(move |$($arg),*| {
            if let Some($app) = weak.upgrade() {
                $body
            }
        });
    };
}

pub(super) fn install(host: &NativeVideoHost, app: &PlayerWindow) {
    forward!(host, app, on_open_media_file, || {
        app.invoke_open_file();
    });
    forward!(host, app, on_open_in_new_window, || {
        app.invoke_open_in_new_window();
    });
    forward!(host, app, on_reveal_current_media, || {
        app.invoke_reveal_current_media();
    });
    forward!(host, app, on_delete_current_media, || {
        app.invoke_delete_current_media();
    });
    forward!(host, app, on_show_playback_history, || {
        app.invoke_show_playback_history();
    });
    forward!(host, app, on_save_current_playlist, || {
        app.invoke_save_current_playlist();
    });
    forward!(host, app, on_save_downloaded_subtitle, || {
        app.invoke_save_downloaded_subtitle();
    });
    forward!(host, app, on_show_about, || {
        app.invoke_show_about();
    });
    let menu_app = app.as_weak();
    host.on_open_help_link(move |destination| {
        let Some(app) = menu_app.upgrade() else {
            return;
        };
        let Some(url) = crate::player_ui::help_menu_url(destination.as_str()) else {
            return;
        };
        if let Err(error) = crate::platform::open_http_url_in_browser(url) {
            app.set_status(format!("Could not open the Help link: {error}").into());
        }
    });
    forward!(host, app, on_dump_debug_info, || {
        app.invoke_dump_debug_info();
    });
    forward!(host, app, on_quit_riina, || {
        app.invoke_quit_riina();
    });
    let menu_host = host.as_weak();
    forward!(host, app, on_hide_riina, || {
        app.invoke_hide_riina();
    });
    host.on_minimize_window(move || {
        if let Some(host) = menu_host.upgrade() {
            crate::platform::minimize_window(host.window());
        }
    });
    let menu_host = host.as_weak();
    host.on_zoom_window(move || {
        if let Some(host) = menu_host.upgrade() {
            crate::platform::toggle_zoom_window(host.window());
        }
    });
    let menu_host = host.as_weak();
    host.on_bring_all_to_front(move || {
        if let Some(host) = menu_host.upgrade()
            && let Err(error) =
                super::super::bring_to_front::bring_all_riina_windows_to_front(host.window())
        {
            eprintln!("Could not bring all Riina windows forward: {error}");
        }
    });
    forward!(host, app, on_close_player, || {
        app.invoke_close_player();
    });
    forward!(host, app, on_open_url_dialog, || {
        app.invoke_open_url_dialog();
    });
    forward!(host, app, on_open_recent_path, |path| {
        app.invoke_open_recent_path(path);
    });
    forward!(host, app, on_clear_open_recent, || {
        app.invoke_clear_open_recent();
    });
    forward!(host, app, on_toggle_music_mode, || {
        app.invoke_toggle_music_mode();
    });
    forward!(host, app, on_toggle_video_reflection, |horizontal| {
        app.invoke_toggle_video_reflection(horizontal);
    });
    forward!(host, app, on_select_menu_playlist_item, |token| {
        app.invoke_playlist_action(12, token);
    });
    forward!(host, app, on_select_menu_chapter_item, |token| {
        app.invoke_select_item(3, token);
    });
    forward!(host, app, on_show_video_panel, || {
        app.invoke_toggle_settings_pane(6);
    });
    forward!(host, app, on_show_plugins_panel, || {
        app.invoke_toggle_plugins_pane();
    });
    forward!(host, app, on_cycle_audio_tracks, || {
        app.invoke_cycle_audio_track();
    });
    forward!(host, app, on_select_menu_audio_track, |token| {
        app.invoke_select_item(1, token);
    });
    forward!(host, app, on_load_external_audio, || {
        app.invoke_browse_files(1);
    });
    forward!(host, app, on_select_audio_device, |device| {
        app.invoke_set_audio_device(device);
    });
    forward!(host, app, on_adjust_volume, |delta| {
        app.invoke_adjust_volume(delta);
    });
    forward!(host, app, on_toggle_mute, || {
        app.invoke_toggle_mute();
    });
    forward!(host, app, on_show_audio_panel, || {
        app.invoke_toggle_settings_pane(1);
    });
    forward!(host, app, on_adjust_audio_delay, |delta| {
        app.invoke_adjust_audio_delay(delta);
    });
    forward!(host, app, on_reset_audio_delay, || {
        app.invoke_reset_audio_delay();
    });
    forward!(host, app, on_cycle_subtitles, || {
        app.invoke_cycle_subtitle_track();
    });
    forward!(host, app, on_toggle_subtitle_visibility, || {
        app.invoke_toggle_subtitle_visibility();
    });
    forward!(host, app, on_show_subtitles_panel, || {
        app.invoke_toggle_settings_pane(2);
    });
    forward!(host, app, on_select_subtitle_track, |token| {
        app.invoke_select_item(2, token);
    });
    forward!(host, app, on_select_secondary_subtitle, |token| {
        app.invoke_select_item(5, token);
    });
    forward!(host, app, on_toggle_secondary_subtitle_visibility, || {
        app.invoke_toggle_secondary_subtitle_visibility();
    });
    forward!(host, app, on_load_external_subtitle, || {
        app.invoke_browse_files(2);
    });
    forward!(host, app, on_find_online_subtitles, || {
        app.invoke_find_online_subtitles();
    });
    forward!(host, app, on_find_online_subtitles_with_provider, |index| {
        app.invoke_find_online_subtitles_with_provider(index);
    });
    forward!(host, app, on_set_subtitle_encoding, |encoding| {
        app.invoke_set_subtitle_encoding(encoding);
    });
    forward!(host, app, on_multiply_subtitle_scale, |factor| {
        app.invoke_multiply_subtitle_scale(factor);
    });
    forward!(host, app, on_reset_subtitle_scale, || {
        app.invoke_set_subtitle_scale_slider(0.0);
    });
    forward!(host, app, on_adjust_subtitle_delay, |delta| {
        app.invoke_adjust_subtitle_delay(delta);
    });
    forward!(host, app, on_reset_subtitle_delay, || {
        app.invoke_reset_subtitle_delay();
    });
    forward!(host, app, on_choose_subtitle_font, || {
        app.invoke_choose_subtitle_font();
    });
    forward!(host, app, on_cycle_video_track, || {
        app.invoke_cycle_video_track();
    });
    forward!(host, app, on_select_video_track, |token| {
        app.invoke_select_item(6, token);
    });
    forward!(host, app, on_fit_window_to_video, || {
        app.invoke_fit_window_to_video();
    });
    forward!(host, app, on_resize_window_command, |command| {
        app.invoke_resize_window_command(command);
    });
    forward!(host, app, on_toggle_fullscreen, || {
        app.invoke_toggle_fullscreen();
    });
    forward!(host, app, on_toggle_float_on_top, || {
        app.set_always_float_on_top(!app.get_always_float_on_top());
    });
    forward!(host, app, on_toggle_window_aspect_lock, || {
        app.set_unlock_window_aspect_ratio(!app.get_unlock_window_aspect_ratio());
    });
    forward!(host, app, on_toggle_live_text, || {
        app.invoke_toggle_live_text();
    });
}

pub(super) fn install_playback(host: &NativeVideoHost, app: &PlayerWindow) {
    forward!(host, app, on_set_menu_video_crop, |index| {
        app.invoke_set_menu_video_crop(index);
    });
    forward!(host, app, on_toggle_delogo, || {
        app.invoke_toggle_delogo();
    });
    forward!(host, app, on_toggle_saved_video_filter, |filter| {
        app.invoke_toggle_saved_video_filter(filter);
    });
    forward!(host, app, on_open_video_filters, || {
        app.invoke_open_video_filters();
    });
    forward!(host, app, on_toggle_saved_audio_filter, |filter| {
        app.invoke_toggle_saved_audio_filter(filter);
    });
    forward!(host, app, on_open_audio_filters, || {
        app.invoke_open_audio_filters();
    });
    forward!(host, app, on_set_menu_video_rotation, |degrees| {
        app.invoke_set_menu_video_rotation(degrees);
    });
    forward!(host, app, on_set_menu_video_aspect, |index| {
        app.invoke_set_menu_video_aspect(index);
    });
    forward!(host, app, on_toggle_deinterlace, || {
        app.invoke_toggle_deinterlace();
    });
    forward!(host, app, on_take_screenshot, || {
        app.invoke_take_screenshot();
    });
    forward!(host, app, on_open_screenshot_folder, || {
        app.invoke_open_screenshot_folder();
    });
    forward!(host, app, on_open_preferences, || {
        app.invoke_open_preferences();
    });
    let plugin_manager_app = app.as_weak();
    host.on_manage_plugins(move || {
        if let Some(app) = plugin_manager_app.upgrade() {
            app.set_preferences_target_pane(9);
            app.invoke_open_preferences();
        }
    });
    forward!(host, app, on_reload_plugins, || {
        app.invoke_reload_plugins();
    });
    forward!(host, app, on_activate_plugin_menu_item, |token| {
        app.invoke_activate_plugin_menu_item(token);
    });
    forward!(host, app, on_open_jump_time_dialog, || {
        app.invoke_open_jump_time_dialog();
    });
    forward!(host, app, on_toggle_pause, || {
        app.invoke_toggle_pause();
    });
    forward!(host, app, on_stop, || {
        app.invoke_stop();
    });
    forward!(host, app, on_seek_relative, |seconds| {
        app.invoke_seek_relative(seconds);
    });
    forward!(host, app, on_seek, |seconds| {
        app.invoke_seek(seconds);
    });
    forward!(host, app, on_frame_step, || {
        app.invoke_frame_step();
    });
    forward!(host, app, on_frame_back_step, || {
        app.invoke_frame_back_step();
    });
    forward!(host, app, on_set_speed, |factor| {
        app.invoke_speed(factor);
    });
    forward!(host, app, on_reset_speed, || {
        app.invoke_reset_speed();
    });
    forward!(host, app, on_cycle_ab_loop, || {
        app.invoke_cycle_ab_loop();
    });
    forward!(host, app, on_toggle_file_loop, || {
        app.invoke_toggle_file_loop();
    });
    forward!(host, app, on_cycle_playlist_loop, || {
        app.invoke_playlist_action(5, "".into());
    });
    forward!(host, app, on_playlist_next, || {
        app.invoke_playlist_next();
    });
    forward!(host, app, on_playlist_previous, || {
        app.invoke_playlist_previous();
    });
    forward!(host, app, on_show_playlist_panel, || {
        app.invoke_toggle_playlist_pane(4);
    });
    forward!(host, app, on_show_chapters_panel, || {
        app.invoke_toggle_playlist_pane(3);
    });
    forward!(host, app, on_adjust_chapter, |delta| {
        app.invoke_adjust_chapter(delta);
    });
}
