//! Validate and commit a preferences edit, restoring live state on failure.
use super::{live::*, refresh::refresh};
use crate::{PlayerWindow, PreferencesWindow};
use riina_core::Command;
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    path::PathBuf,
    sync::{Arc, Mutex},
};

pub(super) fn connect(
    app: &PlayerWindow,
    window: &PreferencesWindow,
    decoder_player: riina_mpv::CommandSender,
    path: PathBuf,
    baseline: riina_core::Preferences,
    key_binding_playback: Arc<Mutex<riina_core::Playback>>,
) {
    let parent = app.as_weak();
    let weak = window.as_weak();
    let writer = crate::preference_store::Writer::new(path.clone(), baseline.clone());
    let saved_open_policy = RefCell::new((
        baseline.allow_multiple_windows,
        baseline.group_simultaneous_opens_in_playlist,
        baseline.allow_duplicate_players,
    ));
    let saved_tone_mapping = RefCell::new(baseline.hdr_tone_mapping.clone());
    let saved_video_threads = RefCell::new(baseline.video_threads);
    window.on_settings_changed(move || {
        let (Some(app), Some(window)) = (parent.upgrade(), weak.upgrade()) else {
            return;
        };
        let previous = match crate::screenshot_preferences::current(&app) {
            Ok(settings) => settings,
            Err(error) => {
                app.set_status(error.into());
                return;
            }
        };
        let previous_pause = app.get_pause_when_open();
        let previous_play_when_entering_fullscreen = app.get_play_when_entering_fullscreen();
        let previous_pause_when_leaving_fullscreen = app.get_pause_when_leaving_fullscreen();
        let previous_pause_when_inactive = app.get_pause_when_inactive();
        let previous_pause_when_minimized = app.get_pause_when_minimized();
        let previous_pause_when_goes_to_sleep = app.get_pause_when_goes_to_sleep();
        let previous_quit_when_no_windows = app.get_quit_when_no_opened_window();
        let previous_keep_open = app.get_keep_open_on_file_end();
        let previous_playlist_auto_play_next = app.get_playlist_auto_play_next();
        let previous_playlist_auto_add = app.get_playlist_auto_add();
        let previous_playlist_show_metadata = app.get_playlist_show_metadata();
        let previous_playlist_show_metadata_in_music_mode =
            app.get_playlist_show_metadata_in_music_mode();
        let previous_auto_repeat = app.get_auto_repeat();
        let previous_default_repeat_mode = app.get_default_repeat_mode_index();
        let selected_quit_when_no_windows = window.get_quit_when_no_opened_window();
        let selected_play_when_entering_fullscreen = window.get_play_when_entering_fullscreen();
        let selected_pause_when_leaving_fullscreen = window.get_pause_when_leaving_fullscreen();
        let selected_pause_when_inactive = window.get_pause_when_inactive();
        let selected_pause_when_minimized = window.get_pause_when_minimized();
        let selected_pause_when_goes_to_sleep = window.get_pause_when_goes_to_sleep();
        let selected_keep_open = window.get_keep_open_on_file_end();
        let selected_playlist_auto_play_next = window.get_playlist_auto_play_next();
        let selected_playlist_auto_add = window.get_playlist_auto_add();
        let selected_playlist_show_metadata = window.get_playlist_show_metadata();
        let selected_playlist_show_metadata_in_music_mode =
            window.get_playlist_show_metadata_in_music_mode();
        let selected_auto_repeat = window.get_auto_repeat();
        let Some(selected_default_repeat_mode) =
            riina_core::DefaultRepeatMode::from_index(window.get_default_repeat_mode_index())
        else {
            app.set_status("Choose a valid default repeat mode".into());
            refresh(&window, &app);
            return;
        };
        let previous_fullscreen = app.get_fullscreen_when_open();
        let previous_resume = app.get_resume_last_position();
        let previous_record_playback_history = app.get_record_playback_history();
        let previous_open_policy = *saved_open_policy.borrow();
        let previous_launch_action = app.get_launch_action_index();
        let previous_osd = crate::osd_preferences::current_player(&app);
        let selected_osd = match crate::osd_preferences::current_preferences(&window) {
            Ok(settings) => settings,
            Err(error) => {
                app.set_status(error.into());
                refresh(&window, &app);
                return;
            }
        };
        let Some(selected_control_bar) = read_or_refresh(
            crate::osc_preferences::current_preferences(&window),
            &app,
            &window,
        ) else {
            return;
        };
        let Some(selected_picture_in_picture) = read_or_refresh(
            crate::pip_preferences::current_preferences(&window),
            &app,
            &window,
        ) else {
            return;
        };
        let Some(selected_thumbnail_preview) = read_or_refresh(
            crate::thumbnail_preferences::current_window(&window),
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
        let selected_liquid_glass = crate::liquid_glass_preferences::current_preferences(&window);
        let Some(selected_theme) =
            riina_core::ThemeMaterial::from_index(window.get_appearance_index())
        else {
            app.set_status("Choose a valid appearance".into());
            refresh(&window, &app);
            return;
        };
        let Some(selected_window_resize) = read_or_refresh(
            crate::window_resize_preferences::from_preferences(&window),
            &app,
            &window,
        ) else {
            return;
        };
        let selected_dark = selected_theme.dark_for(app.get_dark_theme());
        let previous_volume = app.get_volume();
        let previous_decoder = app.get_hardware_decoder_index();
        let previous_font = app.get_subtitle_font().to_string();
        let previous_font_size = crate::subtitle_preferences::current_font_size(&app);
        let Some(previous_text_style) = read_or_refresh(
            crate::subtitle_preferences::current_style(&app),
            &app,
            &window,
        ) else {
            return;
        };
        let previous_ass = (
            app.get_subtitle_ass_override_index(),
            app.get_secondary_subtitle_ass_override_index(),
        );
        let Some(previous_position) = read_or_refresh(
            crate::subtitle_position_preferences::current_player(&app),
            &app,
            &window,
        ) else {
            return;
        };
        let Some(selected_position) = read_or_refresh(
            crate::subtitle_position_preferences::current_window(&window),
            &app,
            &window,
        ) else {
            return;
        };
        let Some(previous_subtitle_other) = read_or_refresh(
            crate::subtitle_other_preferences::current_player(&app),
            &app,
            &window,
        ) else {
            return;
        };
        let Some(selected_subtitle_other) = read_or_refresh(
            crate::subtitle_other_preferences::current_window(&window),
            &app,
            &window,
        ) else {
            return;
        };
        let Some(previous_auto_load) = read_or_refresh(
            crate::subtitle_auto_load_preferences::current_player(&app),
            &app,
            &window,
        ) else {
            return;
        };
        let Some(selected_auto_load) = read_or_refresh(
            crate::subtitle_auto_load_preferences::current_window(&window),
            &app,
            &window,
        ) else {
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
        let Some(previous_network_cache) = read_or_refresh(
            crate::network_cache_preferences::current_player(&app),
            &app,
            &window,
        ) else {
            return;
        };
        let Some(selected_network_cache) = read_or_refresh(
            crate::network_cache_preferences::current_window(&window),
            &app,
            &window,
        ) else {
            return;
        };
        let Some(selected_network) = read_or_refresh(
            crate::network_connection_preferences::current_window(&window),
            &app,
            &window,
        ) else {
            return;
        };
        let Some(selected_audio) = read_or_refresh(
            crate::audio_preferences::current_window(&window),
            &app,
            &window,
        ) else {
            return;
        };
        let selected_tone_mapping = match crate::tone_mapping_preferences::current_window(&window) {
            Ok(settings) => settings,
            Err(error) => {
                let saved = saved_tone_mapping.borrow().clone();
                crate::tone_mapping_preferences::install_window(&window, &saved);
                app.set_status(error.into());
                return;
            }
        };
        let previous_tone_mapping = saved_tone_mapping.borrow().clone();
        let tone_mapping_changed = selected_tone_mapping != previous_tone_mapping;
        let selected_video_threads = match window.get_video_threads_text().trim().parse::<u32>() {
            Ok(threads) if threads <= i32::MAX as u32 => threads,
            _ => {
                window.set_video_threads_text(saved_video_threads.borrow().to_string().into());
                app.set_status("Video decoder threads must be between 0 and 2,147,483,647".into());
                return;
            }
        };
        let previous_video_threads = *saved_video_threads.borrow();
        let video_threads_changed = selected_video_threads != previous_video_threads;
        let selected_controls = match crate::control_preferences::current_window(&window) {
            Ok(preferences) => preferences,
            Err(error) => {
                app.set_status(error.into());
                refresh(&window, &app);
                return;
            }
        };
        let Some(selected_key_bindings) = read_or_refresh(
            crate::key_binding_preferences::current_window(&window),
            &app,
            &window,
        ) else {
            return;
        };
        let Some(selected_advanced) = read_or_refresh(
            crate::advanced_preferences::current_window(&window),
            &app,
            &window,
        ) else {
            return;
        };
        let advanced_changed = selected_advanced != baseline.advanced;
        let audio_changed = selected_audio != baseline.audio;
        let Some(previous_network_ytdl) = read_or_refresh(
            crate::network_ytdl_preferences::current_player(&app),
            &app,
            &window,
        ) else {
            return;
        };
        let Some(selected_network_ytdl) = read_or_refresh(
            crate::network_ytdl_preferences::current_window(&window),
            &app,
            &window,
        ) else {
            return;
        };
        let decoder_index = window.get_hardware_decoder_index();
        let Some(decoder) = crate::codec_preferences::hardware_decoder(decoder_index) else {
            app.set_status("Invalid hardware decoder selection".into());
            refresh(&window, &app);
            return;
        };
        let mut decoder_applied = false;
        let volume_boost = window.get_volume_boost();
        let mut volume_boost_applied = false;
        if volume_boost != previous_volume_boost {
            if let Err(error) = decoder_player.send(Command::SetVolumeBoost(volume_boost)) {
                app.set_status(error.into());
                refresh(&window, &app);
                return;
            }
            if !volume_boost {
                let capped_volume = previous_volume.min(100.0);
                if capped_volume != previous_volume
                    && let Err(error) = decoder_player.send(Command::Volume(capped_volume as f64))
                {
                    let _ = decoder_player.send(Command::SetVolumeBoost(previous_volume_boost));
                    app.set_status(error.into());
                    refresh(&window, &app);
                    return;
                }
                app.set_volume(capped_volume);
            }
            app.set_volume_boost(volume_boost);
            volume_boost_applied = true;
        }
        if decoder_index != previous_decoder {
            if let Err(error) = decoder_player.send(Command::HardwareDecoding(decoder)) {
                if volume_boost_applied {
                    let _ = decoder_player.send(Command::SetVolumeBoost(previous_volume_boost));
                    if previous_volume != app.get_volume() {
                        let _ = decoder_player.send(Command::Volume(previous_volume as f64));
                    }
                    app.set_volume_boost(previous_volume_boost);
                    app.set_volume(previous_volume);
                }
                app.set_status(error.into());
                refresh(&window, &app);
                return;
            }
            app.set_hardware_decoder_index(decoder_index);
            decoder_applied = true;
        }
        let desired_ass = (
            window.get_subtitle_ass_override_primary_index(),
            window.get_subtitle_ass_override_secondary_index(),
        );
        let desired_font = window.get_subtitle_font().to_string();
        let desired_font_size = match crate::subtitle_preferences::font_size(
            window.get_subtitle_font_size_text().as_str(),
        ) {
            Ok(size) => size,
            Err(error) => {
                restore_live_volume_and_decoder(
                    &app,
                    &decoder_player,
                    volume_boost_applied,
                    previous_volume_boost,
                    previous_volume,
                    decoder_applied,
                    previous_decoder,
                );
                app.set_status(error.into());
                refresh(&window, &app);
                return;
            }
        };
        let (subtitle_font, subtitle_font_size) = match apply_subtitle_text_preferences(
            &app,
            &decoder_player,
            &previous_font,
            previous_font_size,
            &desired_font,
            desired_font_size,
        ) {
            Ok(values) => values,
            Err(error) => {
                restore_live_volume_and_decoder(
                    &app,
                    &decoder_player,
                    volume_boost_applied,
                    previous_volume_boost,
                    previous_volume,
                    decoder_applied,
                    previous_decoder,
                );
                app.set_status(error.into());
                refresh(&window, &app);
                return;
            }
        };
        let desired_text_style = match crate::subtitle_preferences::current_window_style(&window) {
            Ok(style) => style,
            Err(error) => {
                restore_subtitle_text_preferences(
                    &app,
                    &decoder_player,
                    &previous_font,
                    previous_font_size,
                    &subtitle_font,
                    subtitle_font_size,
                );
                restore_live_volume_and_decoder(
                    &app,
                    &decoder_player,
                    volume_boost_applied,
                    previous_volume_boost,
                    previous_volume,
                    decoder_applied,
                    previous_decoder,
                );
                app.set_status(error.into());
                refresh(&window, &app);
                return;
            }
        };
        if let Err(error) = apply_text_style(
            &app,
            &decoder_player,
            previous_text_style,
            desired_text_style,
        ) {
            restore_subtitle_text_preferences(
                &app,
                &decoder_player,
                &previous_font,
                previous_font_size,
                &subtitle_font,
                subtitle_font_size,
            );
            restore_live_volume_and_decoder(
                &app,
                &decoder_player,
                volume_boost_applied,
                previous_volume_boost,
                previous_volume,
                decoder_applied,
                previous_decoder,
            );
            app.set_status(error.into());
            refresh(&window, &app);
            return;
        }
        let (ass_primary, ass_secondary) =
            match apply_ass_override(&app, &decoder_player, previous_ass, desired_ass) {
                Ok(levels) => levels,
                Err(error) => {
                    restore_text_style(
                        &app,
                        &decoder_player,
                        previous_text_style,
                        desired_text_style,
                    );
                    restore_subtitle_text_preferences(
                        &app,
                        &decoder_player,
                        &previous_font,
                        previous_font_size,
                        &subtitle_font,
                        subtitle_font_size,
                    );
                    restore_live_volume_and_decoder(
                        &app,
                        &decoder_player,
                        volume_boost_applied,
                        previous_volume_boost,
                        previous_volume,
                        decoder_applied,
                        previous_decoder,
                    );
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
        app.set_pause_when_open(window.get_pause_when_open());
        app.set_fullscreen_when_open(window.get_fullscreen_when_open());
        app.set_resume_last_position(window.get_resume_last_position());
        app.set_record_playback_history(window.get_record_playback_history());
        app.set_launch_action_index(window.get_launch_action_index().clamp(0, 2));
        crate::window_resize_preferences::install_player(&app, selected_window_resize);
        app.set_osd_enabled(window.get_osd_enabled());
        app.set_disable_osd_speed(window.get_disable_osd_speed());
        app.set_disable_osd_seek(window.get_disable_osd_seek());
        app.set_osc_position(window.get_osc_position().clamp(0, 2));
        app.set_show_chapter_pos(window.get_show_chapter_pos());
        app.set_compact_ui(window.get_compact_ui());
        if !app.get_edge_to_edge_video() {
            app.set_docked_controls(window.get_docked_controls());
        }
        let mut tone_mapping_applied = false;
        let mut video_threads_applied = false;
        let mut keep_open_applied = false;
        let result = crate::subtitle_position_preferences::apply(
            &decoder_player,
            previous_position,
            selected_position,
        )
        .and_then(|()| {
            crate::subtitle_other_preferences::apply(
                &decoder_player,
                &previous_subtitle_other,
                &selected_subtitle_other,
            )
        })
        .and_then(|()| decoder_player.send(Command::SubtitleAutoLoad(selected_auto_load.clone())))
        .and_then(|()| {
            crate::network_cache_preferences::apply(
                &decoder_player,
                &app,
                &previous_network_cache,
                &selected_network_cache,
            )
        })
        .and_then(|()| {
            crate::network_ytdl_preferences::apply(
                &decoder_player,
                &app,
                &previous_network_ytdl,
                &selected_network_ytdl,
            )
        })
        .and_then(|()| {
            if selected_keep_open != previous_keep_open
                || selected_playlist_auto_play_next != previous_playlist_auto_play_next
            {
                decoder_player.send(Command::SetPlaybackEndPolicy {
                    keep_open_on_file_end: selected_keep_open,
                    playlist_auto_play_next: selected_playlist_auto_play_next,
                })?;
                keep_open_applied = true;
            }
            if tone_mapping_changed {
                decoder_player.send(Command::SetToneMapping(selected_tone_mapping.clone()))?;
                tone_mapping_applied = true;
            }
            if video_threads_changed {
                decoder_player.send(Command::VideoThreads(selected_video_threads))?;
                video_threads_applied = true;
            }
            crate::subtitle_position_preferences::install_player(&app, selected_position);
            crate::subtitle_other_preferences::install_player(&app, &selected_subtitle_other);
            crate::subtitle_auto_load_preferences::install_player(&app, &selected_auto_load);
            crate::screenshot_preferences::current(&app)
        })
        .and_then(|screenshot| {
            let mut preferences = riina_core::Preferences {
                screenshot,
                thumbnail_preview: selected_thumbnail_preview.clone(),
                volume: f64::from(app.get_volume()).min(if app.get_volume_boost() {
                    200.0
                } else {
                    100.0
                }),
                volume_boost: app.get_volume_boost(),
                show_chapter_pos: window.get_show_chapter_pos(),
                picture_in_picture: selected_picture_in_picture,
                action_after_launch: riina_core::ActionAfterLaunch::from_index(
                    window.get_launch_action_index(),
                )
                .ok_or_else(|| "Choose a valid launch action".to_owned())?,
                allow_multiple_windows: window.get_allow_multiple_windows(),
                group_simultaneous_opens_in_playlist: window
                    .get_group_simultaneous_opens_in_playlist(),
                allow_duplicate_players: window.get_allow_duplicate_players(),
                playlist_auto_add: selected_playlist_auto_add,
                playlist_auto_play_next: selected_playlist_auto_play_next,
                playlist_show_metadata: selected_playlist_show_metadata,
                playlist_show_metadata_in_music_mode: selected_playlist_show_metadata_in_music_mode,
                auto_repeat: selected_auto_repeat,
                default_repeat_mode: selected_default_repeat_mode,
                play_when_entering_fullscreen: selected_play_when_entering_fullscreen,
                pause_when_leaving_fullscreen: selected_pause_when_leaving_fullscreen,
                pause_when_inactive: selected_pause_when_inactive,
                pause_when_minimized: selected_pause_when_minimized,
                pause_when_goes_to_sleep: selected_pause_when_goes_to_sleep,
                quit_when_no_opened_window: selected_quit_when_no_windows,
                keep_open_on_file_end: selected_keep_open,
                live_text_enabled: window.get_live_text_enabled(),
                load_icc_profile: window.get_load_icc_profile(),
                hdr_enabled: window.get_default_hdr_mode_enabled(),
                disable_animations: window.get_disable_animations(),
                theme_material: Some(selected_theme),
                always_float_on_top: window.get_always_float_on_top(),
                always_show_on_top_icon: window.get_always_show_on_top_icon(),
                dark: selected_dark,
                pause_when_open: app.get_pause_when_open(),
                fullscreen_when_open: app.get_fullscreen_when_open(),
                resume_last_position: app.get_resume_last_position(),
                record_playback_history: app.get_record_playback_history(),
                record_recent_files: window.get_record_recent_files(),
                track_all_files_in_recent_open_menu: window
                    .get_track_all_files_in_recent_open_menu(),
                enable_osd: selected_osd.enabled,
                disable_osd_file_start: selected_osd.disable_file_start,
                disable_osd_pause_resume: selected_osd.disable_pause_resume,
                disable_osd_speed: selected_osd.disable_speed,
                disable_osd_seek: selected_osd.disable_seek,
                osd_timeout_secs: selected_osd.timeout_secs,
                osd_text_size: selected_osd.text_size,
                control_bar: selected_control_bar,
                display_time_and_battery_in_fullscreen: selected_osd
                    .display_time_and_battery_in_fullscreen,
                leading_sidebar_width: app.get_leading_sidebar_width() as u32,
                trailing_sidebar_width: app.get_trailing_sidebar_width() as u32,
                settings_at_leading: app.get_settings_at_leading(),
                playlist_at_leading: app.get_playlist_at_leading(),
                plugins_at_leading: app.get_plugins_at_leading(),
                edge_to_edge_video: app.get_edge_to_edge_video(),
                docked_controls: app.get_docked_controls(),
                unlock_window_aspect_ratio: app.get_unlock_window_aspect_ratio(),
                window_resize: selected_window_resize,
                compact_ui: app.get_compact_ui(),
                osc_position: app.get_osc_position() as u8,
                control_bar_position_horizontal: f64::from(
                    app.get_control_bar_position_horizontal(),
                ),
                control_bar_position_vertical: f64::from(app.get_control_bar_position_vertical()),
                hardware_decoder: decoder,
                subtitle_ass_override: ass_primary,
                secondary_subtitle_ass_override: ass_secondary,
                subtitle_position: selected_position,
                subtitle_other: selected_subtitle_other.clone(),
                subtitle_auto_load: selected_auto_load.clone(),
                online_subtitles: selected_online_subtitles.clone(),
                subtitle_font: subtitle_font.clone(),
                subtitle_font_size,
                subtitle_text_style: desired_text_style,
                network: selected_network,
                audio: selected_audio,
                hdr_tone_mapping: selected_tone_mapping.clone(),
                video_threads: selected_video_threads,
                advanced: selected_advanced,
                controls: selected_controls.clone(),
                key_bindings: selected_key_bindings.clone(),
                network_cache: selected_network_cache.clone(),
                network_online_videos: selected_network_ytdl.clone(),
                liquid_glass: selected_liquid_glass,
                ..baseline.clone()
            };
            // Utilities can clear history while this window remains open;
            // preserve that live system action over its original snapshot.
            crate::utilities_preferences::preserve_saved_history(&mut preferences, &path);
            save_preferences(&window, &app, &writer, preferences)
        });
        match result {
            Ok(()) => {
                *saved_open_policy.borrow_mut() = (
                    window.get_allow_multiple_windows(),
                    window.get_group_simultaneous_opens_in_playlist(),
                    window.get_allow_duplicate_players(),
                );
                app.set_show_chapter_pos(window.get_show_chapter_pos());
                app.set_play_when_entering_fullscreen(selected_play_when_entering_fullscreen);
                app.set_pause_when_leaving_fullscreen(selected_pause_when_leaving_fullscreen);
                app.set_pause_when_inactive(selected_pause_when_inactive);
                app.set_pause_when_minimized(selected_pause_when_minimized);
                app.set_pause_when_goes_to_sleep(selected_pause_when_goes_to_sleep);
                app.set_quit_when_no_opened_window(selected_quit_when_no_windows);
                app.set_keep_open_on_file_end(selected_keep_open);
                app.set_record_recent_files(window.get_record_recent_files());
                app.set_track_all_files_in_recent_open_menu(
                    window.get_track_all_files_in_recent_open_menu(),
                );
                app.set_playlist_auto_play_next(selected_playlist_auto_play_next);
                app.set_playlist_auto_add(selected_playlist_auto_add);
                app.set_playlist_show_metadata(selected_playlist_show_metadata);
                app.set_playlist_show_metadata_in_music_mode(
                    selected_playlist_show_metadata_in_music_mode,
                );
                app.set_auto_repeat(selected_auto_repeat);
                app.set_default_repeat_mode_index(selected_default_repeat_mode.index());
                crate::thumbnail_preferences::install_player(&app, &selected_thumbnail_preview);
                if selected_online_subtitles.api_key == app.get_open_subtitles_api_key().as_str() {
                    crate::online_subtitles_preferences::install_player(
                        &app,
                        &selected_online_subtitles,
                    );
                } else {
                    match crate::online_subtitles_credentials::store(
                        &selected_online_subtitles.api_key,
                    ) {
                        Ok(()) => crate::online_subtitles_preferences::install_player(
                            &app,
                            &selected_online_subtitles,
                        ),
                        Err(error) => {
                            let mut retained = selected_online_subtitles.clone();
                            retained.api_key = app.get_open_subtitles_api_key().to_string();
                            crate::online_subtitles_preferences::install_player(&app, &retained);
                            window.set_open_subtitles_api_key(retained.api_key.into());
                            app.set_status(error.into());
                        }
                    }
                }
                if selected_online_subtitles.assrt_token != previous_assrt_token
                    && let Err(error) = crate::online_subtitles_credentials::store_assrt(
                        &selected_online_subtitles.assrt_token,
                    )
                {
                    let mut retained = selected_online_subtitles.clone();
                    retained.api_key = app.get_open_subtitles_api_key().to_string();
                    retained.assrt_token = previous_assrt_token.clone();
                    crate::online_subtitles_preferences::install_player(&app, &retained);
                    window.set_assrt_token(retained.assrt_token.into());
                    app.set_status(error.into());
                }
                crate::pip_preferences::install_player(&app, selected_picture_in_picture);
                app.set_live_text_enabled(window.get_live_text_enabled());
                app.set_load_icc_profile(window.get_load_icc_profile());
                app.set_default_hdr_mode_enabled(window.get_default_hdr_mode_enabled());
                app.set_disable_animations(window.get_disable_animations());
                *saved_tone_mapping.borrow_mut() = selected_tone_mapping.clone();
                *saved_video_threads.borrow_mut() = selected_video_threads;
                app.set_appearance_mode(selected_theme.index());
                app.set_dark_theme(selected_dark);
                selected_osd.install_player(&app);
                crate::osc_preferences::install_player(&app, selected_control_bar);
                app.set_always_float_on_top(window.get_always_float_on_top());
                app.set_always_show_on_top_icon(window.get_always_show_on_top_icon());
                crate::control_preferences::install_player(&app, &selected_controls);
                crate::liquid_glass_preferences::install_player(&app, selected_liquid_glass);
                crate::key_binding_preferences::install_player(
                    &app,
                    &selected_key_bindings,
                    key_binding_playback.clone(),
                );
                if advanced_changed {
                    app.set_status("Advanced settings apply after restarting Riina".into());
                } else if audio_changed {
                    app.set_status("Audio engine preferences apply after restarting Riina".into());
                }
                if app.get_pause_when_open() != previous_pause {
                    app.invoke_pause_when_open_changed(app.get_pause_when_open());
                }
                if app.get_resume_last_position() != previous_resume {
                    app.invoke_resume_last_position_changed(app.get_resume_last_position());
                }
                if app.get_record_playback_history() != previous_record_playback_history {
                    app.invoke_record_playback_history_changed(app.get_record_playback_history());
                }
            }
            Err(error) => {
                window.set_allow_multiple_windows(previous_open_policy.0);
                window.set_group_simultaneous_opens_in_playlist(previous_open_policy.1);
                window.set_allow_duplicate_players(previous_open_policy.2);
                if keep_open_applied {
                    let _ = decoder_player.send(Command::SetPlaybackEndPolicy {
                        keep_open_on_file_end: previous_keep_open,
                        playlist_auto_play_next: previous_playlist_auto_play_next,
                    });
                }
                if video_threads_applied {
                    let _ = decoder_player.send(Command::VideoThreads(previous_video_threads));
                    window.set_video_threads_text(previous_video_threads.to_string().into());
                }
                if tone_mapping_applied {
                    let _ =
                        decoder_player.send(Command::SetToneMapping(previous_tone_mapping.clone()));
                }
                crate::network_ytdl_preferences::restore(
                    &decoder_player,
                    &app,
                    &previous_network_ytdl,
                    &selected_network_ytdl,
                );
                crate::network_cache_preferences::restore(
                    &decoder_player,
                    &app,
                    &previous_network_cache,
                    &selected_network_cache,
                );
                let _ = decoder_player.send(Command::SubtitleAutoLoad(previous_auto_load.clone()));
                let _ = crate::subtitle_other_preferences::apply(
                    &decoder_player,
                    &selected_subtitle_other,
                    &previous_subtitle_other,
                );
                crate::subtitle_other_preferences::install_player(&app, &previous_subtitle_other);
                crate::subtitle_auto_load_preferences::install_player(&app, &previous_auto_load);
                let _ = crate::subtitle_position_preferences::apply(
                    &decoder_player,
                    selected_position,
                    previous_position,
                );
                crate::subtitle_position_preferences::install_player(&app, previous_position);
                restore_text_style(
                    &app,
                    &decoder_player,
                    previous_text_style,
                    desired_text_style,
                );
                restore_subtitle_text_preferences(
                    &app,
                    &decoder_player,
                    &previous_font,
                    previous_font_size,
                    &subtitle_font,
                    subtitle_font_size,
                );
                restore_ass_override(&app, &decoder_player, previous_ass, desired_ass);
                restore_live_volume_and_decoder(
                    &app,
                    &decoder_player,
                    volume_boost_applied,
                    previous_volume_boost,
                    previous_volume,
                    decoder_applied,
                    previous_decoder,
                );
                app.set_pause_when_open(previous_pause);
                app.set_play_when_entering_fullscreen(previous_play_when_entering_fullscreen);
                app.set_pause_when_leaving_fullscreen(previous_pause_when_leaving_fullscreen);
                app.set_pause_when_inactive(previous_pause_when_inactive);
                app.set_pause_when_minimized(previous_pause_when_minimized);
                app.set_pause_when_goes_to_sleep(previous_pause_when_goes_to_sleep);
                app.set_quit_when_no_opened_window(previous_quit_when_no_windows);
                app.set_keep_open_on_file_end(previous_keep_open);
                app.set_playlist_auto_play_next(previous_playlist_auto_play_next);
                app.set_playlist_auto_add(previous_playlist_auto_add);
                app.set_playlist_show_metadata(previous_playlist_show_metadata);
                app.set_playlist_show_metadata_in_music_mode(
                    previous_playlist_show_metadata_in_music_mode,
                );
                app.set_auto_repeat(previous_auto_repeat);
                app.set_default_repeat_mode_index(previous_default_repeat_mode);
                app.set_fullscreen_when_open(previous_fullscreen);
                app.set_resume_last_position(previous_resume);
                app.set_record_playback_history(previous_record_playback_history);
                app.set_launch_action_index(previous_launch_action);
                previous_osd.install_player(&app);
                app.set_osc_position(previous_layout.0);
                app.set_show_chapter_pos(previous_chapter_positions);
                app.set_compact_ui(previous_layout.1);
                app.set_docked_controls(previous_layout.2);
                crate::liquid_glass_preferences::install_player(&app, previous_liquid_glass);
                crate::window_resize_preferences::install_player(&app, previous_window_resize);
                crate::tone_mapping_preferences::install_window(&window, &previous_tone_mapping);
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
    writer: &crate::preference_store::Writer,
    mut preferences: riina_core::Preferences,
) -> Result<(), String> {
    preferences.auto_switch_to_music_mode = window.get_auto_switch_to_music_mode();
    preferences.music_mode_show_playlist = window.get_music_mode_show_playlist();
    preferences.music_mode_show_album_art = window.get_music_mode_show_album_art();
    writer.save(&preferences)?;
    // These setters trigger automatic-mode and window-geometry callbacks. Apply
    // them only after saving, so a rejected edit cannot alter playback policy.
    app.set_auto_switch_to_music_mode(preferences.auto_switch_to_music_mode);
    app.set_music_mode_show_playlist(preferences.music_mode_show_playlist);
    app.set_music_mode_show_album_art(preferences.music_mode_show_album_art);
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
