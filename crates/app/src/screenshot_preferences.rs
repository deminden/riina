//! The UI owns live preferences; capture and session saving read the same intent.
use crate::PlayerWindow;
use riina_core::{
    Command,
    screenshot::{Format, Settings},
};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{cell::RefCell, path::PathBuf, rc::Rc, time::Duration};

pub(crate) fn connect(
    app: &PlayerWindow,
    decoder_player: riina_mpv::CommandSender,
    path: PathBuf,
    baseline: riina_core::Preferences,
    latest_playback: std::sync::Arc<std::sync::Mutex<riina_core::Playback>>,
    clear_history: impl Fn() -> Result<usize, String> + Clone + 'static,
) {
    let current = Rc::new(RefCell::new(None::<crate::PreferencesWindow>));
    let audio_device_refresh = Rc::new(slint::Timer::default());
    let observed_audio_devices = Rc::new(RefCell::new(Vec::new()));
    let preferences_window = current.clone();
    app.on_subtitle_font_selected(move |font| {
        if let Some(window) = preferences_window
            .borrow()
            .as_ref()
            .map(ComponentHandle::clone_strong)
        {
            window.set_subtitle_font(font);
            window.invoke_settings_changed();
        }
    });
    let parent = app.as_weak();
    app.on_open_preferences(move || {
        let Some(app) = parent.upgrade() else { return };
        if app.get_crop_editing() || app.get_dialog_open() {
            return;
        }
        let existing = current.borrow().as_ref().map(ComponentHandle::clone_strong);
        let window = if let Some(window) = existing {
            window
        } else {
            let window = match crate::PreferencesWindow::new() {
                Ok(window) => window,
                Err(error) => {
                    app.set_status(error.to_string().into());
                    return;
                }
            };
            crate::settings_search::connect(&window);
            crate::utilities_preferences::connect(&window, &path, clear_history.clone());
            crate::plugin_preferences::connect(&window, &app, &path);
            crate::advanced_preferences::install_window(&window, &baseline.advanced);
            crate::advanced_preferences::connect_actions(&app, &window, path.clone());
            let available_devices = latest_playback
                .lock()
                .map(|playback| playback.audio_devices.clone())
                .unwrap_or_default();
            crate::audio_preferences::install_window(&window, &baseline.audio, &available_devices);
            crate::tone_mapping_preferences::install_window(&window, &baseline.hdr_tone_mapping);
            window.set_video_threads_text(baseline.video_threads.to_string().into());
            crate::control_preferences::install_window(&window, &baseline.controls);
            if let Err(error) =
                crate::key_binding_preferences::install_window(&window, &baseline.key_bindings)
            {
                app.set_status(error.into());
                return;
            }
            crate::network_connection_preferences::install_window(&window, &baseline.network);
            crate::network_ytdl_preferences::install_window(
                &window,
                &baseline.network_online_videos,
            );
            crate::pip_preferences::install_preferences(&window, baseline.picture_in_picture);
            crate::window_resize_preferences::install_preferences(&window, baseline.window_resize);
            let parent = app.as_weak();
            let weak = window.as_weak();
            let path = path.clone();
            let baseline = baseline.clone();
            let saved_tone_mapping = Rc::new(RefCell::new(baseline.hdr_tone_mapping.clone()));
            let saved_tone_mapping_for_change = saved_tone_mapping.clone();
            let saved_video_threads = Rc::new(RefCell::new(baseline.video_threads));
            let saved_video_threads_for_change = saved_video_threads.clone();
            let decoder_player = decoder_player.clone();
            let selected_profile_window = window.as_weak();
            let selected_profile_parent = app.as_weak();
            window.on_key_binding_profile_selected(move |name| {
                let Some(window) = selected_profile_window.upgrade() else {
                    return;
                };
                if let Err(error) =
                    crate::key_binding_preferences::select_profile(&window, name.as_str())
                    && let Some(app) = selected_profile_parent.upgrade()
                {
                    app.set_status(error.into());
                }
            });
            let settings_refresh_window = window.as_weak();
            let settings_refresh_parent = app.as_weak();
            window.on_key_binding_settings_changed(move || {
                let Some(window) = settings_refresh_window.upgrade() else {
                    return;
                };
                let profile = window.get_key_binding_current_profile().to_string();
                if let Err(error) =
                    crate::key_binding_preferences::select_profile(&window, &profile)
                    && let Some(app) = settings_refresh_parent.upgrade()
                {
                    app.set_status(error.into());
                }
            });
            let search_window = window.as_weak();
            let search_parent = app.as_weak();
            window.on_key_binding_search_changed(move |query| {
                let Some(window) = search_window.upgrade() else {
                    return;
                };
                if let Err(error) = crate::key_binding_preferences::search(&window, query.as_str())
                    && let Some(app) = search_parent.upgrade()
                {
                    app.set_status(error.into());
                }
            });
            let delete_window = window.as_weak();
            let delete_parent = app.as_weak();
            window.on_key_binding_delete_profile(move || {
                let Some(window) = delete_window.upgrade() else {
                    return;
                };
                let profile = window.get_key_binding_current_profile().to_string();
                let result = crate::key_binding_profiles::ProfileStore::for_current_user()
                    .and_then(|store| store.delete(&profile))
                    .map_err(|error| error.to_string());
                if let Err(error) = result {
                    if let Some(app) = delete_parent.upgrade() {
                        app.set_status(error.into());
                    }
                    return;
                }
                let mut settings = match crate::key_binding_preferences::current_window(&window) {
                    Ok(settings) => settings,
                    Err(error) => {
                        if let Some(app) = delete_parent.upgrade() {
                            app.set_status(error.into());
                        }
                        return;
                    }
                };
                settings.current_profile = "IINA Default".into();
                match crate::key_binding_preferences::refresh(&window, &settings) {
                    Ok(()) => window.invoke_settings_changed(),
                    Err(error) => {
                        if let Some(app) = delete_parent.upgrade() {
                            app.set_status(error.into());
                        }
                    }
                }
            });
            let reveal_window = window.as_weak();
            let reveal_parent = app.as_weak();
            window.on_key_binding_reveal_profile(move || {
                let Some(window) = reveal_window.upgrade() else {
                    return;
                };
                let name = window.get_key_binding_current_profile().to_string();
                let result = crate::key_binding_profiles::ProfileStore::for_current_user()
                    .map_err(|error| error.to_string())
                    .and_then(|store| {
                        let profile = store.path_for(&name);
                        if let Ok(path) = profile
                            && path.is_file()
                        {
                            return crate::platform::reveal_file(&path)
                                .map_err(|error| error.to_string());
                        }
                        crate::platform::open_directory(store.directory())
                            .map_err(|error| error.to_string())
                    });
                if let Err(error) = result
                    && let Some(app) = reveal_parent.upgrade()
                {
                    app.set_status(error.into());
                }
            });
            let add_window = window.as_weak();
            window.on_key_binding_add_requested(move || {
                let Some(window) = add_window.upgrade() else {
                    return;
                };
                if window.get_key_binding_profile_index() < 5 {
                    return;
                }
                window.set_key_binding_editor_row(-1);
                window.set_key_binding_editor_key("".into());
                window.set_key_binding_editor_action("".into());
                window.set_key_binding_editor_comment("".into());
                window.set_key_binding_editor_kind(0);
                window.set_key_binding_editor_open(true);
            });
            let edit_window = window.as_weak();
            let edit_parent = app.as_weak();
            window.on_key_binding_edit_requested(move |row| {
                let Some(window) = edit_window.upgrade() else {
                    return;
                };
                match crate::key_binding_preferences::mapping_for_row(&window, row) {
                    Ok(mapping) => {
                        window.set_key_binding_editor_row(row);
                        window.set_key_binding_editor_key(mapping.key.into());
                        window.set_key_binding_editor_action(mapping.action.into());
                        window.set_key_binding_editor_comment(
                            mapping.comment.unwrap_or_default().into(),
                        );
                        window.set_key_binding_editor_kind(
                            if mapping.kind == riina_core::key_bindings::ActionKind::Riina {
                                1
                            } else {
                                0
                            },
                        );
                        window.set_key_binding_editor_open(true);
                    }
                    Err(error) => {
                        if let Some(app) = edit_parent.upgrade() {
                            app.set_status(error.into());
                        }
                    }
                }
            });
            let key_window = window.as_weak();
            window.on_key_binding_key_recorded(move |text, control, alt, shift, meta| {
                if let Some(window) = key_window.upgrade()
                    && let Some(key) = crate::key_binding_preferences::recorded_key(
                        text.as_str(),
                        control,
                        alt,
                        shift,
                        meta,
                    )
                {
                    window.set_key_binding_editor_key(key.into());
                }
            });
            let remove_window = window.as_weak();
            let remove_parent = app.as_weak();
            window.on_key_binding_remove_requested(move |row| {
                let Some(window) = remove_window.upgrade() else {
                    return;
                };
                match crate::key_binding_preferences::remove_mapping(&window, row) {
                    Ok(()) => window.invoke_settings_changed(),
                    Err(error) => {
                        if let Some(app) = remove_parent.upgrade() {
                            app.set_status(error.into());
                        }
                    }
                }
            });
            let submit_window = window.as_weak();
            let submit_parent = app.as_weak();
            window.on_key_binding_mapping_submitted(move |key, action, comment, kind, row| {
                let Some(window) = submit_window.upgrade() else {
                    return;
                };
                match crate::key_binding_preferences::save_mapping(
                    &window,
                    row,
                    key.as_str(),
                    action.as_str(),
                    comment.as_str(),
                    kind,
                ) {
                    Ok(()) => {
                        window.set_key_binding_editor_open(false);
                        window.invoke_key_binding_settings_changed();
                        window.invoke_settings_changed();
                    }
                    Err(error) => {
                        if let Some(app) = submit_parent.upgrade() {
                            app.set_status(error.into());
                        }
                    }
                }
            });
            let cancel_window = window.as_weak();
            window.on_key_binding_editor_cancelled(move || {
                if let Some(window) = cancel_window.upgrade() {
                    window.set_key_binding_editor_open(false);
                }
            });
            let selected_font = app.get_subtitle_font().to_string();
            window.set_subtitle_font(selected_font.into());
            let choose_parent = app.as_weak();
            let choose_window = window.as_weak();
            window.on_choose_subtitle_font(move || {
                if let (Some(app), Some(window)) =
                    (choose_parent.upgrade(), choose_window.upgrade())
                {
                    window.set_subtitle_font(app.get_subtitle_font());
                    app.invoke_choose_subtitle_font();
                }
            });
            window.on_settings_changed(move || {
                let (Some(app), Some(window)) = (parent.upgrade(), weak.upgrade()) else {
                    return;
                };
                let previous = match self::current(&app) {
                    Ok(settings) => settings,
                    Err(error) => {
                        app.set_status(error.into());
                        return;
                    }
                };
                let previous_pause = app.get_pause_when_open();
                let previous_fullscreen = app.get_fullscreen_when_open();
                let previous_resume = app.get_resume_last_position();
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
                let selected_picture_in_picture =
                    match crate::pip_preferences::current_preferences(&window) {
                        Ok(settings) => settings,
                        Err(error) => {
                            app.set_status(error.into());
                            refresh(&window, &app);
                            return;
                        }
                    };
                let previous_layout = (
                    app.get_osc_position(),
                    app.get_compact_ui(),
                    app.get_docked_controls(),
                );
                let previous_chapter_positions = app.get_show_chapter_pos();
                let previous_window_resize =
                    match crate::window_resize_preferences::from_player(&app) {
                        Ok(settings) => settings,
                        Err(error) => {
                            app.set_status(error.into());
                            refresh(&window, &app);
                            return;
                        }
                    };
                let previous_volume_boost = app.get_volume_boost();
                let previous_liquid_glass = crate::liquid_glass_preferences::current_player(&app);
                let selected_liquid_glass =
                    crate::liquid_glass_preferences::current_preferences(&window);
                let Some(selected_theme) =
                    riina_core::ThemeMaterial::from_index(window.get_appearance_index())
                else {
                    app.set_status("Choose a valid appearance".into());
                    refresh(&window, &app);
                    return;
                };
                let selected_window_resize =
                    match crate::window_resize_preferences::from_preferences(&window) {
                        Ok(settings) => settings,
                        Err(error) => {
                            app.set_status(error.into());
                            refresh(&window, &app);
                            return;
                        }
                    };
                let selected_dark = selected_theme.dark_for(app.get_dark_theme());
                let previous_volume = app.get_volume();
                let previous_decoder = app.get_hardware_decoder_index();
                let previous_font = app.get_subtitle_font().to_string();
                let previous_font_size = crate::subtitle_preferences::current_font_size(&app);
                let previous_text_style = match crate::subtitle_preferences::current_style(&app) {
                    Ok(style) => style,
                    Err(error) => {
                        app.set_status(error.into());
                        refresh(&window, &app);
                        return;
                    }
                };
                let previous_ass = (
                    app.get_subtitle_ass_override_index(),
                    app.get_secondary_subtitle_ass_override_index(),
                );
                let previous_position =
                    match crate::subtitle_position_preferences::current_player(&app) {
                        Ok(preferences) => preferences,
                        Err(error) => {
                            app.set_status(error.into());
                            refresh(&window, &app);
                            return;
                        }
                    };
                let selected_position =
                    match crate::subtitle_position_preferences::current_window(&window) {
                        Ok(preferences) => preferences,
                        Err(error) => {
                            app.set_status(error.into());
                            refresh(&window, &app);
                            return;
                        }
                    };
                let previous_subtitle_other =
                    match crate::subtitle_other_preferences::current_player(&app) {
                        Ok(preferences) => preferences,
                        Err(error) => {
                            app.set_status(error.into());
                            refresh(&window, &app);
                            return;
                        }
                    };
                let selected_subtitle_other =
                    match crate::subtitle_other_preferences::current_window(&window) {
                        Ok(preferences) => preferences,
                        Err(error) => {
                            app.set_status(error.into());
                            refresh(&window, &app);
                            return;
                        }
                    };
                let previous_auto_load =
                    match crate::subtitle_auto_load_preferences::current_player(&app) {
                        Ok(preferences) => preferences,
                        Err(error) => {
                            app.set_status(error.into());
                            refresh(&window, &app);
                            return;
                        }
                    };
                let selected_auto_load =
                    match crate::subtitle_auto_load_preferences::current_window(&window) {
                        Ok(preferences) => preferences,
                        Err(error) => {
                            app.set_status(error.into());
                            refresh(&window, &app);
                            return;
                        }
                    };
                let previous_network_cache =
                    match crate::network_cache_preferences::current_player(&app) {
                        Ok(preferences) => preferences,
                        Err(error) => {
                            app.set_status(error.into());
                            refresh(&window, &app);
                            return;
                        }
                    };
                let selected_network_cache =
                    match crate::network_cache_preferences::current_window(&window) {
                        Ok(preferences) => preferences,
                        Err(error) => {
                            app.set_status(error.into());
                            refresh(&window, &app);
                            return;
                        }
                    };
                let selected_network =
                    match crate::network_connection_preferences::current_window(&window) {
                        Ok(preferences) => preferences,
                        Err(error) => {
                            app.set_status(error.into());
                            refresh(&window, &app);
                            return;
                        }
                    };
                let selected_audio = match crate::audio_preferences::current_window(&window) {
                    Ok(preferences) => preferences,
                    Err(error) => {
                        app.set_status(error.into());
                        refresh(&window, &app);
                        return;
                    }
                };
                let selected_tone_mapping =
                    match crate::tone_mapping_preferences::current_window(&window) {
                        Ok(settings) => settings,
                        Err(error) => {
                            let saved = saved_tone_mapping_for_change.borrow().clone();
                            crate::tone_mapping_preferences::install_window(&window, &saved);
                            app.set_status(error.into());
                            return;
                        }
                    };
                let previous_tone_mapping = saved_tone_mapping_for_change.borrow().clone();
                let tone_mapping_changed = selected_tone_mapping != previous_tone_mapping;
                let selected_video_threads =
                    match window.get_video_threads_text().trim().parse::<u32>() {
                        Ok(threads) if threads <= i32::MAX as u32 => threads,
                        _ => {
                            window.set_video_threads_text(
                                saved_video_threads_for_change.borrow().to_string().into(),
                            );
                            app.set_status(
                                "Video decoder threads must be between 0 and 2,147,483,647".into(),
                            );
                            return;
                        }
                    };
                let previous_video_threads = *saved_video_threads_for_change.borrow();
                let video_threads_changed = selected_video_threads != previous_video_threads;
                let selected_controls = match crate::control_preferences::current_window(&window) {
                    Ok(preferences) => preferences,
                    Err(error) => {
                        app.set_status(error.into());
                        refresh(&window, &app);
                        return;
                    }
                };
                let selected_key_bindings =
                    match crate::key_binding_preferences::current_window(&window) {
                        Ok(preferences) => preferences,
                        Err(error) => {
                            app.set_status(error.into());
                            refresh(&window, &app);
                            return;
                        }
                    };
                let selected_advanced = match crate::advanced_preferences::current_window(&window) {
                    Ok(preferences) => preferences,
                    Err(error) => {
                        app.set_status(error.into());
                        refresh(&window, &app);
                        return;
                    }
                };
                let advanced_changed = selected_advanced != baseline.advanced;
                let audio_changed = selected_audio != baseline.audio;
                let previous_network_ytdl =
                    match crate::network_ytdl_preferences::current_player(&app) {
                        Ok(preferences) => preferences,
                        Err(error) => {
                            app.set_status(error.into());
                            refresh(&window, &app);
                            return;
                        }
                    };
                let selected_network_ytdl =
                    match crate::network_ytdl_preferences::current_window(&window) {
                        Ok(preferences) => preferences,
                        Err(error) => {
                            app.set_status(error.into());
                            refresh(&window, &app);
                            return;
                        }
                    };
                let decoder_index = window.get_hardware_decoder_index();
                let Some(decoder) = crate::codec_preferences::hardware_decoder(decoder_index)
                else {
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
                            && let Err(error) =
                                decoder_player.send(Command::Volume(capped_volume as f64))
                        {
                            let _ =
                                decoder_player.send(Command::SetVolumeBoost(previous_volume_boost));
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
                            let _ =
                                decoder_player.send(Command::SetVolumeBoost(previous_volume_boost));
                            if previous_volume != app.get_volume() {
                                let _ =
                                    decoder_player.send(Command::Volume(previous_volume as f64));
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
                let desired_text_style =
                    match crate::subtitle_preferences::current_window_style(&window) {
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
                app.set_launch_action_index(window.get_launch_action_index().clamp(0, 2));
                app.set_auto_switch_to_music_mode(window.get_auto_switch_to_music_mode());
                app.set_music_mode_show_playlist(window.get_music_mode_show_playlist());
                app.set_music_mode_show_album_art(window.get_music_mode_show_album_art());
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
                .and_then(|()| {
                    decoder_player.send(Command::SubtitleAutoLoad(selected_auto_load.clone()))
                })
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
                    if tone_mapping_changed {
                        decoder_player
                            .send(Command::SetToneMapping(selected_tone_mapping.clone()))?;
                        tone_mapping_applied = true;
                    }
                    if video_threads_changed {
                        decoder_player.send(Command::VideoThreads(selected_video_threads))?;
                        video_threads_applied = true;
                    }
                    crate::subtitle_position_preferences::install_player(&app, selected_position);
                    crate::subtitle_other_preferences::install_player(
                        &app,
                        &selected_subtitle_other,
                    );
                    crate::subtitle_auto_load_preferences::install_player(
                        &app,
                        &selected_auto_load,
                    );
                    self::current(&app)
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
                        picture_in_picture: selected_picture_in_picture,
                        action_after_launch: riina_core::ActionAfterLaunch::from_index(
                            window.get_launch_action_index(),
                        )
                        .ok_or_else(|| "Choose a valid launch action".to_owned())?,
                        live_text_enabled: window.get_live_text_enabled(),
                        theme_material: Some(selected_theme),
                        always_float_on_top: window.get_always_float_on_top(),
                        always_show_on_top_icon: window.get_always_show_on_top_icon(),
                        dark: selected_dark,
                        pause_when_open: app.get_pause_when_open(),
                        fullscreen_when_open: app.get_fullscreen_when_open(),
                        resume_last_position: app.get_resume_last_position(),
                        enable_osd: selected_osd.enabled,
                        disable_osd_file_start: selected_osd.disable_file_start,
                        disable_osd_pause_resume: selected_osd.disable_pause_resume,
                        disable_osd_speed: selected_osd.disable_speed,
                        disable_osd_seek: selected_osd.disable_seek,
                        osd_timeout_secs: selected_osd.timeout_secs,
                        osd_text_size: selected_osd.text_size,
                        display_time_and_battery_in_fullscreen: selected_osd
                            .display_time_and_battery_in_fullscreen,
                        leading_sidebar_width: app.get_leading_sidebar_width() as u32,
                        trailing_sidebar_width: app.get_trailing_sidebar_width() as u32,
                        settings_at_leading: app.get_settings_at_leading(),
                        playlist_at_leading: app.get_playlist_at_leading(),
                        edge_to_edge_video: app.get_edge_to_edge_video(),
                        docked_controls: app.get_docked_controls(),
                        unlock_window_aspect_ratio: app.get_unlock_window_aspect_ratio(),
                        window_resize: selected_window_resize,
                        compact_ui: app.get_compact_ui(),
                        osc_position: app.get_osc_position() as u8,
                        hardware_decoder: decoder,
                        subtitle_ass_override: ass_primary,
                        secondary_subtitle_ass_override: ass_secondary,
                        subtitle_position: selected_position,
                        subtitle_other: selected_subtitle_other.clone(),
                        subtitle_auto_load: selected_auto_load.clone(),
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
                    let text = serde_json::to_string_pretty(&preferences)
                        .map_err(|error| error.to_string())?;
                    crate::platform::save_preferences(&path, &text)
                        .map_err(|error| error.to_string())
                });
                match result {
                    Ok(()) => {
                        app.set_show_chapter_pos(window.get_show_chapter_pos());
                        crate::pip_preferences::install_player(&app, selected_picture_in_picture);
                        app.set_live_text_enabled(window.get_live_text_enabled());
                        *saved_tone_mapping_for_change.borrow_mut() = selected_tone_mapping.clone();
                        *saved_video_threads_for_change.borrow_mut() = selected_video_threads;
                        app.set_appearance_mode(selected_theme.index());
                        app.set_dark_theme(selected_dark);
                        selected_osd.install_player(&app);
                        app.set_always_float_on_top(window.get_always_float_on_top());
                        app.set_always_show_on_top_icon(window.get_always_show_on_top_icon());
                        crate::control_preferences::install_player(&app, &selected_controls);
                        crate::liquid_glass_preferences::install_player(
                            &app,
                            selected_liquid_glass,
                        );
                        crate::key_binding_preferences::install_player(
                            &app,
                            &selected_key_bindings,
                        );
                        if advanced_changed {
                            app.set_status("Advanced settings apply after restarting Riina".into());
                        } else if audio_changed {
                            app.set_status(
                                "Audio engine preferences apply after restarting Riina".into(),
                            );
                        }
                        if app.get_pause_when_open() != previous_pause {
                            app.invoke_pause_when_open_changed(app.get_pause_when_open());
                        }
                        if app.get_resume_last_position() != previous_resume {
                            app.invoke_resume_last_position_changed(app.get_resume_last_position());
                        }
                    }
                    Err(error) => {
                        if video_threads_applied {
                            let _ =
                                decoder_player.send(Command::VideoThreads(previous_video_threads));
                            window
                                .set_video_threads_text(previous_video_threads.to_string().into());
                        }
                        if tone_mapping_applied {
                            let _ = decoder_player
                                .send(Command::SetToneMapping(previous_tone_mapping.clone()));
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
                        let _ = decoder_player
                            .send(Command::SubtitleAutoLoad(previous_auto_load.clone()));
                        let _ = crate::subtitle_other_preferences::apply(
                            &decoder_player,
                            &selected_subtitle_other,
                            &previous_subtitle_other,
                        );
                        crate::subtitle_other_preferences::install_player(
                            &app,
                            &previous_subtitle_other,
                        );
                        crate::subtitle_auto_load_preferences::install_player(
                            &app,
                            &previous_auto_load,
                        );
                        let _ = crate::subtitle_position_preferences::apply(
                            &decoder_player,
                            selected_position,
                            previous_position,
                        );
                        crate::subtitle_position_preferences::install_player(
                            &app,
                            previous_position,
                        );
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
                        app.set_fullscreen_when_open(previous_fullscreen);
                        app.set_resume_last_position(previous_resume);
                        app.set_launch_action_index(previous_launch_action);
                        previous_osd.install_player(&app);
                        app.set_osc_position(previous_layout.0);
                        app.set_show_chapter_pos(previous_chapter_positions);
                        app.set_compact_ui(previous_layout.1);
                        app.set_docked_controls(previous_layout.2);
                        crate::liquid_glass_preferences::install_player(
                            &app,
                            previous_liquid_glass,
                        );
                        crate::window_resize_preferences::install_player(
                            &app,
                            previous_window_resize,
                        );
                        crate::tone_mapping_preferences::install_window(
                            &window,
                            &previous_tone_mapping,
                        );
                        install(&app, &previous);
                        refresh(&window, &app);
                        app.set_status(error.into());
                    }
                }
            });
            let parent = app.as_weak();
            let weak = window.as_weak();
            window.on_choose_folder(move || {
                let (Some(app), Some(window)) = (parent.upgrade(), weak.upgrade()) else {
                    return;
                };
                if app.get_dialog_open() || app.get_crop_editing() {
                    return;
                }
                app.set_dialog_open(true);
                window.set_folder_dialog_open(true);
                let parent = app.as_weak();
                let weak = window.as_weak();
                let failure_parent = parent.clone();
                let failure_window = weak.clone();
                let started = slint::spawn_local(async move {
                    let (Some(app), Some(window)) = (parent.upgrade(), weak.upgrade()) else {
                        return;
                    };
                    let result = async {
                        let owner = window
                            .window()
                            .winit_window()
                            .await
                            .map_err(|error| error.to_string())?;
                        let parent = parent.clone();
                        let weak = weak.clone();
                        crate::platform::folder_dialog::pick(owner, move |result| {
                            let (Some(app), Some(window)) = (parent.upgrade(), weak.upgrade())
                            else {
                                return;
                            };
                            app.set_dialog_open(false);
                            window.set_folder_dialog_open(false);
                            match result {
                                Ok(Some(directory)) => {
                                    window.set_directory(directory.into());
                                    window.set_custom_directory(true);
                                    window.invoke_settings_changed();
                                }
                                Ok(None) => {}
                                Err(error) => app.set_status(error.into()),
                            }
                        })
                    }
                    .await;
                    if let Err(error) = result {
                        app.set_dialog_open(false);
                        window.set_folder_dialog_open(false);
                        app.set_status(error.into());
                    }
                });
                if let Err(error) = started {
                    if let Some(app) = failure_parent.upgrade() {
                        app.set_dialog_open(false);
                        app.set_status(error.to_string().into());
                    }
                    if let Some(window) = failure_window.upgrade() {
                        window.set_folder_dialog_open(false);
                    }
                }
            });
            let weak = window.as_weak();
            window.window().on_close_requested(move || {
                if let Some(window) = weak.upgrade() {
                    if window.get_folder_dialog_open() {
                        return slint::CloseRequestResponse::KeepWindowShown;
                    }
                    let _ = window.hide();
                }
                slint::CloseRequestResponse::KeepWindowShown
            });
            *current.borrow_mut() = Some(window.clone_strong());
            window
        };
        let stored_audio = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| riina_core::Preferences::parse(&text).ok())
            .map(|preferences| preferences.audio)
            .unwrap_or_else(|| baseline.audio.clone());
        let available_devices = latest_playback
            .lock()
            .map(|playback| playback.audio_devices.clone())
            .unwrap_or_default();
        *observed_audio_devices.borrow_mut() = available_devices.clone();
        crate::audio_preferences::install_window(&window, &stored_audio, &available_devices);
        refresh(&window, &app);
        if let Err(error) = window.show() {
            app.set_status(error.to_string().into());
            return;
        }
        let weak = window.as_weak();
        let latest_playback = latest_playback.clone();
        let observed_devices = observed_audio_devices.clone();
        let saved_audio = stored_audio.clone();
        audio_device_refresh.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(750),
            move || {
                let Some(window) = weak.upgrade() else { return };
                if !window.window().is_visible() {
                    return;
                }
                let available = latest_playback
                    .lock()
                    .map(|playback| playback.audio_devices.clone())
                    .unwrap_or_default();
                let mut previous = observed_devices.borrow_mut();
                if *previous == available {
                    return;
                }
                let selected = crate::audio_preferences::current_window(&window)
                    .unwrap_or_else(|_| saved_audio.clone());
                let device_disconnected = selected.device != "auto"
                    && previous.iter().any(|device| device.name == selected.device)
                    && !available
                        .iter()
                        .any(|device| device.name == selected.device);
                *previous = available.clone();
                let selected = if device_disconnected {
                    let mut selected = selected;
                    selected.device = "auto".into();
                    selected.device_description = "Autoselect device".into();
                    selected
                } else {
                    selected
                };
                crate::audio_preferences::install_devices(&window, &selected, &available);
                if device_disconnected {
                    window.invoke_settings_changed();
                }
            },
        );
        let weak = window.as_weak();
        let _ = slint::spawn_local(async move {
            if let Some(window) = weak.upgrade()
                && let Ok(native) = window.window().winit_window().await
            {
                native.focus_window();
            }
        });
    });
}

fn refresh(window: &crate::PreferencesWindow, app: &PlayerWindow) {
    crate::osd_preferences::current_player(app).install_preferences(window);
    crate::liquid_glass_preferences::install_preferences(
        window,
        crate::liquid_glass_preferences::current_player(app),
    );
    crate::network_cache_preferences::refresh_window(window, app);
    crate::network_ytdl_preferences::refresh_window(window, app);
    window.set_live_text_enabled(app.get_live_text_enabled());
    window.set_live_text_available(app.get_live_text_available());
    if let Ok(controls) = crate::control_preferences::current_player(app) {
        crate::control_preferences::install_window(window, &controls);
    }
    if let Ok(settings) = crate::key_binding_preferences::current_player(app) {
        let _ = crate::key_binding_preferences::refresh(window, &settings);
    }
    window.set_subtitle_encoding_options(crate::subtitle_other_preferences::encoding_options());
    window
        .set_subtitle_auto_load_mode_options(crate::subtitle_auto_load_preferences::mode_options());
    window.set_subtitle_font(app.get_subtitle_font());
    window.set_subtitle_font_size_text(crate::subtitle_preferences::format_number(
        app.get_subtitle_font_size_preference(),
    ));
    if let Ok(style) = crate::subtitle_preferences::current_style(app) {
        crate::subtitle_preferences::install_window_style(window, style);
    }
    window.set_pause_when_open(app.get_pause_when_open());
    window.set_fullscreen_when_open(app.get_fullscreen_when_open());
    window.set_resume_last_position(app.get_resume_last_position());
    window.set_launch_action_index(app.get_launch_action_index());
    window.set_auto_switch_to_music_mode(app.get_auto_switch_to_music_mode());
    window.set_music_mode_show_playlist(app.get_music_mode_show_playlist());
    window.set_music_mode_show_album_art(app.get_music_mode_show_album_art());
    window.set_osc_position(app.get_osc_position());
    window.set_show_chapter_pos(app.get_show_chapter_pos());
    if let Ok(settings) = crate::pip_preferences::current_player(app) {
        crate::pip_preferences::install_preferences(window, settings);
    }
    window.set_hardware_decoder_index(app.get_hardware_decoder_index());
    window.set_subtitle_ass_override_primary_index(app.get_subtitle_ass_override_index());
    window
        .set_subtitle_ass_override_secondary_index(app.get_secondary_subtitle_ass_override_index());
    if let Ok(position) = crate::subtitle_position_preferences::current_player(app) {
        crate::subtitle_position_preferences::install_window(window, position);
    }
    if let Ok(other) = crate::subtitle_other_preferences::current_player(app) {
        let _ = crate::subtitle_other_preferences::install_window(window, &other);
    }
    if let Ok(auto_load) = crate::subtitle_auto_load_preferences::current_player(app) {
        crate::subtitle_auto_load_preferences::install_window(window, &auto_load);
    }
    window.set_volume_boost(app.get_volume_boost());
    window.set_appearance_index(app.get_appearance_mode());
    window.set_always_float_on_top(app.get_always_float_on_top());
    window.set_always_show_on_top_icon(app.get_always_show_on_top_icon());
    if let Ok(settings) = crate::window_resize_preferences::from_player(app) {
        crate::window_resize_preferences::install_preferences(window, settings);
    }
    window.set_compact_ui(app.get_compact_ui());
    window.set_docked_controls(app.get_docked_controls());
    window.set_edge_to_edge_video(app.get_edge_to_edge_video());
    let index = ["png", "jpg", "jpeg", "webp", "jxl"]
        .iter()
        .position(|format| *format == app.get_screenshot_format().as_str())
        .map_or(-1, |index| index as i32);
    window.set_format_index(index);
    window.set_include_subtitles(app.get_screenshot_include_subtitles());
    window.set_show_preview(app.get_screenshot_show_preview());
    window.set_save_to_file(app.get_screenshot_save_to_file());
    window.set_copy_to_clipboard(app.get_screenshot_copy_to_clipboard());
    window.set_custom_directory(app.get_screenshot_custom_directory());
    window.set_directory(if app.get_screenshot_custom_directory() {
        app.get_screenshot_directory()
    } else {
        dirs::picture_dir()
            .map(|path| path.join("Screenshots").to_string_lossy().into_owned())
            .unwrap_or_default()
            .into()
    });
}

fn apply_text_style(
    app: &PlayerWindow,
    player: &riina_mpv::CommandSender,
    previous: riina_core::subtitle_controls::TextStylePreferences,
    selected: riina_core::subtitle_controls::TextStylePreferences,
) -> Result<(), String> {
    selected.validate().map_err(str::to_owned)?;
    let previous_commands = crate::subtitle_preferences::commands(previous);
    let selected_commands = crate::subtitle_preferences::commands(selected);
    let mut applied = Vec::<usize>::new();
    for (index, (old, new)) in previous_commands
        .into_iter()
        .zip(selected_commands)
        .enumerate()
    {
        if old == new {
            continue;
        }
        if let Err(error) = player.send(new) {
            let rollback = crate::subtitle_preferences::commands(previous);
            for applied_index in applied.into_iter().rev() {
                let _ = player.send(rollback[applied_index].clone());
            }
            return Err(error.to_string());
        }
        applied.push(index);
    }
    crate::subtitle_preferences::install_style(app, selected);
    Ok(())
}

fn restore_text_style(
    app: &PlayerWindow,
    player: &riina_mpv::CommandSender,
    previous: riina_core::subtitle_controls::TextStylePreferences,
    selected: riina_core::subtitle_controls::TextStylePreferences,
) {
    if previous != selected {
        for command in crate::subtitle_preferences::commands(previous) {
            let _ = player.send(command);
        }
    }
    crate::subtitle_preferences::install_style(app, previous);
}

fn apply_subtitle_text_preferences(
    app: &PlayerWindow,
    player: &riina_mpv::CommandSender,
    previous_font: &str,
    previous_size: f64,
    selected_font: &str,
    selected_size: f64,
) -> Result<(String, f64), String> {
    let font_command =
        riina_core::subtitle_controls::font_from_text(selected_font).map_err(str::to_owned)?;
    let font = match font_command {
        Command::SubtitleFont(value) => value,
        _ => return Err("Invalid subtitle font".into()),
    };
    if (selected_size - previous_size).abs() > f64::EPSILON {
        player
            .send(Command::SubtitleFontSize(selected_size))
            .map_err(|error| error.to_string())?;
    }
    if font != previous_font
        && let Err(error) = player.send(Command::SubtitleFont(font.clone()))
    {
        if (selected_size - previous_size).abs() > f64::EPSILON {
            let _ = player.send(Command::SubtitleFontSize(previous_size));
        }
        return Err(error.to_string());
    }
    app.set_subtitle_font(font.clone().into());
    app.set_subtitle_font_size_preference(selected_size as f32);
    Ok((font, selected_size))
}

fn restore_subtitle_text_preferences(
    app: &PlayerWindow,
    player: &riina_mpv::CommandSender,
    previous_font: &str,
    previous_size: f64,
    selected_font: &str,
    selected_size: f64,
) {
    if selected_size != previous_size {
        let _ = player.send(Command::SubtitleFontSize(previous_size));
    }
    if selected_font != previous_font {
        let _ = player.send(Command::SubtitleFont(previous_font.to_owned()));
    }
    app.set_subtitle_font(previous_font.into());
    app.set_subtitle_font_size_preference(previous_size as f32);
}

fn apply_ass_override(
    app: &PlayerWindow,
    player: &riina_mpv::CommandSender,
    previous: (i32, i32),
    selected: (i32, i32),
) -> Result<
    (
        riina_core::subtitle_controls::AssOverrideLevel,
        riina_core::subtitle_controls::AssOverrideLevel,
    ),
    String,
> {
    use riina_core::subtitle_controls::{AssOverrideLevel, SubtitleLane};

    let primary = AssOverrideLevel::from_slider_index(selected.0)
        .ok_or_else(|| "Invalid primary ASS override selection".to_owned())?;
    let secondary = AssOverrideLevel::from_slider_index(selected.1)
        .ok_or_else(|| "Invalid secondary ASS override selection".to_owned())?;
    if selected.0 != previous.0 {
        player
            .send(Command::SubtitleAssOverride {
                lane: SubtitleLane::Primary,
                level: primary,
            })
            .map_err(|error| error.to_string())?;
    }
    if selected.1 != previous.1
        && let Err(error) = player.send(Command::SubtitleAssOverride {
            lane: SubtitleLane::Secondary,
            level: secondary,
        })
    {
        if selected.0 != previous.0
            && let Some(previous_level) = AssOverrideLevel::from_slider_index(previous.0)
        {
            let _ = player.send(Command::SubtitleAssOverride {
                lane: SubtitleLane::Primary,
                level: previous_level,
            });
        }
        return Err(error.to_string());
    }
    app.set_subtitle_ass_override_index(selected.0);
    app.set_secondary_subtitle_ass_override_index(selected.1);
    Ok((primary, secondary))
}

fn restore_ass_override(
    app: &PlayerWindow,
    player: &riina_mpv::CommandSender,
    previous: (i32, i32),
    selected: (i32, i32),
) {
    use riina_core::subtitle_controls::{AssOverrideLevel, SubtitleLane};

    if selected.0 != previous.0
        && let Some(level) = AssOverrideLevel::from_slider_index(previous.0)
    {
        let _ = player.send(Command::SubtitleAssOverride {
            lane: SubtitleLane::Primary,
            level,
        });
    }
    if selected.1 != previous.1
        && let Some(level) = AssOverrideLevel::from_slider_index(previous.1)
    {
        let _ = player.send(Command::SubtitleAssOverride {
            lane: SubtitleLane::Secondary,
            level,
        });
    }
    app.set_subtitle_ass_override_index(previous.0);
    app.set_secondary_subtitle_ass_override_index(previous.1);
}

fn restore_live_volume_and_decoder(
    app: &PlayerWindow,
    player: &riina_mpv::CommandSender,
    volume_boost_applied: bool,
    previous_volume_boost: bool,
    previous_volume: f32,
    decoder_applied: bool,
    previous_decoder: i32,
) {
    if volume_boost_applied {
        let _ = player.send(Command::SetVolumeBoost(previous_volume_boost));
        if previous_volume != app.get_volume() {
            let _ = player.send(Command::Volume(previous_volume as f64));
        }
        app.set_volume_boost(previous_volume_boost);
        app.set_volume(previous_volume);
    }
    if decoder_applied {
        if let Some(previous_mode) = crate::codec_preferences::hardware_decoder(previous_decoder) {
            let _ = player.send(Command::HardwareDecoding(previous_mode));
        }
        app.set_hardware_decoder_index(previous_decoder);
    }
}

pub(crate) fn install(app: &PlayerWindow, settings: &Settings) {
    app.set_screenshot_save_to_file(settings.destinations.save_to_file);
    app.set_screenshot_copy_to_clipboard(settings.destinations.copy_to_clipboard);
    app.set_screenshot_format(settings.encoding.format.native_value().into());
    app.set_screenshot_template(settings.encoding.template.clone().into());
    app.set_screenshot_custom_directory(settings.directory.is_some());
    app.set_screenshot_directory(settings.directory.clone().unwrap_or_default().into());
    app.set_screenshot_include_subtitles(settings.include_subtitles);
    app.set_screenshot_show_preview(settings.show_preview);
}

pub(crate) fn current(app: &PlayerWindow) -> Result<Settings, String> {
    let format = match app.get_screenshot_format().as_str() {
        "png" => Format::Png,
        "jpg" => Format::Jpg,
        "jpeg" => Format::Jpeg,
        "webp" => Format::Webp,
        "jxl" => Format::Jxl,
        "avif" => Format::Avif,
        _ => return Err("Unknown screenshot format".into()),
    };
    let settings = Settings {
        destinations: riina_core::screenshot::Destinations {
            save_to_file: app.get_screenshot_save_to_file(),
            copy_to_clipboard: app.get_screenshot_copy_to_clipboard(),
        },
        encoding: riina_core::screenshot::Encoding {
            format,
            template: app.get_screenshot_template().to_string(),
        },
        directory: app
            .get_screenshot_custom_directory()
            .then(|| app.get_screenshot_directory().to_string()),
        include_subtitles: app.get_screenshot_include_subtitles(),
        show_preview: app.get_screenshot_show_preview(),
    };
    // An invalid draft must not replace saved preferences or reach libmpv.
    settings.validate()?;
    Ok(settings)
}
