//! The UI owns live preferences; capture and session saving read the same intent.
use crate::PlayerWindow;
use riina_core::screenshot::{Format, Settings};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{cell::RefCell, path::PathBuf, rc::Rc};

pub(crate) fn connect(app: &PlayerWindow, path: PathBuf, baseline: riina_core::Preferences) {
    let current = Rc::new(RefCell::new(None::<crate::PreferencesWindow>));
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
            let parent = app.as_weak();
            let weak = window.as_weak();
            let path = path.clone();
            let baseline = baseline.clone();
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
                let result =
                    self::current(&app).and_then(|screenshot| {
                        let preferences =
                            riina_core::Preferences {
                                screenshot,
                                volume: f64::from(app.get_volume())
                                    .min(if app.get_volume_boost() { 200.0 } else { 100.0 }),
                                volume_boost: app.get_volume_boost(),
                                ..baseline.clone()
                            };
                        let text = serde_json::to_string_pretty(&preferences)
                            .map_err(|error| error.to_string())?;
                        crate::platform::save_preferences(&path, &text)
                            .map_err(|error| error.to_string())
                    });
                if let Err(error) = result {
                    install(&app, &previous);
                    refresh(&window, &app);
                    app.set_status(error.into());
                }
            });
            let weak = window.as_weak();
            window.window().on_close_requested(move || {
                if let Some(window) = weak.upgrade() {
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
    let index = ["png", "jpg", "jpeg", "webp", "jxl"]
        .iter()
        .position(|format| *format == app.get_screenshot_format().as_str())
        .map_or(-1, |index| index as i32);
    window.set_format_index(index);
    window.set_include_subtitles(app.get_screenshot_include_subtitles());
    window.set_show_preview(app.get_screenshot_show_preview());
}

pub(crate) fn install(app: &PlayerWindow, settings: &Settings) {
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
