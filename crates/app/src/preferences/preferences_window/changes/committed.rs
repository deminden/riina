//! Project a persisted edit and notify the owners of live settings.
use slint::ComponentHandle;

use super::CredentialStore;
use crate::{
    PlayerWindow, PreferencesWindow, osd_preferences::Settings as Osd,
    preferences::model::PreferenceModel,
};
use riina_core::{
    online_subtitles::Preferences as SubtitleProviders,
    pip_preferences::Settings as PictureInPicture,
    thumbnail_preferences::Settings as ThumbnailPreview,
};

pub(super) struct PreviousValues<'a> {
    pub(super) pause_when_open: bool,
    pub(super) resume_last_position: bool,
    pub(super) record_playback_history: bool,
    pub(super) assrt_token: &'a str,
}

pub(super) fn project(
    app: &PlayerWindow,
    window: &PreferencesWindow,
    model: &PreferenceModel,
    credentials: &CredentialStore,
    playback: &crate::PlaybackStore,
    previous: PreviousValues<'_>,
    restart_notice: Option<&str>,
) {
    let player_preferences = app.global::<riina_ui::PlayerPreferences>();
    let saved = model.snapshot();
    crate::screenshot_preferences::install(app, &saved.screenshot);
    player_preferences.set_show_chapter_pos(saved.show_chapter_pos);
    player_preferences.set_play_when_entering_fullscreen(saved.play_when_entering_fullscreen);
    player_preferences.set_pause_when_leaving_fullscreen(saved.pause_when_leaving_fullscreen);
    player_preferences.set_black_out_other_monitors(saved.black_out_other_monitors);
    player_preferences.set_pause_when_inactive(saved.pause_when_inactive);
    player_preferences.set_pause_when_minimized(saved.pause_when_minimized);
    player_preferences.set_pause_when_goes_to_sleep(saved.pause_when_goes_to_sleep);
    player_preferences.set_quit_when_no_opened_window(saved.quit_when_no_opened_window);
    player_preferences.set_keep_open_on_file_end(saved.keep_open_on_file_end);
    player_preferences.set_record_recent_files(saved.record_recent_files);
    crate::player_ui::project_open_window_policy(app, saved.allow_multiple_windows);
    player_preferences
        .set_track_all_files_in_recent_open_menu(saved.track_all_files_in_recent_open_menu);
    player_preferences.set_playlist_auto_play_next(saved.playlist_auto_play_next);
    player_preferences.set_playlist_auto_add(saved.playlist_auto_add);
    player_preferences.set_playlist_show_metadata(saved.playlist_show_metadata);
    player_preferences
        .set_playlist_show_metadata_in_music_mode(saved.playlist_show_metadata_in_music_mode);
    player_preferences.set_auto_repeat(saved.auto_repeat);
    player_preferences.set_default_repeat_mode_index(saved.default_repeat_mode.index());
    model.write_player::<ThumbnailPreview>(app);
    project_subtitle_providers(
        app,
        window,
        &saved.online_subtitles,
        previous.assrt_token,
        credentials,
    );
    model.retain_subtitle_credentials(app);
    model.write_player::<PictureInPicture>(app);
    app.set_live_text_enabled(saved.live_text_enabled);
    player_preferences.set_load_icc_profile(saved.load_icc_profile);
    player_preferences.set_default_hdr_mode_enabled(saved.hdr_enabled);
    player_preferences.set_disable_animations(saved.disable_animations);
    app.global::<riina_ui::PlayerAppearance>()
        .set_appearance_mode(saved.effective_theme_material().index());
    app.global::<riina_ui::PlayerAppearance>()
        .set_dark_theme(saved.dark);
    model.write_player::<Osd>(app);
    let previous_controller = crate::osc_preferences::player_controller_options(app);
    model.write_player::<riina_core::osc_preferences::Settings>(app);
    if previous_controller != saved.control_bar.controller {
        app.global::<riina_ui::PlayerControlActions>()
            .invoke_publish_controller_options();
    }
    app.global::<riina_ui::PlayerLayoutState>()
        .set_always_float_on_top(saved.always_float_on_top);
    app.global::<riina_ui::PlayerLayoutState>()
        .set_always_show_on_top_icon(saved.always_show_on_top_icon);
    model.write_player::<riina_core::control_preferences::Settings>(app);
    model.write_player::<riina_core::liquid_glass::SurfaceSettings>(app);
    crate::key_binding_preferences::install_player(app, &saved.key_bindings, playback.clone());
    if let Some(message) = restart_notice {
        app.set_status(message.into());
    }
    if player_preferences.get_pause_when_open() != previous.pause_when_open {
        app.invoke_pause_when_open_changed(player_preferences.get_pause_when_open());
    }
    if player_preferences.get_resume_last_position() != previous.resume_last_position {
        app.invoke_resume_last_position_changed(player_preferences.get_resume_last_position());
    }
    if player_preferences.get_record_playback_history() != previous.record_playback_history {
        app.invoke_record_playback_history_changed(
            player_preferences.get_record_playback_history(),
        );
    }
    app.invoke_recent_files_visibility_changed();
}

fn project_subtitle_providers(
    app: &PlayerWindow,
    window: &PreferencesWindow,
    selected: &SubtitleProviders,
    previous_assrt_token: &str,
    credentials: &CredentialStore,
) {
    let player_preferences = app.global::<riina_ui::PlayerPreferences>();
    if selected.api_key == player_preferences.get_open_subtitles_api_key().as_str() {
        crate::online_subtitles_preferences::install_player(app, selected);
    } else {
        match (credentials.open_subtitles)(&selected.api_key) {
            Ok(()) => crate::online_subtitles_preferences::install_player(app, selected),
            Err(error) => {
                let mut retained = selected.clone();
                retained.api_key = player_preferences.get_open_subtitles_api_key().to_string();
                crate::online_subtitles_preferences::install_player(app, &retained);
                window
                    .global::<crate::SubtitleSettingsState>()
                    .set_open_subtitles_api_key(retained.api_key.into());
                app.set_status(error.into());
            }
        }
    }
    if selected.assrt_token != previous_assrt_token
        && let Err(error) = (credentials.assrt)(&selected.assrt_token)
    {
        let mut retained = selected.clone();
        retained.api_key = player_preferences.get_open_subtitles_api_key().to_string();
        retained.assrt_token = previous_assrt_token.into();
        crate::online_subtitles_preferences::install_player(app, &retained);
        window
            .global::<crate::SubtitleSettingsState>()
            .set_assrt_token(retained.assrt_token.into());
        app.set_status(error.into());
    }
}
