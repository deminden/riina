//! Project saved preferences before installing playback handlers.
use super::{
    media_actions::recent::{recent_open_menu_rows, refresh_welcome_history},
    video_controls::refresh_video_menu,
};
use crate::{PlayerWindow, WelcomeWindow};
use riina_core::{Preferences, RecentMedia};
use std::{cell::RefCell, path::Path, rc::Rc};

pub(super) struct SessionProjection {
    pub(super) latest_playback: crate::PlaybackStore,
    pub(super) system_media: crate::platform::SystemMediaSession,
    pub(super) appearance_timer: Rc<slint::Timer>,
}

pub(super) fn install_session(
    app: &PlayerWindow,
    prefs: &Preferences,
    prefs_path: &std::path::Path,
    native: Option<&crate::platform::NativePresentation>,
    welcome: Option<&WelcomeWindow>,
    recent_open_documents: &Rc<RefCell<Vec<riina_core::RecentMedia>>>,
) -> SessionProjection {
    install_platform(app, prefs);
    let latest_playback = crate::PlaybackStore::default();
    crate::about_window::connect(app, latest_playback.clone());
    super::lifecycle::install_pause_on_minimize(
        app,
        native.map(|presentation| presentation.host()),
        latest_playback.clone(),
    );
    crate::preferences::model::install_player_sections(prefs, app);
    crate::key_binding_preferences::install_player(
        app,
        &prefs.key_bindings,
        latest_playback.clone(),
    );
    let system_media = crate::platform::SystemMediaSession::new(app, latest_playback.clone());
    install_media(app, prefs, welcome, recent_open_documents);
    let appearance_timer = install_layout(app, prefs, prefs_path);
    SessionProjection {
        latest_playback,
        system_media,
        appearance_timer,
    }
}

fn install_platform(app: &PlayerWindow, prefs: &Preferences) {
    app.set_volume_boost(prefs.volume_boost);
    app.set_auto_switch_to_music_mode(prefs.auto_switch_to_music_mode);
    app.set_disable_animations(prefs.disable_animations);
    app.set_system_reduce_motion(crate::platform::system_reduce_motion_enabled());
    app.set_platform_macos(cfg!(target_os = "macos"));
    app.set_platform_windows(cfg!(target_os = "windows"));
    #[cfg(target_os = "windows")]
    {
        app.set_system_materials_enabled(crate::platform::native_materials_enabled());
        if let Some(accent) = crate::platform::windows_accent_color() {
            app.set_windows_accent_color(accent);
        }
        if let Some(foreground) = crate::platform::windows_accent_foreground() {
            app.set_windows_accent_foreground(foreground);
        }
    }
    crate::appearance_preferences::install_system_typography(app);
    app.set_music_mode_show_playlist(prefs.music_mode_show_playlist);
    app.set_music_mode_show_album_art(prefs.music_mode_show_album_art);
}

fn install_media(
    app: &PlayerWindow,
    prefs: &Preferences,
    welcome: Option<&WelcomeWindow>,
    recent_open_documents: &RefCell<Vec<RecentMedia>>,
) {
    app.set_hardware_decoder_index(crate::codec_preferences::index(prefs.hardware_decoder));
    app.set_subtitle_ass_override_index(crate::subtitle_preferences::index(
        prefs.subtitle_ass_override,
    ));
    app.set_secondary_subtitle_ass_override_index(crate::subtitle_preferences::index(
        prefs.secondary_subtitle_ass_override,
    ));
    app.set_subtitle_font(prefs.subtitle_font.clone().into());
    app.set_subtitle_font_size_preference(prefs.subtitle_font_size as f32);
    crate::subtitle_preferences::install_style(app, prefs.subtitle_text_style);
    if let Some(welcome) = welcome {
        welcome.set_app_version(env!("CARGO_PKG_VERSION").into());
        refresh_welcome_history(welcome, &prefs.recent_media, prefs.resume_last_position);
    }
    app.set_video_aspect_presets(slint::ModelRc::new(slint::VecModel::from(
        riina_core::video_controls::ASPECT_PRESETS
            .map(slint::SharedString::from)
            .to_vec(),
    )));
    crate::screenshot_preferences::install(app, &prefs.screenshot);
    app.set_pause_when_open(prefs.pause_when_open);
    app.set_play_when_entering_fullscreen(prefs.play_when_entering_fullscreen);
    app.set_pause_when_leaving_fullscreen(prefs.pause_when_leaving_fullscreen);
    app.set_pause_when_inactive(prefs.pause_when_inactive);
    app.set_pause_when_minimized(prefs.pause_when_minimized);
    app.set_pause_when_goes_to_sleep(prefs.pause_when_goes_to_sleep);
    app.set_quit_when_no_opened_window(prefs.quit_when_no_opened_window);
    app.set_keep_open_on_file_end(prefs.keep_open_on_file_end);
    app.set_playlist_auto_play_next(prefs.playlist_auto_play_next);
    app.set_playlist_auto_add(prefs.playlist_auto_add);
    app.set_playlist_show_metadata(prefs.playlist_show_metadata);
    app.set_playlist_show_metadata_in_music_mode(prefs.playlist_show_metadata_in_music_mode);
    app.set_auto_repeat(prefs.auto_repeat);
    app.set_default_repeat_mode_index(prefs.default_repeat_mode.index());
    app.set_fullscreen_when_open(prefs.fullscreen_when_open);
    app.set_resume_last_position(prefs.resume_last_position);
    app.set_record_playback_history(prefs.record_playback_history);
    app.set_record_recent_files(prefs.record_recent_files);
    app.set_track_all_files_in_recent_open_menu(prefs.track_all_files_in_recent_open_menu);
    app.set_recent_open_menu_rows(slint::ModelRc::new(slint::VecModel::from(
        recent_open_menu_rows(&recent_open_documents.borrow()),
    )));
    app.set_launch_action_index(prefs.action_after_launch.index());
}

fn install_layout(app: &PlayerWindow, prefs: &Preferences, prefs_path: &Path) -> Rc<slint::Timer> {
    app.set_leading_sidebar_width(prefs.leading_sidebar_width as f32);
    app.set_trailing_sidebar_width(prefs.trailing_sidebar_width as f32);
    app.set_settings_at_leading(prefs.settings_at_leading);
    app.set_playlist_at_leading(prefs.playlist_at_leading);
    app.set_plugins_at_leading(prefs.plugins_at_leading);
    app.set_edge_to_edge_video(prefs.edge_to_edge_video);
    app.set_docked_controls(prefs.docked_controls);
    app.set_unlock_window_aspect_ratio(prefs.unlock_window_aspect_ratio);
    app.set_compact_ui(prefs.compact_ui);
    app.set_always_float_on_top(prefs.always_float_on_top);
    app.set_always_show_on_top_icon(prefs.always_show_on_top_icon);
    let appearance = prefs.effective_theme_material();
    app.set_appearance_mode(appearance.index());
    app.set_surface_material_title(crate::platform::surface_material_title().into());
    app.set_dark_theme(appearance.dark_for(prefs.dark));
    let appearance_timer = crate::appearance_preferences::connect(app);
    app.set_osc_position(i32::from(prefs.osc_position));
    app.set_control_bar_position_horizontal(prefs.control_bar_position_horizontal as f32);
    app.set_control_bar_position_vertical(prefs.control_bar_position_vertical as f32);
    app.set_show_chapter_pos(prefs.show_chapter_pos);
    crate::sidebar_preferences::connect(app, prefs_path.to_path_buf(), prefs.clone());
    crate::layout_preferences::connect(app, prefs_path.to_path_buf(), prefs.clone());
    refresh_video_menu(app);
    app.set_video_crop_presets(slint::ModelRc::new(slint::VecModel::from(
        riina_core::crop::LABELS
            .map(slint::SharedString::from)
            .to_vec(),
    )));
    app.set_primary_shortcut_is_meta(cfg!(target_os = "macos"));
    app.set_current_file_manager_name(
        if cfg!(target_os = "macos") {
            "Finder"
        } else if cfg!(target_os = "windows") {
            "Explorer"
        } else {
            "File Manager"
        }
        .into(),
    );
    appearance_timer
}
