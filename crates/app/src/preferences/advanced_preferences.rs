//! Advanced mpv options stay opt-in and are applied before the next engine starts.
use crate::{AdvancedMpvOptionRow, PlayerWindow, PreferencesWindow};
use riina_core::advanced_preferences::{LogLevel, MpvOption, Settings};
use slint::{ComponentHandle, Model, ModelRc, VecModel, winit_030::WinitWindowAccessor};
use std::{cell::RefCell, path::Path, path::PathBuf, rc::Rc};

pub(crate) fn install_window(window: &PreferencesWindow, settings: &Settings) {
    window.set_advanced_enabled(settings.enabled);
    window.set_advanced_log_level_index(log_level_index(settings.log_level));
    window.set_advanced_logging_enabled(settings.logging_enabled);
    window.set_advanced_use_mpv_osd(settings.use_mpv_osd);
    window.set_advanced_use_config_directory(settings.use_config_directory);
    window.set_advanced_config_directory_text(settings.config_directory.clone().into());
    let options = Rc::new(RefCell::new(settings.user_options.clone()));
    refresh_options(window, &options);

    let weak = window.as_weak();
    let shared = options.clone();
    window.on_advanced_option_added(move || {
        let Some(window) = weak.upgrade() else { return };
        let index = {
            let mut options = shared.borrow_mut();
            if options.len() >= 256 {
                return;
            }
            options.push(MpvOption {
                name: "name".into(),
                value: "value".into(),
            });
            options.len() as i32 - 1
        };
        refresh_options(&window, &shared);
        window.set_advanced_mpv_option_selection(index);
        window.invoke_settings_changed();
    });
    let weak = window.as_weak();
    let shared = options.clone();
    window.on_advanced_option_removed(move |index| {
        let Some(window) = weak.upgrade() else { return };
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let selection = {
            let mut options = shared.borrow_mut();
            if index >= options.len() {
                return;
            }
            options.remove(index);
            index.min(options.len().saturating_sub(1))
        };
        refresh_options(&window, &shared);
        let count = shared.borrow().len();
        window.set_advanced_mpv_option_selection(if count == 0 { -1 } else { selection as i32 });
        window.invoke_settings_changed();
    });
    let weak = window.as_weak();
    let shared = options.clone();
    window.on_advanced_option_edited(move |index, name, value| {
        let (Some(window), Ok(index)) = (weak.upgrade(), usize::try_from(index)) else {
            return;
        };
        {
            let mut options = shared.borrow_mut();
            let Some(option) = options.get_mut(index) else {
                return;
            };
            (option.name, option.value) = split_pasted_option(&name, &value);
        }
        refresh_options(&window, &shared);
    });
}

fn split_pasted_option(name: &str, value: &str) -> (String, String) {
    // Match IINA's key=value paste shortcut when a complete pair reaches the key field.
    if let Some((key, pasted_value)) = name.split_once('=') {
        let key = key.trim();
        let pasted_value = pasted_value.trim();
        if !key.is_empty() && !pasted_value.is_empty() {
            return (key.into(), pasted_value.into());
        }
    }
    (name.into(), value.into())
}

pub(crate) fn connect_actions(
    app: &PlayerWindow,
    window: &PreferencesWindow,
    preferences_path: PathBuf,
) {
    let app_weak = app.as_weak();
    let weak_window = window.as_weak();
    let log_directory = preferences_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    window.on_advanced_open_log_folder(move || {
        if let Err(error) = crate::platform::open_directory(&log_directory)
            && let Some(app) = app_weak.upgrade()
        {
            app.set_status(format!("Could not open the log directory: {error}").into());
        }
    });

    let log_file = preferences_path.with_file_name("native-player.log");
    crate::log_viewer::connect(app, window, log_file);

    let chooser_app = app.as_weak();
    window.on_advanced_choose_config_directory(move || {
        let (Some(app), Some(window)) = (chooser_app.upgrade(), weak_window.upgrade()) else {
            return;
        };
        if app.get_dialog_open() || window.get_folder_dialog_open() {
            return;
        }
        app.set_dialog_open(true);
        window.set_folder_dialog_open(true);
        let app = app.as_weak();
        let window = window.as_weak();
        let failure_app = app.clone();
        let failure_window = window.clone();
        let started = slint::spawn_local(async move {
            let (Some(app), Some(window)) = (app.upgrade(), window.upgrade()) else {
                return;
            };
            let result = async {
                let owner = window
                    .window()
                    .winit_window()
                    .await
                    .map_err(|error| error.to_string())?;
                let parent = app.as_weak();
                let weak = window.as_weak();
                crate::platform::folder_dialog::pick(owner, move |result| {
                    let (Some(app), Some(window)) = (parent.upgrade(), weak.upgrade()) else {
                        return;
                    };
                    app.set_dialog_open(false);
                    window.set_folder_dialog_open(false);
                    match result {
                        Ok(Some(directory)) => {
                            window.set_advanced_config_directory_text(directory.into());
                            window.set_advanced_use_config_directory(true);
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
            if let Some(app) = failure_app.upgrade() {
                app.set_dialog_open(false);
                app.set_status(error.to_string().into());
            }
            if let Some(window) = failure_window.upgrade() {
                window.set_folder_dialog_open(false);
            }
        }
    });
}

pub(crate) fn current_window(window: &PreferencesWindow) -> Result<Settings, String> {
    let log_level = log_level_from_index(window.get_advanced_log_level_index())
        .ok_or("Choose a valid log level")?;
    let model = window.get_advanced_mpv_options();
    let user_options = (0..model.row_count())
        .filter_map(|index| model.row_data(index))
        .map(|row| MpvOption {
            name: row.name.to_string(),
            value: row.value.to_string(),
        })
        .collect();
    let settings = Settings {
        enabled: window.get_advanced_enabled(),
        logging_enabled: window.get_advanced_logging_enabled(),
        log_level,
        use_mpv_osd: window.get_advanced_use_mpv_osd(),
        use_config_directory: window.get_advanced_use_config_directory(),
        config_directory: window.get_advanced_config_directory_text().to_string(),
        user_options,
    };
    settings.validate()?;
    Ok(settings)
}

pub(crate) fn engine_options(
    settings: &Settings,
    preferences_path: &Path,
) -> Result<Vec<(String, String)>, String> {
    settings.validate()?;
    let mut options = vec![(
        "osd-level".into(),
        if settings.enabled && settings.use_mpv_osd {
            "1"
        } else {
            "0"
        }
        .into(),
    )];
    if !settings.enabled {
        return Ok(options);
    }
    if settings.logging_enabled {
        let log_path = preferences_path.with_file_name("native-player.log");
        options.push(("log-file".into(), log_path.to_string_lossy().into_owned()));
        options.push((
            "msg-level".into(),
            mpv_message_level(settings.log_level).into(),
        ));
    }
    if settings.use_config_directory {
        let directory = resolve_directory(&settings.config_directory)?;
        options.push(("config".into(), "yes".into()));
        options.push(("config-dir".into(), directory));
    }
    options.extend(
        settings
            .user_options
            .iter()
            .map(|option| (option.name.clone(), option.value.clone())),
    );
    Ok(options)
}

fn resolve_directory(value: &str) -> Result<String, String> {
    let home = dirs::home_dir().ok_or("The home directory is unavailable")?;
    let path = if value == "~" {
        home
    } else if let Some(rest) = value
        .strip_prefix("~/")
        .or_else(|| value.strip_prefix("~\\"))
    {
        home.join(rest)
    } else {
        let path = PathBuf::from(value);
        if path.is_absolute() {
            path
        } else {
            std::env::current_dir()
                .map_err(|error| error.to_string())?
                .join(path)
        }
    };
    Ok(path.to_string_lossy().into_owned())
}

fn refresh_options(window: &PreferencesWindow, options: &Rc<RefCell<Vec<MpvOption>>>) {
    let rows = options
        .borrow()
        .iter()
        .map(|option| AdvancedMpvOptionRow {
            name: option.name.clone().into(),
            value: option.value.clone().into(),
        })
        .collect::<Vec<_>>();
    window.set_advanced_mpv_options(ModelRc::new(VecModel::from(rows)));
}

fn log_level_index(level: LogLevel) -> i32 {
    match level {
        LogLevel::Verbose => 0,
        LogLevel::Debug => 1,
        LogLevel::Warning => 2,
        LogLevel::Error => 3,
    }
}

fn log_level_from_index(index: i32) -> Option<LogLevel> {
    match index {
        0 => Some(LogLevel::Verbose),
        1 => Some(LogLevel::Debug),
        2 => Some(LogLevel::Warning),
        3 => Some(LogLevel::Error),
        _ => None,
    }
}

fn mpv_message_level(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Verbose => "all=v",
        LogLevel::Debug => "all=debug",
        LogLevel::Warning => "all=warn",
        LogLevel::Error => "all=error",
    }
}

#[cfg(test)]
mod tests {
    use super::{engine_options, resolve_directory, split_pasted_option};
    use riina_core::advanced_preferences::{LogLevel, MpvOption, Settings};
    use std::path::{Path, PathBuf};

    #[test]
    fn relative_mpv_config_directories_become_absolute() {
        let resolved = resolve_directory("profile/mpv").unwrap();
        assert!(PathBuf::from(resolved).is_absolute());
    }

    #[test]
    fn pasted_key_value_pairs_fill_both_editor_fields_without_dropping_equals_in_value() {
        assert_eq!(
            split_pasted_option("  profile = fast ", "old"),
            ("profile".into(), "fast".into())
        );
        assert_eq!(
            split_pasted_option("  screenshot-template = frame=%n ", "old"),
            ("screenshot-template".into(), "frame=%n".into())
        );
        assert_eq!(
            split_pasted_option("incomplete=", "existing"),
            ("incomplete=".into(), "existing".into())
        );
    }

    #[test]
    fn engine_defaults_keep_the_internal_osd_off_and_advanced_options_opt_in() {
        assert_eq!(
            engine_options(&Settings::default(), Path::new("settings.json")).unwrap(),
            [("osd-level".into(), "0".into())]
        );

        let disabled = Settings {
            enabled: false,
            user_options: vec![MpvOption {
                name: "profile".into(),
                value: "fast".into(),
            }],
            use_mpv_osd: true,
            ..Settings::default()
        };
        assert_eq!(
            engine_options(&disabled, Path::new("settings.json")).unwrap(),
            [("osd-level".into(), "0".into())]
        );
    }

    #[test]
    fn enabled_logging_and_user_options_map_to_native_startup_settings() {
        let settings = Settings {
            enabled: true,
            logging_enabled: true,
            log_level: LogLevel::Warning,
            use_mpv_osd: true,
            user_options: vec![MpvOption {
                name: "profile".into(),
                value: "fast".into(),
            }],
            ..Settings::default()
        };
        let preferences_path = Path::new("C:/riina/preferences.json");
        let expected_log_path = preferences_path
            .with_file_name("native-player.log")
            .to_string_lossy()
            .into_owned();
        let options = engine_options(&settings, preferences_path).unwrap();
        assert_eq!(options[0], ("osd-level".into(), "1".into()));
        assert_eq!(options[1], ("log-file".into(), expected_log_path));
        assert_eq!(options[2], ("msg-level".into(), "all=warn".into()));
        assert_eq!(options[3], ("profile".into(), "fast".into()));
    }
}
