//! Validate and commit a preferences edit, restoring live state on failure.
mod committed;
mod general;
mod playback_settings;
mod subtitles;

use super::{
    live::{AppliedDecoderSettings, DecoderState},
    refresh::refresh,
};
use crate::{
    PlayerWindow, PreferencesWindow,
    osd_preferences::Settings as Osd,
    preferences::model::{PreferenceModel, PreferenceSection},
};
use riina_core::{
    Command, pip_preferences::Settings as PictureInPicture,
    thumbnail_preferences::Settings as ThumbnailPreview,
};
use slint::ComponentHandle;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

pub(super) fn connect(
    app: &PlayerWindow,
    window: &PreferencesWindow,
    decoder_player: riina_mpv::CommandSender,
    path: PathBuf,
    model: PreferenceModel,
    key_binding_playback: Arc<Mutex<riina_core::Playback>>,
) {
    connect_with_credentials(
        app,
        window,
        decoder_player,
        path,
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

#[expect(
    clippy::too_many_lines,
    reason = "decomposition tracked in docs/review"
)]
fn connect_with_credentials(
    app: &PlayerWindow,
    window: &PreferencesWindow,
    decoder_player: riina_mpv::CommandSender,
    path: PathBuf,
    model: PreferenceModel,
    key_binding_playback: Arc<Mutex<riina_core::Playback>>,
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
        let previous_osd = crate::osd_preferences::current_player(&app);
        let Some(selected_osd) = read_or_refresh(model.read_section::<Osd>(&window), &app, &window)
        else {
            return;
        };
        let Some(selected_control_bar) = read_or_refresh(
            model.read_section::<riina_core::osc_preferences::Settings>(&window),
            &app,
            &window,
        ) else {
            return;
        };
        let Some(selected_picture_in_picture) = read_or_refresh(
            model.read_section::<PictureInPicture>(&window),
            &app,
            &window,
        ) else {
            return;
        };
        let Some(selected_thumbnail_preview) = read_or_refresh(
            model.read_section::<ThumbnailPreview>(&window),
            &app,
            &window,
        ) else {
            return;
        };
        let previous_layout = (
            app.get_osc_position(),
            app.get_compact_ui(),
            app.get_docked_controls(),
        );
        let previous_chapter_positions = app.get_show_chapter_pos();
        let Some(previous_window_resize) = read_or_refresh(
            crate::window_resize_preferences::from_player(&app),
            &app,
            &window,
        ) else {
            return;
        };
        let previous_volume_boost = app.get_volume_boost();
        let previous_liquid_glass = crate::liquid_glass_preferences::current_player(&app);
        let Some(selected_liquid_glass) = read_or_refresh(
            model.read_section::<riina_core::liquid_glass::SurfaceSettings>(&window),
            &app,
            &window,
        ) else {
            return;
        };
        let Some(selected_theme) =
            riina_core::ThemeMaterial::from_index(window.get_appearance_index())
        else {
            app.set_status("Choose a valid appearance".into());
            refresh(&window, &app);
            return;
        };
        let Some(selected_window_resize) = read_or_refresh(
            model.read_section::<riina_core::window_resize::Settings>(&window),
            &app,
            &window,
        ) else {
            return;
        };
        let selected_dark = selected_theme.dark_for(app.get_dark_theme());
        let previous_volume = app.get_volume();
        let previous_decoder = app.get_hardware_decoder_index();
        let Some(subtitles) = subtitles::Draft::read(&app, &window, &model) else {
            return;
        };
        let Some(selected_online_subtitles) = read_or_refresh(
            crate::online_subtitles_preferences::current_window(&window),
            &app,
            &window,
        ) else {
            return;
        };
        let previous_assrt_token = app.get_assrt_token().to_string();
        let Some(playback) =
            playback_settings::Draft::read(&app, &window, &model, &saved, &baseline)
        else {
            return;
        };
        let decoder_index = window.get_hardware_decoder_index();
        let Some(decoder) = crate::codec_preferences::hardware_decoder(decoder_index) else {
            app.set_status("Invalid hardware decoder selection".into());
            refresh(&window, &app);
            return;
        };
        let applied_decoder = match AppliedDecoderSettings::apply(
            &app,
            &decoder_player,
            DecoderState {
                boost: previous_volume_boost,
                volume: previous_volume,
                decoder_index: previous_decoder,
            },
            window.get_volume_boost(),
            decoder_index,
            decoder,
        ) {
            Ok(applied) => applied,
            Err(error) => {
                app.set_status(error.into());
                refresh(&window, &app);
                return;
            }
        };
        let applied_subtitles = match subtitles.apply(&app, &window, &decoder_player) {
            Ok(applied) => applied,
            Err(error) => {
                applied_decoder.restore(&app, &decoder_player);
                app.set_status(error.into());
                refresh(&window, &app);
                return;
            }
        };
        let formats = ["png", "jpg", "jpeg", "webp", "jxl"];
        // AVIF can be configured externally but has no item in the pinned
        // General pane. Editing another control must preserve that choice.
        if let Ok(index) = usize::try_from(window.get_format_index())
            && let Some(format) = formats.get(index)
        {
            app.set_screenshot_format((*format).into());
        }
        app.set_screenshot_include_subtitles(window.get_include_subtitles());
        app.set_screenshot_show_preview(window.get_show_preview());
        app.set_screenshot_save_to_file(window.get_save_to_file());
        app.set_screenshot_copy_to_clipboard(window.get_copy_to_clipboard());
        app.set_screenshot_custom_directory(window.get_custom_directory());
        app.set_screenshot_directory(window.get_directory());
        general.selected.project_open_policy(&app);
        selected_window_resize.write_player(&app);
        app.set_osd_enabled(window.get_osd_enabled());
        app.set_disable_osd_speed(window.get_disable_osd_speed());
        app.set_disable_osd_seek(window.get_disable_osd_seek());
        app.set_osc_position(window.get_osc_position().clamp(0, 2));
        app.set_show_chapter_pos(window.get_show_chapter_pos());
        app.set_compact_ui(window.get_compact_ui());
        if !app.get_edge_to_edge_video() {
            app.set_docked_controls(window.get_docked_controls());
        }
        let restart_notice = playback.restart_notice();
        let mut tone_mapping_applied = false;
        let mut video_threads_applied = false;
        let mut keep_open_applied = false;
        let result = crate::subtitle_position_preferences::apply(
            &decoder_player,
            subtitles.previous_position,
            subtitles.position,
        )
        .and_then(|()| {
            crate::subtitle_other_preferences::apply(
                &decoder_player,
                &subtitles.previous_other,
                &subtitles.other,
            )
        })
        .and_then(|()| decoder_player.send(Command::SubtitleAutoLoad(subtitles.auto_load.clone())))
        .and_then(|()| {
            crate::network_cache_preferences::apply(
                &decoder_player,
                &app,
                &playback.previous_cache,
                &playback.cache,
            )
        })
        .and_then(|()| {
            crate::network_ytdl_preferences::apply(
                &decoder_player,
                &app,
                &playback.previous_online_videos,
                &playback.online_videos,
            )
        })
        .and_then(|()| {
            if general.selected.keep_open_on_file_end != general.previous.keep_open_on_file_end
                || general.selected.playlist_auto_play_next
                    != general.previous.playlist_auto_play_next
            {
                decoder_player.send(Command::SetPlaybackEndPolicy {
                    keep_open_on_file_end: general.selected.keep_open_on_file_end,
                    playlist_auto_play_next: general.selected.playlist_auto_play_next,
                })?;
                keep_open_applied = true;
            }
            if playback.tone_mapping_changed {
                decoder_player.send(Command::SetToneMapping(playback.tone_mapping.clone()))?;
                tone_mapping_applied = true;
            }
            if playback.video_threads_changed {
                decoder_player.send(Command::VideoThreads(playback.video_threads))?;
                video_threads_applied = true;
            }
            subtitles.position.write_player(&app);
            subtitles.other.write_player(&app);
            subtitles.auto_load.write_player(&app);
            crate::screenshot_preferences::current(&app)
        })
        .and_then(|screenshot| {
            let mut preferences = riina_core::Preferences {
                screenshot,
                volume: f64::from(app.get_volume()).min(if app.get_volume_boost() {
                    200.0
                } else {
                    100.0
                }),
                volume_boost: app.get_volume_boost(),
                show_chapter_pos: window.get_show_chapter_pos(),
                allow_multiple_windows: window.get_allow_multiple_windows(),
                group_simultaneous_opens_in_playlist: window
                    .get_group_simultaneous_opens_in_playlist(),
                allow_duplicate_players: window.get_allow_duplicate_players(),
                live_text_enabled: window.get_live_text_enabled(),
                load_icc_profile: window.get_load_icc_profile(),
                hdr_enabled: window.get_default_hdr_mode_enabled(),
                disable_animations: window.get_disable_animations(),
                theme_material: Some(selected_theme),
                always_float_on_top: window.get_always_float_on_top(),
                always_show_on_top_icon: window.get_always_show_on_top_icon(),
                dark: selected_dark,
                record_recent_files: window.get_record_recent_files(),
                track_all_files_in_recent_open_menu: window
                    .get_track_all_files_in_recent_open_menu(),
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
                control_bar_position_horizontal: f64::from(
                    app.get_control_bar_position_horizontal(),
                ),
                control_bar_position_vertical: f64::from(app.get_control_bar_position_vertical()),
                hardware_decoder: decoder,
                subtitle_ass_override: applied_subtitles.primary,
                secondary_subtitle_ass_override: applied_subtitles.secondary,
                online_subtitles: selected_online_subtitles.clone(),
                subtitle_font: applied_subtitles.font.clone(),
                subtitle_font_size: applied_subtitles.font_size,
                subtitle_text_style: applied_subtitles.style,
                audio: playback.audio,
                video_threads: playback.video_threads,
                key_bindings: playback.key_bindings,
                ..baseline.clone()
            };
            general.put(&mut preferences)?;
            selected_thumbnail_preview.put(&mut preferences);
            selected_picture_in_picture.put(&mut preferences);
            selected_osd.put(&mut preferences);
            playback.tone_mapping.clone().put(&mut preferences);
            playback.network.put(&mut preferences);
            playback.advanced.put(&mut preferences);
            subtitles.auto_load.clone().put(&mut preferences);
            subtitles.other.clone().put(&mut preferences);
            subtitles.position.put(&mut preferences);
            playback.online_videos.clone().put(&mut preferences);
            playback.cache.clone().put(&mut preferences);
            selected_liquid_glass.put(&mut preferences);
            playback.controls.put(&mut preferences);
            selected_control_bar.put(&mut preferences);
            selected_window_resize.put(&mut preferences);
            // Utilities can clear history while this window remains open;
            // preserve that live system action over its original snapshot.
            crate::utilities_preferences::preserve_saved_history(&mut preferences, &path);
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
                window.set_allow_multiple_windows(saved.allow_multiple_windows);
                window.set_group_simultaneous_opens_in_playlist(
                    saved.group_simultaneous_opens_in_playlist,
                );
                window.set_allow_duplicate_players(saved.allow_duplicate_players);
                if keep_open_applied {
                    let _ = decoder_player.send(Command::SetPlaybackEndPolicy {
                        keep_open_on_file_end: general.previous.keep_open_on_file_end,
                        playlist_auto_play_next: general.previous.playlist_auto_play_next,
                    });
                }
                if video_threads_applied {
                    let _ =
                        decoder_player.send(Command::VideoThreads(playback.previous_video_threads));
                    window
                        .set_video_threads_text(playback.previous_video_threads.to_string().into());
                }
                if tone_mapping_applied {
                    let _ = decoder_player.send(Command::SetToneMapping(
                        playback.previous_tone_mapping.clone(),
                    ));
                }
                crate::network_ytdl_preferences::restore(
                    &decoder_player,
                    &app,
                    &playback.previous_online_videos,
                    &playback.online_videos,
                );
                crate::network_cache_preferences::restore(
                    &decoder_player,
                    &app,
                    &playback.previous_cache,
                    &playback.cache,
                );
                let _ = decoder_player.send(Command::SubtitleAutoLoad(
                    subtitles.previous_auto_load.clone(),
                ));
                let _ = crate::subtitle_other_preferences::apply(
                    &decoder_player,
                    &subtitles.other,
                    &subtitles.previous_other,
                );
                crate::subtitle_other_preferences::install_player(&app, &subtitles.previous_other);
                crate::subtitle_auto_load_preferences::install_player(
                    &app,
                    &subtitles.previous_auto_load,
                );
                let _ = crate::subtitle_position_preferences::apply(
                    &decoder_player,
                    subtitles.position,
                    subtitles.previous_position,
                );
                crate::subtitle_position_preferences::install_player(
                    &app,
                    subtitles.previous_position,
                );
                applied_subtitles.restore(&app, &decoder_player, &subtitles);
                applied_decoder.restore(&app, &decoder_player);
                general.previous.restore(&app);
                previous_osd.install_player(&app);
                app.set_osc_position(previous_layout.0);
                app.set_show_chapter_pos(previous_chapter_positions);
                app.set_compact_ui(previous_layout.1);
                app.set_docked_controls(previous_layout.2);
                crate::liquid_glass_preferences::install_player(&app, previous_liquid_glass);
                crate::window_resize_preferences::install_player(&app, previous_window_resize);
                crate::tone_mapping_preferences::install_window(
                    &window,
                    &playback.previous_tone_mapping,
                );
                crate::screenshot_preferences::install(&app, &previous);
                refresh(&window, &app);
                app.set_status(error.into());
            }
        }
    });
}

fn save_preferences(
    window: &PreferencesWindow,
    app: &PlayerWindow,
    model: &PreferenceModel,
    mut preferences: riina_core::Preferences,
) -> Result<(), String> {
    preferences.auto_switch_to_music_mode = window.get_auto_switch_to_music_mode();
    preferences.music_mode_show_playlist = window.get_music_mode_show_playlist();
    preferences.music_mode_show_album_art = window.get_music_mode_show_album_art();
    // These setters trigger automatic-mode and window-geometry callbacks. Apply
    // them only after saving, so a rejected edit cannot alter playback policy.
    let auto_switch = preferences.auto_switch_to_music_mode;
    let show_playlist = preferences.music_mode_show_playlist;
    let show_album_art = preferences.music_mode_show_album_art;
    model.save(preferences)?;
    app.set_auto_switch_to_music_mode(auto_switch);
    app.set_music_mode_show_playlist(show_playlist);
    app.set_music_mode_show_album_art(show_album_art);
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
