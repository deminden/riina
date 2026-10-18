use crate::{
    PlayerWindow, SavedFilterMenuRow, VideoFilterParameterRow, VideoFilterWindowRow,
    VideoFiltersWindow,
};
use riina_core::{Playback, video_filters};
use slint::{ComponentHandle, ModelRc, VecModel, winit_030::WinitWindowAccessor};
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

type Settings = Arc<Mutex<video_filters::Settings>>;
type WindowSlot = Rc<RefCell<Option<VideoFiltersWindow>>>;

#[derive(Default)]
struct PresetDraft {
    index: Option<usize>,
    values: Vec<String>,
}

pub(crate) fn install(
    app: &PlayerWindow,
    latest: Arc<Mutex<Playback>>,
    video_settings: Settings,
    audio_settings: Settings,
    player: riina_mpv::CommandSender,
    preferences_path: PathBuf,
) -> (
    (WindowSlot, Rc<slint::Timer>),
    (WindowSlot, Rc<slint::Timer>),
) {
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
            settings.lock().ok().and_then(|settings| {
                settings
                    .saved
                    .iter()
                    .find(|saved| {
                        saved.shortcut_key == key.as_str() && saved.shortcut_modifiers == modifiers
                    })
                    .map(|saved| saved.filter.clone())
            })
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
    (video, audio)
}

fn install_for_kind(
    app: &PlayerWindow,
    latest: Arc<Mutex<Playback>>,
    settings: Settings,
    player: riina_mpv::CommandSender,
    preferences_path: PathBuf,
    is_audio: bool,
) -> (WindowSlot, Rc<slint::Timer>) {
    let slot = Rc::new(RefCell::new(None::<VideoFiltersWindow>));
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
            window
                .window()
                .on_close_requested(|| slint::CloseRequestResponse::HideWindow);
            crate::platform::center_auxiliary_window(&window, &app);
            *show_slot.borrow_mut() = Some(window.clone_strong());
            window
        };
        refresh(&app, &window, &show_latest, &show_settings, is_audio);
        if let Err(error) = window.show() {
            app.set_status(format!("Could not show filter window: {error}").into());
        } else {
            crate::platform::enable_video_filters_acrylic(&window);
        }
    };
    if is_audio {
        app.on_open_audio_filters(open_filter_window);
    } else {
        app.on_open_video_filters(open_filter_window);
    }

    let timer = Rc::new(slint::Timer::default());
    let timer_app = app.as_weak();
    let timer_slot = slot.clone();
    let timer_latest = latest;
    let timer_settings = settings;
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(300),
        move || {
            let Some(app) = timer_app.upgrade() else {
                return;
            };
            let Some(window) = timer_slot
                .borrow()
                .as_ref()
                .map(VideoFiltersWindow::clone_strong)
            else {
                return;
            };
            if window.window().is_visible() {
                refresh(&app, &window, &timer_latest, &timer_settings, is_audio);
            }
        },
    );
    (slot, timer)
}

fn connect_window(
    app: &PlayerWindow,
    window: &VideoFiltersWindow,
    latest: Arc<Mutex<Playback>>,
    settings: Settings,
    player: riina_mpv::CommandSender,
    preferences_path: PathBuf,
    is_audio: bool,
) {
    let draft = Rc::new(RefCell::new(PresetDraft::default()));
    let weak_window = window.as_weak();
    let select_draft = draft.clone();
    window.on_select_preset(move |index| {
        let Some(window) = weak_window.upgrade() else {
            return;
        };
        if index <= 0 {
            *select_draft.borrow_mut() = PresetDraft::default();
            window.set_preset_parameters(ModelRc::new(VecModel::from(Vec::<
                VideoFilterParameterRow,
            >::new())));
            return;
        }
        let Some(preset) = usize::try_from(index - 1).ok() else {
            return;
        };
        let values = if is_audio {
            video_filters::default_audio_parameter_values(preset)
        } else {
            video_filters::default_parameter_values(preset)
        };
        let Some(values) = values else {
            return;
        };
        *select_draft.borrow_mut() = PresetDraft {
            index: Some(preset),
            values,
        };
        update_preset_form(&window, &select_draft.borrow(), is_audio);
    });

    let text_draft = draft.clone();
    let text_window = window.as_weak();
    window.on_update_preset_text(move |parameter, value| {
        if let Some(window) = text_window.upgrade()
            && let Some(slot) = text_draft.borrow_mut().values.get_mut(parameter as usize)
        {
            *slot = value.to_string();
            update_preset_form(&window, &text_draft.borrow(), is_audio);
        }
    });

    let number_draft = draft.clone();
    let number_window = window.as_weak();
    window.on_update_preset_number(move |parameter, value| {
        if let Some(window) = number_window.upgrade()
            && let Some(slot) = number_draft.borrow_mut().values.get_mut(parameter as usize)
        {
            *slot = value.to_string();
            update_preset_form(&window, &number_draft.borrow(), is_audio);
        }
    });

    let choice_draft = draft.clone();
    let choice_window = window.as_weak();
    window.on_update_preset_choice(move |parameter, choice| {
        if let (Some(window), Some(preset)) = (choice_window.upgrade(), choice_draft.borrow().index)
            && let Some(spec) = preset_parameters(preset, is_audio)
                .and_then(|parameters| parameters.get(parameter as usize))
            && let Some(value) = spec.choices.get(choice as usize)
            && let Some(slot) = choice_draft.borrow_mut().values.get_mut(parameter as usize)
        {
            *slot = (*value).to_owned();
            update_preset_form(&window, &choice_draft.borrow(), is_audio);
        }
    });

    let browse_draft = draft.clone();
    let browse_window = window.as_weak();
    let browse_app = app.as_weak();
    window.on_browse_filter_file(move || {
        let Some(app) = browse_app.upgrade() else {
            return;
        };
        if is_audio || app.get_dialog_open() {
            return;
        }
        app.set_dialog_open(true);
        app.invoke_activity();
        let weak_app = app.as_weak();
        let weak_window = browse_window.clone();
        let selection_window = browse_window.clone();
        let selection_draft = browse_draft.clone();
        let path_result = slint::spawn_local(async move {
            let selected = async {
                let Some(window) = weak_window.upgrade() else {
                    return Ok(None);
                };
                let owner = window
                    .window()
                    .winit_window()
                    .await
                    .map_err(|error| error.to_string())?;
                let file = rfd::AsyncFileDialog::new()
                    .set_title("Choose a 3D LUT file")
                    .set_parent(owner.as_ref())
                    .pick_file()
                    .await;
                file.map(|file| {
                    file.path()
                        .to_str()
                        .map(str::to_owned)
                        .ok_or_else(|| "The selected path contains invalid Unicode".to_owned())
                })
                .transpose()
            }
            .await;
            let Some(app) = weak_app.upgrade() else {
                return;
            };
            app.set_dialog_open(false);
            app.invoke_activity();
            match selected {
                Ok(Some(path)) => {
                    let Some(window) = selection_window.upgrade() else {
                        return;
                    };
                    let mut draft = selection_draft.borrow_mut();
                    if let Some(index) = draft.index
                        && let Some(parameter) =
                            preset_parameters(index, false).and_then(|parameters| {
                                parameters
                                    .iter()
                                    .position(|parameter| parameter.name == "file")
                            })
                        && let Some(value) = draft.values.get_mut(parameter)
                    {
                        *value = path;
                        update_preset_form(&window, &draft, false);
                    }
                }
                Ok(None) => {}
                Err(error) => app.set_status(error.into()),
            }
        });
        if let Err(error) = path_result {
            app.set_dialog_open(false);
            app.invoke_activity();
            app.set_status(error.to_string().into());
        }
    });

    let raw_draft = draft.clone();
    let raw_window = window.as_weak();
    window.on_raw_filter_edited(move || {
        *raw_draft.borrow_mut() = PresetDraft::default();
        if let Some(window) = raw_window.upgrade() {
            window.set_preset_index(0);
            window.set_preset_parameters(ModelRc::new(VecModel::from(Vec::<
                VideoFilterParameterRow,
            >::new())));
        }
    });

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
            let playback = remove_latest.lock().ok()?;
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

    let weak_app = app.as_weak();
    let save_settings = settings.clone();
    let save_path = preferences_path.clone();
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
    let remove_path = preferences_path.clone();
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
    let update_path = preferences_path.clone();
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

fn preset_parameters(
    index: usize,
    is_audio: bool,
) -> Option<&'static [video_filters::ParameterSpec]> {
    if is_audio {
        video_filters::audio_preset_parameters(index)
    } else {
        video_filters::preset_parameters(index)
    }
}

fn update_preset_form(window: &VideoFiltersWindow, draft: &PresetDraft, is_audio: bool) {
    let Some(index) = draft.index else {
        window.set_preset_parameters(ModelRc::new(VecModel::from(
            Vec::<VideoFilterParameterRow>::new(),
        )));
        return;
    };
    let Some(specs) = preset_parameters(index, is_audio) else {
        return;
    };
    let rows = specs
        .iter()
        .zip(&draft.values)
        .map(|(spec, value)| {
            let kind = match spec.kind {
                video_filters::ParameterKind::Text => 0,
                video_filters::ParameterKind::Integer => 1,
                video_filters::ParameterKind::Float => 2,
                video_filters::ParameterKind::Choose => 3,
            };
            VideoFilterParameterRow {
                label: spec.label.into(),
                kind,
                value: value.as_str().into(),
                path_input: spec.name == "file",
                number_value: value.parse::<f64>().unwrap_or(spec.minimum) as f32,
                minimum: spec.minimum as f32,
                maximum: spec.maximum as f32,
                step: spec.step as f32,
                choices: ModelRc::new(VecModel::from(
                    spec.choices
                        .iter()
                        .map(|choice| (*choice).into())
                        .collect::<Vec<_>>(),
                )),
                choice_index: spec
                    .choices
                    .iter()
                    .position(|choice| *choice == value)
                    .unwrap_or(0) as i32,
            }
        })
        .collect::<Vec<_>>();
    window.set_preset_parameters(ModelRc::new(VecModel::from(rows)));
    let filter = if is_audio {
        video_filters::build_audio_preset_filter(index, &draft.values)
    } else {
        video_filters::build_preset_filter(index, &draft.values)
    };
    if let Ok(filter) = filter {
        window.set_filter_string(filter.into());
    }
}

fn refresh(
    app: &PlayerWindow,
    window: &VideoFiltersWindow,
    latest: &Arc<Mutex<Playback>>,
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
    let Ok(settings) = settings.lock() else {
        return;
    };
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
    latest: &Arc<Mutex<Playback>>,
    settings: &Settings,
    is_audio: bool,
) {
    let playback = latest
        .lock()
        .map(|playback| playback.clone())
        .unwrap_or_default();
    let Ok(settings) = settings.lock() else {
        return;
    };
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
