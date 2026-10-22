//! Own the preferences window and its platform dialogs and refresh timers.
mod changes;
mod key_bindings;
mod live;
mod refresh;

use crate::PlayerWindow;
use refresh::refresh;
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{cell::RefCell, path::PathBuf, rc::Rc, time::Duration};

pub(crate) fn connect(
    app: &PlayerWindow,
    decoder_player: riina_mpv::CommandSender,
    path: PathBuf,
    baseline: riina_core::Preferences,
    latest_playback: std::sync::Arc<std::sync::Mutex<riina_core::Playback>>,
    plugin_host: Option<crate::plugin_host::PluginHostHandle>,
    clear_history: impl Fn() -> Result<usize, String> + Clone + 'static,
) {
    let current = Rc::new(RefCell::new(None::<crate::PreferencesWindow>));
    let reduce_motion_refresh = Rc::new(RefCell::new(None::<Rc<slint::Timer>>));
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
            crate::appearance_preferences::install_system_typography(&window);
            crate::platform::enable_preferences_mica(&window);
            crate::settings_search::connect(&window);
            let assrt_token_page_parent = app.as_weak();
            window.on_open_assrt_token_page(move || {
                if let Err(error) =
                    crate::platform::open_http_url_in_browser("https://2.assrt.net/api/doc")
                    && let Some(app) = assrt_token_page_parent.upgrade()
                {
                    app.set_status(
                        format!("Could not open Assrt API instructions: {error}").into(),
                    );
                }
            });
            crate::language_token_preferences::connect(&window);
            crate::osc_preferences::connect_preferences(&window);
            crate::utilities_preferences::connect(&window, &path, clear_history.clone());
            crate::plugin_preferences::connect(
                &window,
                &app,
                &path,
                plugin_host.clone(),
                decoder_player.clone(),
            );
            crate::advanced_preferences::install_window(&window, &baseline.advanced);
            crate::advanced_preferences::connect_actions(&app, &window, path.clone());
            let available_devices = latest_playback
                .lock()
                .map(|playback| playback.audio_devices.clone())
                .unwrap_or_default();
            crate::audio_preferences::install_window(&window, &baseline.audio, &available_devices);
            crate::tone_mapping_preferences::install_window(&window, &baseline.hdr_tone_mapping);
            window.set_load_icc_profile(baseline.load_icc_profile);
            window.set_default_hdr_mode_enabled(baseline.hdr_enabled);
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
            crate::thumbnail_preferences::install_window(&window, &baseline.thumbnail_preview);
            crate::online_subtitles_preferences::install_window(
                &window,
                &baseline.online_subtitles,
            );
            crate::window_resize_preferences::install_preferences(&window, baseline.window_resize);
            window.set_allow_multiple_windows(baseline.allow_multiple_windows);
            window.set_group_simultaneous_opens_in_playlist(
                baseline.group_simultaneous_opens_in_playlist,
            );
            window.set_allow_duplicate_players(baseline.allow_duplicate_players);
            window.set_playlist_auto_add(baseline.playlist_auto_add);
            #[cfg(target_os = "macos")]
            {
                let active_color_panel = Rc::new(RefCell::new(None));
                let active_panel_slot = active_color_panel.clone();
                let color_panel_window = window.as_weak();
                let color_panel_parent = app.as_weak();
                window.on_native_subtitle_color_requested(move |target, color| {
                    let Some(window) = color_panel_window.upgrade() else {
                        return;
                    };
                    let initial = [color.red(), color.green(), color.blue(), color.alpha()];
                    let weak_window = window.as_weak();
                    let panel =
                        crate::platform::MacSubtitleColorPanel::open(initial, move |channels| {
                            let Some(window) = weak_window.upgrade() else {
                                return;
                            };
                            let color = slint::Color::from_argb_u8(
                                channels[3],
                                channels[0],
                                channels[1],
                                channels[2],
                            );
                            match target {
                                1 => window.set_subtitle_text_color(color),
                                2 => {
                                    window.set_subtitle_background_color(color);
                                    window.set_subtitle_shadow_color(color);
                                }
                                3 => window.set_subtitle_outline_color(color),
                                4 => {
                                    window.set_subtitle_shadow_color(color);
                                    window.set_subtitle_background_color(color);
                                }
                                _ => return,
                            }
                            window.invoke_settings_changed();
                        });
                    match panel {
                        Ok(panel) => *active_panel_slot.borrow_mut() = Some(panel),
                        Err(error) => {
                            if let Some(app) = color_panel_parent.upgrade() {
                                app.set_status(error.into());
                            }
                        }
                    }
                });
            }
            key_bindings::connect(&app, &window);
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
            let advanced_settings_window = window.as_weak();
            window.on_advanced_settings_changed(move || {
                if let Some(window) = advanced_settings_window.upgrade() {
                    window.invoke_settings_changed();
                }
            });
            changes::connect(
                &app,
                &window,
                decoder_player.clone(),
                path.clone(),
                baseline.clone(),
                latest_playback.clone(),
            );
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
            let weak = window.as_weak();
            let timer = Rc::new(slint::Timer::default());
            timer.start(
                slint::TimerMode::Repeated,
                Duration::from_millis(500),
                move || {
                    if let Some(window) = weak.upgrade() {
                        let reduce_motion = crate::platform::system_reduce_motion_enabled();
                        if window.get_system_reduce_motion() != reduce_motion {
                            window.set_system_reduce_motion(reduce_motion);
                        }
                    }
                },
            );
            *reduce_motion_refresh.borrow_mut() = Some(timer);
            crate::platform::center_auxiliary_window(&window, &app);
            *current.borrow_mut() = Some(window.clone_strong());
            window
        };
        let target_pane = app.get_preferences_target_pane();
        if target_pane >= 0 {
            window.set_selected_pane(target_pane);
            app.set_preferences_target_pane(-1);
        }
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
