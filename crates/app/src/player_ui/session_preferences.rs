//! Persist session changes without replacing settings owned by other writers.
use super::*;

impl UiSession {
    pub(crate) fn save(&self, app: &PlayerWindow) -> Result<(), Box<dyn std::error::Error>> {
        let mut prefs = Preferences {
            volume: (app.get_volume() as f64).min(if app.get_volume_boost() {
                200.0
            } else {
                100.0
            }),
            volume_boost: app.get_volume_boost(),
            show_chapter_pos: app.get_show_chapter_pos(),
            picture_in_picture: crate::pip_preferences::current_player(app)
                .map_err(std::io::Error::other)?,
            action_after_launch: riina_core::ActionAfterLaunch::from_index(
                app.get_launch_action_index(),
            )
            .ok_or_else(|| std::io::Error::other("Invalid launch action selection"))?,
            hdr_enabled: app.get_default_hdr_mode_enabled(),
            live_text_enabled: app.get_live_text_enabled(),
            always_float_on_top: app.get_always_float_on_top(),
            always_show_on_top_icon: app.get_always_show_on_top_icon(),
            theme_material: Some(
                riina_core::ThemeMaterial::from_index(app.get_appearance_mode())
                    .ok_or_else(|| std::io::Error::other("Invalid appearance selection"))?,
            ),
            pause_when_open: app.get_pause_when_open(),
            play_when_entering_fullscreen: app.get_play_when_entering_fullscreen(),
            pause_when_leaving_fullscreen: app.get_pause_when_leaving_fullscreen(),
            pause_when_inactive: app.get_pause_when_inactive(),
            pause_when_minimized: app.get_pause_when_minimized(),
            pause_when_goes_to_sleep: app.get_pause_when_goes_to_sleep(),
            quit_when_no_opened_window: app.get_quit_when_no_opened_window(),
            keep_open_on_file_end: app.get_keep_open_on_file_end(),
            playlist_auto_play_next: app.get_playlist_auto_play_next(),
            playlist_auto_add: app.get_playlist_auto_add(),
            playlist_show_metadata: app.get_playlist_show_metadata(),
            playlist_show_metadata_in_music_mode: app.get_playlist_show_metadata_in_music_mode(),
            auto_repeat: app.get_auto_repeat(),
            default_repeat_mode: riina_core::DefaultRepeatMode::from_index(
                app.get_default_repeat_mode_index(),
            )
            .ok_or_else(|| std::io::Error::other("Invalid default repeat mode"))?,
            fullscreen_when_open: app.get_fullscreen_when_open(),
            resume_last_position: app.get_resume_last_position(),
            enable_osd: app.get_osd_enabled(),
            disable_osd_file_start: app.get_disable_osd_file_start(),
            disable_osd_pause_resume: app.get_disable_osd_pause_resume(),
            disable_osd_speed: app.get_disable_osd_speed(),
            disable_osd_seek: app.get_disable_osd_seek(),
            osd_timeout_secs: f64::from(app.get_osd_timeout_secs()),
            osd_text_size: f64::from(app.get_osd_text_size()),
            control_bar: crate::osc_preferences::current_player(app),
            display_time_and_battery_in_fullscreen: app
                .get_display_time_and_battery_in_fullscreen(),
            liquid_glass: crate::liquid_glass_preferences::current_player(app),
            leading_sidebar_width: app.get_leading_sidebar_width() as u32,
            trailing_sidebar_width: app.get_trailing_sidebar_width() as u32,
            settings_at_leading: app.get_settings_at_leading(),
            playlist_at_leading: app.get_playlist_at_leading(),
            plugins_at_leading: app.get_plugins_at_leading(),
            edge_to_edge_video: app.get_edge_to_edge_video(),
            docked_controls: app.get_docked_controls(),
            unlock_window_aspect_ratio: app.get_unlock_window_aspect_ratio(),
            window_resize: crate::window_resize_preferences::from_player(app)
                .map_err(std::io::Error::other)?,
            compact_ui: app.get_compact_ui(),
            auto_switch_to_music_mode: app.get_auto_switch_to_music_mode(),
            disable_animations: app.get_disable_animations(),
            music_mode_show_playlist: app.get_music_mode_show_playlist(),
            music_mode_show_album_art: app.get_music_mode_show_album_art(),
            osc_position: app.get_osc_position() as u8,
            control_bar_position_horizontal: f64::from(app.get_control_bar_position_horizontal()),
            control_bar_position_vertical: f64::from(app.get_control_bar_position_vertical()),
            hardware_decoder: crate::codec_preferences::hardware_decoder(
                app.get_hardware_decoder_index(),
            )
            .ok_or_else(|| std::io::Error::other("Invalid hardware decoder selection"))?,
            subtitle_ass_override: crate::subtitle_preferences::level(
                app.get_subtitle_ass_override_index(),
            )
            .ok_or_else(|| std::io::Error::other("Invalid ASS override selection"))?,
            secondary_subtitle_ass_override: crate::subtitle_preferences::level(
                app.get_secondary_subtitle_ass_override_index(),
            )
            .ok_or_else(|| std::io::Error::other("Invalid secondary ASS override selection"))?,
            subtitle_position: crate::subtitle_position_preferences::current_player(app)
                .map_err(std::io::Error::other)?,
            subtitle_other: crate::subtitle_other_preferences::current_player(app)
                .map_err(std::io::Error::other)?,
            subtitle_auto_load: crate::subtitle_auto_load_preferences::current_player(app)
                .map_err(std::io::Error::other)?,
            online_subtitles: crate::online_subtitles_preferences::current_player(app)
                .map_err(std::io::Error::other)?,
            subtitle_font: app.get_subtitle_font().to_string(),
            subtitle_font_size: crate::subtitle_preferences::current_font_size(app),
            subtitle_text_style: crate::subtitle_preferences::current_style(app)
                .map_err(std::io::Error::other)?,
            screenshot: crate::screenshot_preferences::current(app)
                .map_err(std::io::Error::other)?,
            thumbnail_preview: crate::thumbnail_preferences::current_player(app)
                .map_err(std::io::Error::other)?,
            dark: app.get_dark_theme(),
            network_cache: crate::network_cache_preferences::current_player(app)
                .map_err(std::io::Error::other)?,
            network_online_videos: crate::network_ytdl_preferences::current_player(app)
                .map_err(std::io::Error::other)?,
            ..self.prefs.clone()
        };
        prefs.audio_equalizer = self
            .audio_equalizer
            .lock()
            .map_err(|_| std::io::Error::other("audio equalizer settings lock poisoned"))?
            .clone();
        prefs.video_filters = self
            .saved_video_filters
            .lock()
            .map_err(|_| std::io::Error::other("saved video filter settings lock poisoned"))?
            .clone();
        prefs.audio_filters = self
            .saved_audio_filters
            .lock()
            .map_err(|_| std::io::Error::other("saved audio filter settings lock poisoned"))?
            .clone();
        prefs.recent_media = self.recent_media.borrow().clone();
        prefs.recent_open_documents = self.recent_open_documents.borrow().clone();
        prefs.record_recent_files = app.get_record_recent_files();
        prefs.track_all_files_in_recent_open_menu = app.get_track_all_files_in_recent_open_menu();
        prefs.playback_history = self
            .playback_history
            .lock()
            .map_err(|_| std::io::Error::other("playback history lock poisoned"))?
            .entries()
            .to_vec();
        prefs.record_playback_history = self.record_playback_history.load(Ordering::Acquire);
        // The Settings window persists audio engine preferences immediately;
        // preserve those newer values when the player writes its final state.
        if let Ok(text) = std::fs::read_to_string(&self.prefs_path)
            && let Ok(saved) = Preferences::parse(&text)
        {
            prefs.audio = saved.audio;
            prefs.advanced = saved.advanced;
            prefs.allow_multiple_windows = saved.allow_multiple_windows;
            prefs.group_simultaneous_opens_in_playlist = saved.group_simultaneous_opens_in_playlist;
            prefs.allow_duplicate_players = saved.allow_duplicate_players;
            prefs.quit_when_no_opened_window = saved.quit_when_no_opened_window;
            prefs.keep_open_on_file_end = saved.keep_open_on_file_end;
            prefs.playlist_auto_play_next = saved.playlist_auto_play_next;
            prefs.playlist_auto_add = saved.playlist_auto_add;
            prefs.playlist_show_metadata = saved.playlist_show_metadata;
            prefs.playlist_show_metadata_in_music_mode = saved.playlist_show_metadata_in_music_mode;
            prefs.auto_repeat = saved.auto_repeat;
            prefs.default_repeat_mode = saved.default_repeat_mode;
            prefs.play_when_entering_fullscreen = saved.play_when_entering_fullscreen;
            prefs.pause_when_leaving_fullscreen = saved.pause_when_leaving_fullscreen;
            prefs.pause_when_inactive = saved.pause_when_inactive;
            prefs.pause_when_minimized = saved.pause_when_minimized;
            prefs.pause_when_goes_to_sleep = saved.pause_when_goes_to_sleep;
            for recent in saved.recent_media {
                if !prefs
                    .recent_media
                    .iter()
                    .any(|current| current.path == recent.path)
                {
                    prefs.recent_media.push(recent);
                }
            }
            prefs.recent_media.truncate(10);
            if !self.recent_open_cleared.get() {
                for recent in saved.recent_open_documents {
                    if !prefs
                        .recent_open_documents
                        .iter()
                        .any(|current| current.path == recent.path)
                    {
                        prefs.recent_open_documents.push(recent);
                    }
                }
                prefs.recent_open_documents.truncate(10);
            }
            if !self.history_cleared.load(Ordering::Acquire) {
                for entry in saved.playback_history {
                    if !prefs
                        .playback_history
                        .iter()
                        .any(|current| current.path == entry.path)
                    {
                        prefs.playback_history.push(entry);
                    }
                }
            }
        }
        if let Ok(state) = self.latest_playback.lock()
            && let Some(position) = state
                .position
                .filter(|value| value.is_finite() && *value >= 0.0)
            && !matches!(state.phase, Phase::Loading | Phase::Failed)
            && let Some(entry) = state.playlist.iter().find(|entry| entry.current)
        {
            let title = entry
                .title
                .as_ref()
                .filter(|title| !title.trim().is_empty())
                .cloned()
                .unwrap_or_else(|| {
                    if state.title.trim().is_empty() {
                        PathBuf::from(&entry.filename).file_name().map_or_else(
                            || entry.filename.clone(),
                            |name| name.to_string_lossy().into_owned(),
                        )
                    } else {
                        state.title.clone()
                    }
                });
            let position = if app.get_resume_last_position() {
                position
            } else {
                prefs
                    .recent_media
                    .iter()
                    .find(|item| item.path == entry.filename)
                    .map_or(0.0, |item| item.position)
            };
            prefs.remember_media(riina_core::RecentMedia {
                path: entry.filename.clone(),
                title,
                position,
            });
        }
        self.preference_writer
            .save(&prefs)
            .map_err(std::io::Error::other)?;
        Ok(())
    }
}
