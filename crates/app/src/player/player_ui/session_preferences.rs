//! Save changes that belong to an individual player session.

use super::*;

pub(super) fn connect(
    app: &PlayerWindow,
    context: &PlayerContext,
    history: &history_session::Tracking,
) {
    crate::preferences_window::connect(
        app,
        context.commands().clone(),
        context.preferences_path().to_path_buf(),
        context.preferences().clone(),
        context.playback().clone(),
        context.plugin_host().cloned(),
        history.clear_hook(
            context.preferences_path().to_path_buf(),
            context.plugin_host().cloned(),
            context.playback().clone(),
        ),
    );
}

impl UiSession {
    pub(crate) fn save(&self, app: &PlayerWindow) -> Result<(), Box<dyn std::error::Error>> {
        let mut prefs = capture_session_state(app, self.preference_model.snapshot())?;
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
        prefs.playback_history = self
            .history
            .state
            .lock()
            .map_err(|_| std::io::Error::other("playback history lock poisoned"))?
            .entries()
            .to_vec();
        prefs.record_playback_history = self.history.recording.load(Ordering::Acquire);

        self.merge_saved_recents(&mut prefs);
        self.remember_current_media(&mut prefs);
        self.preference_model
            .save_session(prefs)
            .map_err(std::io::Error::other)?;
        Ok(())
    }

    fn merge_saved_recents(&self, prefs: &mut Preferences) {
        let Ok(Some(saved)) = crate::preference_load::read_valid(&self.prefs_path) else {
            return;
        };
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
        if !self.history.cleared.load(Ordering::Acquire) {
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

    fn remember_current_media(&self, prefs: &mut Preferences) {
        let state = riina_core::sync::lock(&self.latest_playback);
        if let Some(position) = state
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
            let position = if prefs.resume_last_position {
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
    }
}

fn capture_session_state(
    app: &PlayerWindow,
    preferences: Preferences,
) -> std::io::Result<Preferences> {
    // These values can change through the player, its native menus, or engine
    // observations. Settings-only controls are already in the model.
    Ok(Preferences {
        volume: f64::from(app.get_volume()).min(if app.get_volume_boost() { 200.0 } else { 100.0 }),
        volume_boost: app.get_volume_boost(),
        resume_last_position: app.get_resume_last_position(),
        live_text_enabled: app.get_live_text_enabled(),
        always_float_on_top: app.get_always_float_on_top(),
        theme_material: Some(
            riina_core::ThemeMaterial::from_index(app.get_appearance_mode())
                .ok_or_else(|| std::io::Error::other("Invalid appearance selection"))?,
        ),
        dark: app.get_dark_theme(),
        leading_sidebar_width: app.get_leading_sidebar_width() as u32,
        trailing_sidebar_width: app.get_trailing_sidebar_width() as u32,
        settings_at_leading: app.get_settings_at_leading(),
        playlist_at_leading: app.get_playlist_at_leading(),
        plugins_at_leading: app.get_plugins_at_leading(),
        edge_to_edge_video: app.get_edge_to_edge_video(),
        docked_controls: app.get_docked_controls(),
        unlock_window_aspect_ratio: app.get_unlock_window_aspect_ratio(),
        compact_ui: app.get_compact_ui(),
        osc_position: app.get_osc_position() as u8,
        control_bar_position_horizontal: f64::from(app.get_control_bar_position_horizontal()),
        control_bar_position_vertical: f64::from(app.get_control_bar_position_vertical()),
        music_mode_show_playlist: app.get_music_mode_show_playlist(),
        music_mode_show_album_art: app.get_music_mode_show_album_art(),
        record_recent_files: app.get_record_recent_files(),
        track_all_files_in_recent_open_menu: app.get_track_all_files_in_recent_open_menu(),
        subtitle_other: crate::subtitle_other_preferences::current_player(app)
            .map_err(std::io::Error::other)?,
        subtitle_font: app.get_subtitle_font().to_string(),
        subtitle_font_size: crate::subtitle_preferences::current_font_size(app),
        subtitle_text_style: crate::subtitle_preferences::current_style(app)
            .map_err(std::io::Error::other)?,
        ..preferences
    })
}
