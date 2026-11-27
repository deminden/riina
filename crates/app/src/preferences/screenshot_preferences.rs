//! Screenshot intent shared by capture and session persistence.
use crate::{PlayerWindow, PreferencesWindow};
use riina_core::screenshot::{Format, Settings};
use slint::ComponentHandle;

// IINA's current Settings pane sorts the format identifiers alphabetically.
const WINDOW_FORMATS: [Format; 6] = [
    Format::Avif,
    Format::Jpeg,
    Format::Jpg,
    Format::Jxl,
    Format::Png,
    Format::Webp,
];

pub(crate) fn install(app: &PlayerWindow, settings: &Settings) {
    let player_preferences = app.global::<riina_ui::PlayerPreferences>();
    player_preferences.set_screenshot_save_to_file(settings.destinations.save_to_file);
    player_preferences.set_screenshot_copy_to_clipboard(settings.destinations.copy_to_clipboard);
    player_preferences.set_screenshot_format(settings.encoding.format.native_value().into());
    player_preferences.set_screenshot_template(settings.encoding.template.clone().into());
    player_preferences.set_screenshot_custom_directory(settings.directory.is_some());
    player_preferences
        .set_screenshot_directory(settings.directory.clone().unwrap_or_default().into());
    player_preferences.set_screenshot_include_subtitles(settings.include_subtitles);
    player_preferences.set_screenshot_show_preview(settings.show_preview);
}

pub(crate) fn install_window(window: &PreferencesWindow, settings: &Settings) {
    let general = window.global::<riina_ui::GeneralSettingsState>();
    let index = WINDOW_FORMATS
        .iter()
        .position(|format| *format == settings.encoding.format)
        .map_or(-1, |index| index as i32);
    general.set_format_index(index);
    general.set_include_subtitles(settings.include_subtitles);
    general.set_show_preview(settings.show_preview);
    general.set_save_to_file(settings.destinations.save_to_file);
    general.set_copy_to_clipboard(settings.destinations.copy_to_clipboard);
    window.set_custom_directory(settings.directory.is_some());
    let directory = settings.directory.clone().or_else(|| {
        dirs::picture_dir().map(|path| path.join("Screenshots").to_string_lossy().into_owned())
    });
    general.set_directory(directory.unwrap_or_default().into());
}

pub(crate) fn read_window(
    window: &PreferencesWindow,
    previous: &Settings,
) -> Result<Settings, String> {
    let general = window.global::<riina_ui::GeneralSettingsState>();
    let mut settings = previous.clone();
    if let Ok(index) = usize::try_from(general.get_format_index())
        && let Some(format) = WINDOW_FORMATS.get(index)
    {
        settings.encoding.format = *format;
    }
    settings.include_subtitles = general.get_include_subtitles();
    settings.show_preview = general.get_show_preview();
    settings.destinations.save_to_file = general.get_save_to_file();
    settings.destinations.copy_to_clipboard = general.get_copy_to_clipboard();
    settings.directory = window
        .get_custom_directory()
        .then(|| general.get_directory().to_string());
    settings.validate()?;
    Ok(settings)
}

pub(crate) fn current(app: &PlayerWindow) -> Result<Settings, String> {
    let player_preferences = app.global::<riina_ui::PlayerPreferences>();
    let format = match player_preferences.get_screenshot_format().as_str() {
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
            save_to_file: player_preferences.get_screenshot_save_to_file(),
            copy_to_clipboard: player_preferences.get_screenshot_copy_to_clipboard(),
        },
        encoding: riina_core::screenshot::Encoding {
            format,
            template: player_preferences.get_screenshot_template().to_string(),
        },
        directory: player_preferences
            .get_screenshot_custom_directory()
            .then(|| player_preferences.get_screenshot_directory().to_string()),
        include_subtitles: player_preferences.get_screenshot_include_subtitles(),
        show_preview: player_preferences.get_screenshot_show_preview(),
    };
    // An invalid draft must not replace saved preferences or reach libmpv.
    settings.validate()?;
    Ok(settings)
}
