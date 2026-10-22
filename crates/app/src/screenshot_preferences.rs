//! Screenshot intent shared by capture and session persistence.
use crate::PlayerWindow;
use riina_core::screenshot::{Format, Settings};

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
