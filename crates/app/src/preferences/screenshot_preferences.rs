//! Screenshot intent shared by capture and session persistence.
use crate::PlayerWindow;
use riina_core::screenshot::{Format, Settings};
use slint::ComponentHandle;

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

pub(crate) fn project_window_edit(app: &PlayerWindow, window: &crate::PreferencesWindow) {
    let player_preferences = app.global::<riina_ui::PlayerPreferences>();
    let formats = ["png", "jpg", "jpeg", "webp", "jxl"];
    // AVIF can be configured externally but has no item in the pinned
    // General pane. Editing another control must preserve that choice.
    if let Ok(index) = usize::try_from(
        window
            .global::<riina_ui::GeneralSettingsState>()
            .get_format_index(),
    ) && let Some(format) = formats.get(index)
    {
        player_preferences.set_screenshot_format((*format).into());
    }
    player_preferences.set_screenshot_include_subtitles(
        window
            .global::<riina_ui::GeneralSettingsState>()
            .get_include_subtitles(),
    );
    player_preferences.set_screenshot_show_preview(
        window
            .global::<riina_ui::GeneralSettingsState>()
            .get_show_preview(),
    );
    player_preferences.set_screenshot_save_to_file(
        window
            .global::<riina_ui::GeneralSettingsState>()
            .get_save_to_file(),
    );
    player_preferences.set_screenshot_copy_to_clipboard(
        window
            .global::<riina_ui::GeneralSettingsState>()
            .get_copy_to_clipboard(),
    );
    player_preferences.set_screenshot_custom_directory(window.get_custom_directory());
    player_preferences.set_screenshot_directory(
        window
            .global::<riina_ui::GeneralSettingsState>()
            .get_directory(),
    );
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
