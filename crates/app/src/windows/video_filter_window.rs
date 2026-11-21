mod preset_editor;

use crate::{PlayerWindow, SavedFilterMenuRow, VideoFilterWindowRow, VideoFiltersWindow};
use riina_core::video_filters;
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

type Settings = Arc<Mutex<video_filters::Settings>>;
type WindowSlot = Rc<RefCell<Option<VideoFiltersWindow>>>;

struct FilterWindow {
    _window: WindowSlot,
    _refresh: Rc<slint::Timer>,
}

pub(crate) struct Guard {
    _video: FilterWindow,
    _audio: FilterWindow,
}

pub(crate) fn install(
    app: &PlayerWindow,
    latest: crate::PlaybackStore,
    video_settings: Settings,
    audio_settings: Settings,
    player: riina_mpv::CommandSender,
    preferences_path: PathBuf,
) -> Guard {
    let shortcut_video_settings = video_settings.clone();
    let shortcut_audio_settings = audio_settings.clone();
    let shortcut_player = player.clone();
    app.on_activate_saved_filter_shortcut(move |key, control, alt, shift, meta| {
        let modifiers = (if control {
            video_filters::SHORTCUT_CONTROL
        } else {
            0
        }) | (if alt { video_filters::SHORTCUT_ALT } else { 0 })
            | (if shift {
                video_filters::SHORTCUT_SHIFT
            } else {
                0
            })
            | (if meta {
                video_filters::SHORTCUT_META
            } else {
                0
            });
        let matches = |settings: &Settings| {
            riina_core::sync::lock(settings)
                .saved
                .iter()
                .find(|saved| {
                    saved.shortcut_key == key.as_str() && saved.shortcut_modifiers == modifiers
                })
                .map(|saved| saved.filter.clone())
        };
        let command = matches(&shortcut_video_settings)
            .map(riina_core::Command::ToggleVideoFilter)
            .or_else(|| {
                matches(&shortcut_audio_settings).map(riina_core::Command::ToggleAudioFilter)
            });
        command.is_some_and(|command| shortcut_player.send(command).is_ok())
    });

    let video = install_for_kind(
        app,
        latest.clone(),
        video_settings,
        player.clone(),
        preferences_path.clone(),
        false,
    );
    let audio = install_for_kind(app, latest, audio_settings, player, preferences_path, true);
    Guard {
        _video: video,
        _audio: audio,
    }
}

fn install_for_kind(
    app: &PlayerWindow,
    latest: crate::PlaybackStore,
    settings: Settings,
    player: riina_mpv::CommandSender,
    preferences_path: PathBuf,
    is_audio: bool,
) -> FilterWindow {
    let slot = Rc::new(RefCell::new(None::<VideoFiltersWindow>));
    let timer = Rc::new(slint::Timer::default());
    let show_timer = timer.clone();
    let show_slot = slot.clone();
    let show_latest = latest.clone();
    let show_settings = settings.clone();
    let show_player = player.clone();
    let show_path = preferences_path.clone();
    let saved_menu_player = player.clone();
    let menu_weak_app = app.as_weak();
    let toggle_saved = move |filter: slint::SharedString| {
        let Some(app) = menu_weak_app.upgrade() else {
            return;
        };
        let command = if is_audio {
            riina_core::Command::ToggleAudioFilter(filter.to_string())
        } else {
            riina_core::Command::ToggleVideoFilter(filter.to_string())
        };
        if let Err(error) = saved_menu_player.send(command) {
            app.set_status(error.into());
        }
    };
    if is_audio {
        app.on_toggle_saved_audio_filter(toggle_saved);
    } else {
        app.on_toggle_saved_video_filter(toggle_saved);
    }

    let open_weak_app = app.as_weak();
    let open_filter_window = move || {
        let Some(app) = open_weak_app.upgrade() else {
            return;
        };
        let window = if let Some(window) = show_slot.borrow().as_ref() {
            window.clone_strong()
        } else {
            let window = match VideoFiltersWindow::new() {
                Ok(window) => window,
                Err(error) => {
                    let label = if is_audio { "Audio" } else { "Video" };
                    app.set_status(format!("Could not open {label} Filters: {error}").into());
                    return;
                }
            };
            crate::appearance_preferences::install_system_typography(&window);
            window.set_is_audio(is_audio);
            window.set_window_title(if is_audio {
                "Audio Filters".into()
            } else {
                "Video Filters".into()
            });
            window.set_presets(ModelRc::new(VecModel::from(
                std::iter::once("Choose Preset…")
                    .chain(
                        if is_audio {
                            &video_filters::AUDIO_PRESET_LABELS[..]
                        } else {
                            &video_filters::PRESET_LABELS[..]
                        }
                        .iter()
                        .copied(),
                    )
                    .map(Into::into)
                    .collect::<Vec<slint::SharedString>>(),
            )));
            connect_window(
                &app,
                &window,
                show_latest.clone(),
                show_settings.clone(),
                show_player.clone(),
                show_path.clone(),
                is_audio,
            );
            crate::platform::center_auxiliary_window(&window, &app);
            *show_slot.borrow_mut() = Some(window.clone_strong());
            window
        };
        refresh(&app, &window, &show_latest, &show_settings, is_audio);
        if let Err(error) = window.show() {
            app.set_status(format!("Could not show filter window: {error}").into());
        } else {
            crate::platform::enable_video_filters_acrylic(&window);
            let app = app.as_weak();
            let latest = show_latest.clone();
            let settings = show_settings.clone();
            super::visible_refresh::start(
                &window,
                &show_timer,
                Duration::from_millis(300),
                move |window| {
                    if let Some(app) = app.upgrade() {
                        refresh(&app, window, &latest, &settings, is_audio);
                    }
                },
            );
        }
    };
    if is_audio {
        app.on_open_audio_filters(open_filter_window);
    } else {
        app.on_open_video_filters(open_filter_window);
    }

    FilterWindow {
        _window: slot,
        _refresh: timer,
    }
}

fn connect_window(
    app: &PlayerWindow,
    window: &VideoFiltersWindow,
    latest: crate::PlaybackStore,
    settings: Settings,
    player: riina_mpv::CommandSender,
    preferences_path: PathBuf,
    is_audio: bool,
) {
    preset_editor::install(app, window, is_audio);
    bind_current_filters(app, window, &latest, &player, is_audio);
    bind_saved_filters(
        app,
        window,
        &latest,
        &settings,
        &player,
        &preferences_path,
        is_audio,
    );
}

fn bind_current_filters(
    app: &PlayerWindow,
    window: &VideoFiltersWindow,
    latest: &crate::PlaybackStore,
    player: &riina_mpv::CommandSender,
    is_audio: bool,
) {
    let weak_app = app.as_weak();
    let add_window = window.as_weak();
    let add_player = player.clone();
    let add_is_audio = is_audio;
    window.on_add_filter(move |filter| {
        let Some(app) = weak_app.upgrade() else {
            return;
        };
        let filter = filter.to_string();
        if !video_filters::valid_filter_string(&filter) {
            app.set_status("The filter string is empty or invalid".into());
            return;
        }
        let command = if add_is_audio {
            riina_core::Command::AddAudioFilter(filter)
        } else {
            riina_core::Command::AddVideoFilter(filter)
        };
        match add_player.send(command) {
            Ok(()) => {
                if let Some(window) = add_window.upgrade() {
                    window.set_filter_string("".into());
                }
            }
            Err(error) => app.set_status(error.into()),
        }
    });

    let weak_app = app.as_weak();
    let remove_latest = latest.clone();
    let remove_player = player.clone();
    let remove_is_audio = is_audio;
    window.on_remove_current_filter(move |token| {
        let Some(app) = weak_app.upgrade() else {
            return;
        };
        let index = token.parse::<usize>().ok();
        let filter = index.and_then(|index| {
            let playback = riina_core::sync::lock(&remove_latest);
            if remove_is_audio {
                playback.audio_filters.as_ref()?.get(index).cloned()
            } else {
                playback.video_filters.as_ref()?.get(index).cloned()
            }
        });
        let Some(filter) = filter else {
            app.set_status("The selected filter is no longer active".into());
            return;
        };
        let command = if remove_is_audio {
            riina_core::Command::RemoveAudioFilter(filter)
        } else {
            riina_core::Command::RemoveVideoFilter(filter)
        };
        if let Err(error) = remove_player.send(command) {
            app.set_status(error.into());
        }
    });

    let weak_app = app.as_weak();
    let edit_window = window.as_weak();
    let edit_player = player.clone();
    let edit_is_audio = is_audio;
    window.on_apply_current_filter(move |index, filter| {
        let Some(app) = weak_app.upgrade() else {
            return;
        };
        let filter = filter.to_string();
        if !video_filters::valid_filter_string(&filter) {
            app.set_status("The filter string is empty or invalid".into());
            return;
        }
        let Ok(index) = usize::try_from(index) else {
            app.set_status("The selected filter is no longer active".into());
            return;
        };
        let command = if edit_is_audio {
            riina_core::Command::SetAudioFilterAt { index, filter }
        } else {
            riina_core::Command::SetVideoFilterAt { index, filter }
        };
        match edit_player.send(command) {
            Ok(()) => {
                if let Some(window) = edit_window.upgrade() {
                    window.set_editing_current_index(-1);
                    window.set_editing_current_filter("".into());
                }
            }
            Err(error) => app.set_status(error.into()),
        }
    });
}

fn bind_saved_filters(
    app: &PlayerWindow,
    window: &VideoFiltersWindow,
    latest: &crate::PlaybackStore,
    settings: &Settings,
    player: &riina_mpv::CommandSender,
    preferences_path: &std::path::Path,
    is_audio: bool,
) {
    let weak_app = app.as_weak();
    let save_settings = settings.clone();
    let save_path = preferences_path.to_path_buf();
    window.on_save_current_filter(move |filter, name| {
        let Some(app) = weak_app.upgrade() else {
            return;
        };
        let result = save_settings
            .lock()
            .map_err(|_| "Saved filter settings are unavailable")
            .and_then(|mut settings| settings.save(name.as_str(), filter.as_str()));
        if let Err(error) = result {
            app.set_status(error.into());
        } else if let Err(error) = persist(&save_settings, &save_path, is_audio) {
            app.set_status(error.into());
        }
    });

    let weak_app = app.as_weak();
    let toggle_player = player.clone();
    let toggle_is_audio = is_audio;
    window.on_toggle_saved_filter(move |filter| {
        let Some(app) = weak_app.upgrade() else {
            return;
        };
        let command = if toggle_is_audio {
            riina_core::Command::ToggleAudioFilter(filter.to_string())
        } else {
            riina_core::Command::ToggleVideoFilter(filter.to_string())
        };
        if let Err(error) = toggle_player.send(command) {
            app.set_status(error.into());
        }
    });

    let weak_app = app.as_weak();
    let remove_settings = settings.clone();
    let remove_path = preferences_path.to_path_buf();
    window.on_remove_saved_filter(move |index| {
        let Some(app) = weak_app.upgrade() else {
            return;
        };
        let index = usize::try_from(index).ok();
        let result = index
            .ok_or("The selected saved filter no longer exists")
            .and_then(|index| {
                remove_settings
                    .lock()
                    .map_err(|_| "Saved filter settings are unavailable")?
                    .remove(index)
            });
        if let Err(error) = result {
            app.set_status(error.into());
        } else if let Err(error) = persist(&remove_settings, &remove_path, is_audio) {
            app.set_status(error.into());
        }
    });

    let weak_app = app.as_weak();
    let update_settings = settings.clone();
    let update_path = preferences_path.to_path_buf();
    let update_latest = latest.clone();
    window.on_update_saved_filter(
        move |index, name, filter, shortcut_key, shortcut_modifiers| {
            let Some(app) = weak_app.upgrade() else {
                return;
            };
            let index = usize::try_from(index).ok();
            let result = index
                .ok_or("The selected saved filter no longer exists")
                .and_then(|index| {
                    let modifiers = u8::try_from(shortcut_modifiers)
                        .map_err(|_| "The saved-filter shortcut is invalid")?;
                    update_settings
                        .lock()
                        .map_err(|_| "Saved filter settings are unavailable")?
                        .update_with_shortcut(
                            index,
                            name.as_str(),
                            filter.as_str(),
                            shortcut_key.as_str(),
                            modifiers,
                        )
                });
            if let Err(error) = result {
                app.set_status(error.into());
            } else if let Err(error) = persist(&update_settings, &update_path, is_audio) {
                app.set_status(error.into());
            } else {
                refresh_player_menu_rows(&app, &update_latest, &update_settings, is_audio);
            }
        },
    );
}

fn refresh(
    app: &PlayerWindow,
    window: &VideoFiltersWindow,
    latest: &crate::PlaybackStore,
    settings: &Settings,
    is_audio: bool,
) {
    window.set_dark_theme(app.get_dark_theme());
    let playback = latest
        .lock()
        .map(|playback| playback.clone())
        .unwrap_or_default();
    let filters = if is_audio {
        playback.audio_filters.as_deref().unwrap_or_default()
    } else {
        playback.video_filters.as_deref().unwrap_or_default()
    };
    window.set_current_filters(ModelRc::new(VecModel::from(
        filters
            .iter()
            .enumerate()
            .map(|(index, filter)| VideoFilterWindowRow {
                token: index.to_string().into(),
                name: filter.name.clone().into(),
                detail: video_filters::format_filter(filter).into(),
                filter: video_filters::format_filter(filter).into(),
            })
            .collect::<Vec<_>>(),
    )));
    let settings = riina_core::sync::lock(settings);
    let rows = saved_filter_rows(&settings.saved, filters);
    window.set_saved_filters(ModelRc::new(VecModel::from(rows.clone())));
    if is_audio {
        app.set_saved_audio_filter_rows(ModelRc::new(VecModel::from(rows)));
    } else {
        app.set_saved_video_filter_rows(ModelRc::new(VecModel::from(rows)));
    }
}

fn refresh_player_menu_rows(
    app: &PlayerWindow,
    latest: &crate::PlaybackStore,
    settings: &Settings,
    is_audio: bool,
) {
    let playback = riina_core::sync::lock(latest).clone();
    let settings = riina_core::sync::lock(settings);
    let active = if is_audio {
        playback.audio_filters.as_deref().unwrap_or_default()
    } else {
        playback.video_filters.as_deref().unwrap_or_default()
    };
    let rows = ModelRc::new(VecModel::from(saved_filter_rows(&settings.saved, active)));
    if is_audio {
        app.set_saved_audio_filter_rows(rows);
    } else {
        app.set_saved_video_filter_rows(rows);
    }
}

pub(crate) fn saved_filter_rows(
    saved_filters: &[video_filters::SavedFilter],
    active_filters: &[riina_core::crop::VideoFilter],
) -> Vec<SavedFilterMenuRow> {
    saved_filters
        .iter()
        .map(|saved| SavedFilterMenuRow {
            token: saved.filter.clone().into(),
            label: saved.name.clone().into(),
            selected: video_filters::is_active(&saved.filter, active_filters),
            shortcut: saved_filter_menu_shortcut(&saved.shortcut_key, saved.shortcut_modifiers),
            shortcut_label: format_shortcut(&saved.shortcut_key, saved.shortcut_modifiers).into(),
            shortcut_key: saved.shortcut_key.clone().into(),
            shortcut_modifiers: i32::from(saved.shortcut_modifiers),
        })
        .collect()
}

fn saved_filter_menu_shortcut(key: &str, modifiers: u8) -> slint::Keys {
    if key.is_empty() {
        return slint::Keys::default();
    }
    let mut parts = Vec::with_capacity(5);
    for (flag, name) in [
        (video_filters::SHORTCUT_META, "Meta"),
        (video_filters::SHORTCUT_CONTROL, "Control"),
        (video_filters::SHORTCUT_ALT, "Alt"),
        (video_filters::SHORTCUT_SHIFT, "Shift"),
    ] {
        if modifiers & flag != 0 {
            parts.push(name.to_owned());
        }
    }
    let mut key = match key.to_ascii_uppercase().as_str() {
        "SPACE" => "Space".to_owned(),
        "TAB" => "Tab".to_owned(),
        "LEFT" | "LEFTARROW" => "LeftArrow".to_owned(),
        "RIGHT" | "RIGHTARROW" => "RightArrow".to_owned(),
        "UP" | "UPARROW" => "UpArrow".to_owned(),
        "DOWN" | "DOWNARROW" => "DownArrow".to_owned(),
        "ENTER" | "RETURN" => "Return".to_owned(),
        "ESC" | "ESCAPE" => "Escape".to_owned(),
        "BS" | "BACKSPACE" => "Backspace".to_owned(),
        "DEL" | "DELETE" => "Delete".to_owned(),
        "-" => "HyphenMinus".to_owned(),
        "=" => "Equals".to_owned(),
        "[" => "OpenBracket".to_owned(),
        "]" => "CloseBracket".to_owned(),
        "\\" => "BackSlash".to_owned(),
        "`" => "BackQuote".to_owned(),
        "," => "Comma".to_owned(),
        "." => "Period".to_owned(),
        "/" => "Slash".to_owned(),
        "<" => "LessThan".to_owned(),
        ">" => "GreaterThan".to_owned(),
        "(" => "OpenParen".to_owned(),
        ")" => "CloseParen".to_owned(),
        "+" => "Plus".to_owned(),
        "_" => "Underscore".to_owned(),
        _ if key.chars().count() == 1 && key.chars().next().is_some_and(char::is_uppercase) => {
            if modifiers & video_filters::SHORTCUT_SHIFT == 0 {
                parts.push("Shift".to_owned());
            }
            key.to_lowercase()
        }
        _ => key.to_owned(),
    };
    parts.push(std::mem::take(&mut key));
    slint::Keys::from_parts(parts.iter().map(String::as_str)).unwrap_or_default()
}

fn format_shortcut(key: &str, modifiers: u8) -> String {
    if key.is_empty() {
        return String::new();
    }
    #[cfg(target_os = "macos")]
    let labels = [
        (video_filters::SHORTCUT_CONTROL, "⌃"),
        (video_filters::SHORTCUT_ALT, "⌥"),
        (video_filters::SHORTCUT_SHIFT, "⇧"),
        (video_filters::SHORTCUT_META, "⌘"),
    ];
    #[cfg(not(target_os = "macos"))]
    let labels = [
        (video_filters::SHORTCUT_CONTROL, "Ctrl+"),
        (video_filters::SHORTCUT_ALT, "Alt+"),
        (video_filters::SHORTCUT_SHIFT, "Shift+"),
        (video_filters::SHORTCUT_META, "Win+"),
    ];
    let mut result = labels
        .iter()
        .filter(|(flag, _)| modifiers & flag != 0)
        .map(|(_, label)| *label)
        .collect::<String>();
    result.push_str(key);
    result
}

fn persist(settings: &Settings, path: &std::path::Path, is_audio: bool) -> Result<(), String> {
    let settings = settings
        .lock()
        .map_err(|_| "Saved filter settings are unavailable")?
        .clone();
    crate::preference_store::update(path, &riina_core::Preferences::default(), |preferences| {
        if is_audio {
            preferences.audio_filters = settings;
        } else {
            preferences.video_filters = settings;
        }
        Ok(())
    })
}
