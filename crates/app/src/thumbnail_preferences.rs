use crate::{PlayerWindow, PreferencesWindow};
use riina_core::thumbnail_preferences::Settings;

pub(crate) fn install_player(window: &PlayerWindow, settings: &Settings) {
    window.set_thumbnail_preview_enabled(settings.enabled);
    window.set_thumbnail_cache_size_text(settings.maximum_cache_size_mb.to_string().into());
    window.set_thumbnail_remote_enabled(settings.enable_for_remote_files);
    window.set_thumbnail_width_text(settings.width.to_string().into());
}

pub(crate) fn current_player(window: &PlayerWindow) -> Result<Settings, String> {
    current(
        window.get_thumbnail_preview_enabled(),
        window.get_thumbnail_cache_size_text().as_str(),
        window.get_thumbnail_remote_enabled(),
        window.get_thumbnail_width_text().as_str(),
    )
}

pub(crate) fn install_window(window: &PreferencesWindow, settings: &Settings) {
    window.set_thumbnail_preview_enabled(settings.enabled);
    window.set_thumbnail_cache_size_text(settings.maximum_cache_size_mb.to_string().into());
    window.set_thumbnail_remote_enabled(settings.enable_for_remote_files);
    window.set_thumbnail_width_text(settings.width.to_string().into());
}

pub(crate) fn current_window(window: &PreferencesWindow) -> Result<Settings, String> {
    current(
        window.get_thumbnail_preview_enabled(),
        window.get_thumbnail_cache_size_text().as_str(),
        window.get_thumbnail_remote_enabled(),
        window.get_thumbnail_width_text().as_str(),
    )
}

fn current(
    enabled: bool,
    maximum_cache_size_mb: &str,
    enable_for_remote_files: bool,
    width: &str,
) -> Result<Settings, String> {
    let settings = Settings {
        enabled,
        maximum_cache_size_mb: maximum_cache_size_mb
            .parse()
            .map_err(|_| "Thumbnail cache size must be a whole number of megabytes".to_owned())?,
        enable_for_remote_files,
        width: width
            .parse()
            .map_err(|_| "Thumbnail width must be a whole number of points".to_owned())?,
    };
    settings.validate().map_err(str::to_owned)?;
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::current;
    use riina_core::thumbnail_preferences::Settings;

    #[test]
    fn numeric_settings_parse_as_whole_numbers_and_allow_a_zero_cache_budget() {
        assert_eq!(
            current(true, "0", false, "120").unwrap(),
            Settings {
                maximum_cache_size_mb: 0,
                ..Settings::default()
            }
        );
        assert!(current(true, "1.5", false, "120").is_err());
        assert!(current(true, "500", false, "2001").is_err());
    }
}
