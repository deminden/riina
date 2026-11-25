//! Project saved preferences before installing playback handlers.
use super::video_controls::refresh_video_menu;
use crate::PlayerWindow;
use riina_core::Preferences;
use slint::ComponentHandle;
use std::{path::Path, rc::Rc};

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
) -> SessionProjection {
    install_platform(app, prefs);
    let latest_playback = crate::PlaybackStore::default();
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
    install_media(app, prefs);
    let appearance_timer = install_layout(app, prefs, prefs_path);
    SessionProjection {
        latest_playback,
        system_media,
        appearance_timer,
    }
}

fn install_platform(app: &PlayerWindow, prefs: &Preferences) {
    app.global::<riina_ui::PlayerPreferences>()
        .set_volume_boost(prefs.volume_boost);
    app.global::<riina_ui::PlayerMusicModeState>()
        .set_auto_switch_to_music_mode(prefs.auto_switch_to_music_mode);
    app.global::<riina_ui::PlayerPreferences>()
        .set_disable_animations(prefs.disable_animations);
    crate::platform::configure_player_appearance(app);
    crate::appearance_preferences::install_system_typography(app);
    app.global::<riina_ui::PlayerMusicModeState>()
        .set_music_mode_show_playlist(prefs.music_mode_show_playlist);
    app.global::<riina_ui::PlayerMusicModeState>()
        .set_music_mode_playlist_preference(prefs.music_mode_show_playlist);
    app.global::<riina_ui::PlayerMusicModeState>()
        .set_music_mode_show_album_art(prefs.music_mode_show_album_art);
}

fn install_media(app: &PlayerWindow, prefs: &Preferences) {
    let player_preferences = app.global::<riina_ui::PlayerPreferences>();
    player_preferences
        .set_hardware_decoder_index(crate::codec_preferences::index(prefs.hardware_decoder));
    player_preferences.set_subtitle_ass_override_index(crate::subtitle_preferences::index(
        prefs.subtitle_ass_override,
    ));
    player_preferences.set_secondary_subtitle_ass_override_index(
        crate::subtitle_preferences::index(prefs.secondary_subtitle_ass_override),
    );
    app.global::<riina_ui::PlaybackSubtitleState>()
        .set_subtitle_font(prefs.subtitle_font.clone().into());
    player_preferences.set_subtitle_font_size_preference(prefs.subtitle_font_size as f32);
    crate::subtitle_preferences::install_style(app, prefs.subtitle_text_style);
    app.global::<riina_ui::PlaybackVideoState>()
        .set_video_aspect_presets(slint::ModelRc::new(slint::VecModel::from(
            riina_core::video_controls::ASPECT_PRESETS
                .map(slint::SharedString::from)
                .to_vec(),
        )));
    crate::screenshot_preferences::install(app, &prefs.screenshot);
    player_preferences.set_pause_when_open(prefs.pause_when_open);
    player_preferences.set_play_when_entering_fullscreen(prefs.play_when_entering_fullscreen);
    player_preferences.set_pause_when_leaving_fullscreen(prefs.pause_when_leaving_fullscreen);
    player_preferences.set_black_out_other_monitors(prefs.black_out_other_monitors);
    player_preferences.set_pause_when_inactive(prefs.pause_when_inactive);
    player_preferences.set_pause_when_minimized(prefs.pause_when_minimized);
    player_preferences.set_pause_when_goes_to_sleep(prefs.pause_when_goes_to_sleep);
    player_preferences.set_quit_when_no_opened_window(prefs.quit_when_no_opened_window);
    player_preferences.set_keep_open_on_file_end(prefs.keep_open_on_file_end);
    player_preferences.set_playlist_auto_play_next(prefs.playlist_auto_play_next);
    player_preferences.set_playlist_auto_add(prefs.playlist_auto_add);
    player_preferences.set_playlist_show_metadata(prefs.playlist_show_metadata);
    player_preferences
        .set_playlist_show_metadata_in_music_mode(prefs.playlist_show_metadata_in_music_mode);
    player_preferences.set_prefetch_playlist_video_duration(prefs.prefetch_playlist_video_duration);
    player_preferences.set_auto_repeat(prefs.auto_repeat);
    player_preferences.set_default_repeat_mode_index(prefs.default_repeat_mode.index());
    player_preferences.set_fullscreen_when_open(prefs.fullscreen_when_open);
    player_preferences.set_resume_last_position(prefs.resume_last_position);
    player_preferences.set_record_playback_history(prefs.record_playback_history);
    player_preferences.set_record_recent_files(prefs.record_recent_files);
    player_preferences.set_enable_new_window_shortcut(prefs.enable_new_window_shortcut);
    player_preferences.set_open_in_new_player(prefs.allow_multiple_windows);
    player_preferences
        .set_track_all_files_in_recent_open_menu(prefs.track_all_files_in_recent_open_menu);
    player_preferences.set_launch_action_index(prefs.action_after_launch.index());
}

fn install_layout(app: &PlayerWindow, prefs: &Preferences, prefs_path: &Path) -> Rc<slint::Timer> {
    app.global::<riina_ui::PlayerSidebarState>()
        .set_leading_sidebar_width(prefs.leading_sidebar_width as f32);
    app.global::<riina_ui::PlayerSidebarState>()
        .set_trailing_sidebar_width(prefs.trailing_sidebar_width as f32);
    app.global::<riina_ui::PlayerSidebarState>()
        .set_settings_at_leading(prefs.settings_at_leading);
    app.global::<riina_ui::PlayerSidebarState>()
        .set_playlist_at_leading(prefs.playlist_at_leading);
    app.global::<riina_ui::PlayerSidebarState>()
        .set_plugins_at_leading(prefs.plugins_at_leading);
    app.global::<riina_ui::PlayerLayoutState>()
        .set_edge_to_edge_video(prefs.edge_to_edge_video);
    app.global::<riina_ui::PlayerLayoutState>()
        .set_docked_controls(prefs.docked_controls);
    app.global::<riina_ui::PlayerLayoutState>()
        .set_unlock_window_aspect_ratio(prefs.unlock_window_aspect_ratio);
    app.global::<riina_ui::PlayerAppearance>()
        .set_compact_ui(prefs.compact_ui);
    app.global::<riina_ui::PlayerLayoutState>()
        .set_always_float_on_top(prefs.always_float_on_top);
    app.global::<riina_ui::PlayerLayoutState>()
        .set_always_show_on_top_icon(prefs.always_show_on_top_icon);
    let appearance = prefs.effective_theme_material();
    app.global::<riina_ui::PlayerAppearance>()
        .set_appearance_mode(appearance.index());
    app.global::<riina_ui::PlayerAppearance>()
        .set_surface_material_title(crate::platform::surface_material_title().into());
    app.global::<riina_ui::PlayerAppearance>()
        .set_dark_theme(appearance.dark_for(prefs.dark));
    let appearance_timer = crate::appearance_preferences::connect(app);
    app.global::<riina_ui::PlayerLayoutState>()
        .set_osc_position(i32::from(prefs.osc_position));
    app.global::<riina_ui::PlayerLayoutState>()
        .set_control_bar_position_horizontal(prefs.control_bar_position_horizontal as f32);
    app.global::<riina_ui::PlayerLayoutState>()
        .set_control_bar_position_vertical(prefs.control_bar_position_vertical as f32);
    app.global::<riina_ui::PlayerPreferences>()
        .set_show_chapter_pos(prefs.show_chapter_pos);
    crate::sidebar_preferences::connect(app, prefs_path.to_path_buf(), prefs.clone());
    crate::layout_preferences::connect(app, prefs_path.to_path_buf(), prefs.clone());
    refresh_video_menu(app);
    app.global::<riina_ui::PlaybackVideoState>()
        .set_video_crop_presets(slint::ModelRc::new(slint::VecModel::from(
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
