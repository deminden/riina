//! Validate and commit a preferences edit, restoring live state on failure.
mod committed;
mod display;
mod playback_settings;
mod subtitles;

use super::general_policy as general;
use super::{
    live::{AppliedDecoderSettings, DecoderState},
    refresh::refresh,
};
use crate::{PlayerWindow, PreferencesWindow, preferences::model::PreferenceModel};
#[cfg(test)]
use riina_core::Command;
use slint::ComponentHandle;

pub(super) fn connect(
    app: &PlayerWindow,
    window: &PreferencesWindow,
    decoder_player: riina_mpv::CommandSender,
    model: PreferenceModel,
    key_binding_playback: crate::PlaybackStore,
) {
    connect_with_credentials(
        app,
        window,
        decoder_player,
        model,
        key_binding_playback,
        CredentialStore::system(),
    );
}

struct CredentialStore {
    open_subtitles: fn(&str) -> Result<(), String>,
    assrt: fn(&str) -> Result<(), String>,
}

impl CredentialStore {
    fn system() -> Self {
        Self {
            open_subtitles: crate::online_subtitles_credentials::store,
            assrt: crate::online_subtitles_credentials::store_assrt,
        }
    }
}

fn connect_with_credentials(
    app: &PlayerWindow,
    window: &PreferencesWindow,
    decoder_player: riina_mpv::CommandSender,
    model: PreferenceModel,
    key_binding_playback: crate::PlaybackStore,
    credentials: CredentialStore,
) {
    let parent = app.as_weak();
    let weak = window.as_weak();
    let baseline = model.snapshot();
    window.on_settings_changed(move || {
        let (Some(app), Some(window)) = (parent.upgrade(), weak.upgrade()) else {
            return;
        };
        let saved = model.snapshot();
        let previous = match crate::screenshot_preferences::current(&app) {
            Ok(settings) => settings,
            Err(error) => {
                app.set_status(error.into());
                return;
            }
        };
        let Some(general) = general::Draft::read(&app, &window) else {
            return;
        };
        let Some(display) = display::Draft::read(&app, &window, &model) else {
            return;
        };
        let previous_volume = app.global::<riina_ui::PlaybackAudioState>().get_volume();
        let previous_decoder = app
            .global::<riina_ui::PlayerPreferences>()
            .get_hardware_decoder_index();
        let Some(subtitles) = subtitles::Draft::read(&app, &window, &model) else {
            return;
        };
        let Some(selected_online_subtitles) = read_or_refresh(
            model.read_section::<riina_core::online_subtitles::Preferences>(&window),
            &app,
            &window,
        ) else {
            return;
        };
        let previous_assrt_token = app
            .global::<riina_ui::PlayerPreferences>()
            .get_assrt_token()
            .to_string();
        let Some(playback) =
            playback_settings::Draft::read(&app, &window, &model, &saved, &baseline)
        else {
            return;
        };
        let Some((decoder, applied_decoder)) = read_or_refresh(
            apply_decoder_edit(
                &app,
                &window,
                &decoder_player,
                DecoderState {
                    boost: display.previous_volume_boost,
                    volume: previous_volume,
                    decoder_index: previous_decoder,
                },
            ),
            &app,
            &window,
        ) else {
            return;
        };
        let Some(applied_subtitles) = read_or_refresh(
            subtitles
                .apply(&app, &window, &decoder_player)
                .inspect_err(|_| applied_decoder.restore(&app, &decoder_player)),
            &app,
            &window,
        ) else {
            return;
        };
        general.selected.project_open_policy(&app);
        display.project_before_save(&app, &window);
        let restart_notice = playback.restart_notice();
        let mut playback_changes = playback_settings::Applied::default();
        let result = subtitles
            .apply_tracks(&decoder_player)
            .and_then(|()| playback.apply(&app, &decoder_player, &general, &mut playback_changes))
            .and_then(|()| {
                subtitles.project_tracks(&app);
                crate::screenshot_preferences::read_window(&window, &previous)
            })
            .and_then(|screenshot| {
                let mut preferences = capture_preferences(
                    &app,
                    &window,
                    &baseline,
                    screenshot,
                    decoder,
                    &selected_online_subtitles,
                    &applied_subtitles,
                );
                general.put(&mut preferences);
                display.put(&mut preferences);
                playback.put(&mut preferences);
                subtitles.put_tracks(&mut preferences);
                save_preferences(&window, &app, &model, preferences)
            });
        match result {
            Ok(()) => committed::project(
                &app,
                &window,
                &model,
                &credentials,
                &key_binding_playback,
                committed::PreviousValues {
                    pause_when_open: general.previous.pause_when_open,
                    resume_last_position: general.previous.resume_last_position,
                    record_playback_history: general.previous.record_playback_history,
                    assrt_token: &previous_assrt_token,
                },
                restart_notice,
            ),
            Err(error) => {
                general::install_open_policy(&window, &saved);
                playback.restore(&app, &window, &decoder_player, &general, &playback_changes);
                subtitles.restore_tracks(&app, &decoder_player);
                applied_subtitles.restore(&app, &decoder_player, &subtitles);
                applied_decoder.restore(&app, &decoder_player);
                general.previous.restore(&app);
                display.restore(&app);
                crate::tone_mapping_preferences::install_window(
                    &window,
                    &playback.previous_tone_mapping,
                );
                refresh(&window, &app);
                app.set_status(error.into());
            }
        }
    });
}

fn apply_decoder_edit(
    app: &PlayerWindow,
    window: &PreferencesWindow,
    sender: &riina_mpv::CommandSender,
    previous: DecoderState,
) -> Result<
    (
        riina_core::video_controls::HardwareDecoder,
        AppliedDecoderSettings,
    ),
    String,
> {
    let index = window.get_hardware_decoder_index();
    let decoder = crate::codec_preferences::hardware_decoder(index)
        .ok_or_else(|| "Invalid hardware decoder selection".to_owned())?;
    let applied = AppliedDecoderSettings::apply(
        app,
        sender,
        previous,
        window.get_volume_boost(),
        index,
        decoder,
    )?;
    Ok((decoder, applied))
}

fn capture_preferences(
    app: &PlayerWindow,
    window: &PreferencesWindow,
    baseline: &riina_core::Preferences,
    screenshot: riina_core::screenshot::Settings,
    decoder: riina_core::video_controls::HardwareDecoder,
    providers: &riina_core::online_subtitles::Preferences,
    applied_subtitles: &subtitles::Applied,
) -> riina_core::Preferences {
    riina_core::Preferences {
        screenshot,
        volume: f64::from(app.global::<riina_ui::PlaybackAudioState>().get_volume()).min(
            if app
                .global::<riina_ui::PlayerPreferences>()
                .get_volume_boost()
            {
                200.0
            } else {
                100.0
            },
        ),
        volume_boost: app
            .global::<riina_ui::PlayerPreferences>()
            .get_volume_boost(),
        show_chapter_pos: window.get_show_chapter_pos(),
        allow_multiple_windows: window
            .global::<riina_ui::GeneralSettingsState>()
            .get_allow_multiple_windows(),
        group_simultaneous_opens_in_playlist: window
            .global::<riina_ui::GeneralSettingsState>()
            .get_group_simultaneous_opens_in_playlist(),
        allow_duplicate_players: window
            .global::<riina_ui::GeneralSettingsState>()
            .get_allow_duplicate_players(),
        live_text_enabled: window.get_live_text_enabled(),
        load_icc_profile: window.get_load_icc_profile(),
        hdr_enabled: window.get_default_hdr_mode_enabled(),
        disable_animations: window.get_disable_animations(),
        always_float_on_top: window.get_always_float_on_top(),
        always_show_on_top_icon: window.get_always_show_on_top_icon(),
        record_recent_files: window
            .global::<riina_ui::GeneralSettingsState>()
            .get_record_recent_files(),
        track_all_files_in_recent_open_menu: window
            .global::<riina_ui::GeneralSettingsState>()
            .get_track_all_files_in_recent_open_menu(),
        leading_sidebar_width: app
            .global::<riina_ui::PlayerSidebarState>()
            .get_leading_sidebar_width() as u32,
        trailing_sidebar_width: app
            .global::<riina_ui::PlayerSidebarState>()
            .get_trailing_sidebar_width() as u32,
        settings_at_leading: app
            .global::<riina_ui::PlayerSidebarState>()
            .get_settings_at_leading(),
        playlist_at_leading: app
            .global::<riina_ui::PlayerSidebarState>()
            .get_playlist_at_leading(),
        plugins_at_leading: app
            .global::<riina_ui::PlayerSidebarState>()
            .get_plugins_at_leading(),
        edge_to_edge_video: app
            .global::<riina_ui::PlayerLayoutState>()
            .get_edge_to_edge_video(),
        docked_controls: app
            .global::<riina_ui::PlayerLayoutState>()
            .get_docked_controls(),
        unlock_window_aspect_ratio: app
            .global::<riina_ui::PlayerLayoutState>()
            .get_unlock_window_aspect_ratio(),
        compact_ui: app.global::<riina_ui::PlayerAppearance>().get_compact_ui(),
        osc_position: app
            .global::<riina_ui::PlayerLayoutState>()
            .get_osc_position() as u8,
        control_bar_position_horizontal: f64::from(
            app.global::<riina_ui::PlayerLayoutState>()
                .get_control_bar_position_horizontal(),
        ),
        control_bar_position_vertical: f64::from(
            app.global::<riina_ui::PlayerLayoutState>()
                .get_control_bar_position_vertical(),
        ),
        hardware_decoder: decoder,
        subtitle_ass_override: applied_subtitles.primary,
        secondary_subtitle_ass_override: applied_subtitles.secondary,
        online_subtitles: providers.clone(),
        subtitle_font: applied_subtitles.font.clone(),
        subtitle_font_size: applied_subtitles.font_size,
        subtitle_text_style: applied_subtitles.style,
        ..baseline.clone()
    }
}

fn save_preferences(
    window: &PreferencesWindow,
    app: &PlayerWindow,
    model: &PreferenceModel,
    mut preferences: riina_core::Preferences,
) -> Result<(), String> {
    preferences.auto_switch_to_music_mode = window
        .global::<riina_ui::GeneralSettingsState>()
        .get_auto_switch_to_music_mode();
    preferences.music_mode_show_playlist = window
        .global::<riina_ui::GeneralSettingsState>()
        .get_music_mode_show_playlist();
    preferences.music_mode_show_album_art = window
        .global::<riina_ui::GeneralSettingsState>()
        .get_music_mode_show_album_art();
    // These setters trigger automatic-mode and window-geometry callbacks. Apply
    // them only after saving, so a rejected edit cannot alter playback policy.
    let auto_switch = preferences.auto_switch_to_music_mode;
    let show_playlist = preferences.music_mode_show_playlist;
    let show_album_art = preferences.music_mode_show_album_art;
    model.save(preferences)?;
    app.global::<riina_ui::PlayerMusicModeState>()
        .set_auto_switch_to_music_mode(auto_switch);
    app.global::<riina_ui::PlayerMusicModeState>()
        .set_music_mode_show_playlist(show_playlist);
    app.global::<riina_ui::PlayerMusicModeState>()
        .set_music_mode_show_album_art(show_album_art);
    Ok(())
}

fn read_or_refresh<T>(
    value: Result<T, String>,
    app: &PlayerWindow,
    window: &PreferencesWindow,
) -> Option<T> {
    match value {
        Ok(value) => Some(value),
        Err(error) => {
            app.set_status(error.into());
            refresh(window, app);
            None
        }
    }
}

#[cfg(test)]
mod tests;
