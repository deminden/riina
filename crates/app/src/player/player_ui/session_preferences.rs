//! Save changes that belong to an individual player session.

use super::{
    ApplicationServices, PlayerContext, UiSession, history_session,
    recent_activity::RecentActivity, startup,
};
use crate::{PlaybackStore, PlayerWindow};
use riina_core::{Phase, Preferences, audio_equalizer, video_filters};
use riina_ui::{
    PlaybackAudioState, PlaybackSubtitleState, PlayerAppearance, PlayerLayoutState,
    PlayerMusicModeState, PlayerPreferences, PlayerSidebarState,
};
use slint::ComponentHandle;
use std::{
    io,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Mutex, atomic::Ordering},
};

pub(super) fn connect(
    app: &PlayerWindow,
    context: &PlayerContext,
    history: &history_session::Tracking,
    recent: &Rc<RecentActivity>,
    services: &ApplicationServices,
) {
    crate::preferences_window::connect(
        app,
        crate::preferences_window::Session {
            decoder_player: context.commands().clone(),
            path: context.preferences_path().to_path_buf(),
            model: context.preferences().clone(),
            latest_playback: context.playback().clone(),
            plugin_host: context.plugin_host().cloned(),
            updates: services.updates.clone(),
            power: services.power.clone(),
            #[cfg(target_os = "macos")]
            application_settings: services.settings.clone(),
        },
        history.clear_hook(
            context.preferences_path().to_path_buf(),
            recent.clone(),
            services.history.clone(),
        ),
    );
}

impl UiSession {
    pub(crate) fn save(&self, app: &PlayerWindow) -> Result<(), Box<dyn std::error::Error>> {
        let candidate = self
            .preference_state
            .capture(app, self.preference_model.snapshot())?;
        self.preference_model
            .save_session(candidate)
            .map_err(io::Error::other)?;
        self.preference_state
            .save_current_media()
            .map_err(io::Error::other)?;
        Ok(())
    }
}

pub(super) struct SessionState {
    pub(super) audio_equalizer: Arc<Mutex<audio_equalizer::Settings>>,
    pub(super) saved_video_filters: Arc<Mutex<video_filters::Settings>>,
    pub(super) saved_audio_filters: Arc<Mutex<video_filters::Settings>>,
    pub(super) recent: startup::SessionRecents,
    pub(super) history: history_session::Tracking,
    pub(super) latest_playback: PlaybackStore,
}

impl SessionState {
    fn capture(&self, app: &PlayerWindow, preferences: Preferences) -> io::Result<Preferences> {
        let mut prefs = capture_window_state(app, preferences)?;
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
        prefs.playback_history = self
            .history
            .state
            .lock()
            .map_err(|_| std::io::Error::other("playback history lock poisoned"))?
            .entries()
            .to_vec();
        prefs.record_playback_history = self.history.recording.load(Ordering::Acquire);

        Ok(prefs)
    }

    fn save_current_media(&self) -> Result<(), String> {
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
            let media = riina_core::RecentMedia {
                path: entry.filename.clone(),
                title,
                position,
            };
            drop(state);
            self.recent
                .activity
                .save_resume(media, self.recent.resume_revision.get())?;
        }
        Ok(())
    }
}

fn capture_window_state(app: &PlayerWindow, preferences: Preferences) -> io::Result<Preferences> {
    let player_preferences = app.global::<PlayerPreferences>();
    let layout = app.global::<PlayerLayoutState>();
    let appearance = app.global::<PlayerAppearance>();
    let sidebar = app.global::<PlayerSidebarState>();
    let music_mode = app.global::<PlayerMusicModeState>();
    let volume_boost = player_preferences.get_volume_boost();
    // These values can change through the player, its native menus, or engine
    // observations. Settings-only controls are already in the model.
    Ok(Preferences {
        volume: f64::from(app.global::<PlaybackAudioState>().get_volume()).min(if volume_boost {
            200.0
        } else {
            100.0
        }),
        volume_boost,
        resume_last_position: player_preferences.get_resume_last_position(),
        live_text_enabled: app.get_live_text_enabled(),
        always_float_on_top: layout.get_always_float_on_top(),
        theme_material: Some(
            riina_core::ThemeMaterial::from_index(appearance.get_appearance_mode())
                .ok_or_else(|| io::Error::other("Invalid appearance selection"))?,
        ),
        dark: appearance.get_dark_theme(),
        leading_sidebar_width: sidebar.get_leading_sidebar_width() as u32,
        trailing_sidebar_width: sidebar.get_trailing_sidebar_width() as u32,
        settings_at_leading: sidebar.get_settings_at_leading(),
        playlist_at_leading: sidebar.get_playlist_at_leading(),
        plugins_at_leading: sidebar.get_plugins_at_leading(),
        edge_to_edge_video: layout.get_edge_to_edge_video(),
        docked_controls: layout.get_docked_controls(),
        unlock_window_aspect_ratio: layout.get_unlock_window_aspect_ratio(),
        compact_ui: appearance.get_compact_ui(),
        osc_position: layout.get_osc_position() as u8,
        control_bar_position_horizontal: f64::from(layout.get_control_bar_position_horizontal()),
        control_bar_position_vertical: f64::from(layout.get_control_bar_position_vertical()),
        music_mode_show_playlist: music_mode.get_music_mode_show_playlist(),
        music_mode_show_album_art: music_mode.get_music_mode_show_album_art(),
        record_recent_files: player_preferences.get_record_recent_files(),
        track_all_files_in_recent_open_menu: player_preferences
            .get_track_all_files_in_recent_open_menu(),
        subtitle_other: crate::subtitle_other_preferences::current_player(app)
            .map_err(io::Error::other)?,
        subtitle_font: app
            .global::<PlaybackSubtitleState>()
            .get_subtitle_font()
            .to_string(),
        subtitle_font_size: crate::subtitle_preferences::current_font_size(app),
        subtitle_text_style: crate::subtitle_preferences::current_style(app)
            .map_err(io::Error::other)?,
        ..preferences
    })
}
