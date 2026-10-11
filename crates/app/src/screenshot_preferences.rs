//! The UI owns live preferences; capture and session saving read the same intent.
use crate::PlayerWindow;
use riina_core::{
    Command,
    screenshot::{Format, Settings},
};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{cell::RefCell, path::PathBuf, rc::Rc};

pub(crate) fn connect(
    app: &PlayerWindow,
    decoder_player: riina_mpv::CommandSender,
    path: PathBuf,
    baseline: riina_core::Preferences,
) {
    let current = Rc::new(RefCell::new(None::<crate::PreferencesWindow>));
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
            crate::network_connection_preferences::install_window(&window, &baseline.network);
            let parent = app.as_weak();
            let weak = window.as_weak();
            let path = path.clone();
            let baseline = baseline.clone();
            let decoder_player = decoder_player.clone();
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
                let previous_osd_enabled = app.get_osd_enabled();
                let previous_osd_suppression =
                    (app.get_disable_osd_speed(), app.get_disable_osd_seek());
                let previous_layout = (
                    app.get_osc_position(),
                    app.get_compact_ui(),
                    app.get_docked_controls(),
                );
                let previous_volume_boost = app.get_volume_boost();
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
                app.set_osd_enabled(window.get_osd_enabled());
                app.set_disable_osd_speed(window.get_disable_osd_speed());
                app.set_disable_osd_seek(window.get_disable_osd_seek());
                app.set_osc_position(window.get_osc_position().clamp(0, 2));
                app.set_compact_ui(window.get_compact_ui());
                if !app.get_edge_to_edge_video() {
                    app.set_docked_controls(window.get_docked_controls());
                }
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
                    let preferences = riina_core::Preferences {
                        screenshot,
                        volume: f64::from(app.get_volume()).min(if app.get_volume_boost() {
                            200.0
                        } else {
                            100.0
                        }),
                        volume_boost: app.get_volume_boost(),
                        pause_when_open: app.get_pause_when_open(),
                        fullscreen_when_open: app.get_fullscreen_when_open(),
                        resume_last_position: app.get_resume_last_position(),
                        enable_osd: app.get_osd_enabled(),
                        disable_osd_speed: app.get_disable_osd_speed(),
                        disable_osd_seek: app.get_disable_osd_seek(),
                        leading_sidebar_width: app.get_leading_sidebar_width() as u32,
                        trailing_sidebar_width: app.get_trailing_sidebar_width() as u32,
                        settings_at_leading: app.get_settings_at_leading(),
                        playlist_at_leading: app.get_playlist_at_leading(),
                        edge_to_edge_video: app.get_edge_to_edge_video(),
                        docked_controls: app.get_docked_controls(),
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
                        network_cache: selected_network_cache.clone(),
                        ..baseline.clone()
                    };
                    let text = serde_json::to_string_pretty(&preferences)
                        .map_err(|error| error.to_string())?;
                    crate::platform::save_preferences(&path, &text)
                        .map_err(|error| error.to_string())
                });
                match result {
                    Ok(()) => {
                        if app.get_pause_when_open() != previous_pause {
                            app.invoke_pause_when_open_changed(app.get_pause_when_open());
                        }
                        if app.get_resume_last_position() != previous_resume {
                            app.invoke_resume_last_position_changed(app.get_resume_last_position());
                        }
                    }
                    Err(error) => {
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
                        app.set_osd_enabled(previous_osd_enabled);
                        app.set_disable_osd_speed(previous_osd_suppression.0);
                        app.set_disable_osd_seek(previous_osd_suppression.1);
                        app.set_osc_position(previous_layout.0);
                        app.set_compact_ui(previous_layout.1);
                        app.set_docked_controls(previous_layout.2);
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
        refresh(&window, &app);
        if let Err(error) = window.show() {
            app.set_status(error.to_string().into());
            return;
        }
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
    crate::network_cache_preferences::refresh_window(window, app);
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
    window.set_osd_enabled(app.get_osd_enabled());
    window.set_disable_osd_speed(app.get_disable_osd_speed());
    window.set_disable_osd_seek(app.get_disable_osd_seek());
    window.set_osc_position(app.get_osc_position());
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
