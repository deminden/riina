//! Translate editable controller settings into the shared validated policy.
use crate::{PlayerWindow, PreferencesWindow};
use riina_core::osc_preferences::Settings;
use slint::{ComponentHandle, Model, ModelRc, VecModel};

pub(crate) fn current_player(window: &PlayerWindow) -> Settings {
    let defaults = Settings::default();
    Settings {
        auto_hide_enabled: window.get_control_auto_hide_enabled(),
        auto_hide_timeout_secs: f64::from(window.get_control_auto_hide_timeout_secs()),
        toolbar_buttons: toolbar_mask([
            window.get_toolbar_show_plugins(),
            window.get_toolbar_show_pip(),
            window.get_toolbar_show_playlist(),
            window.get_toolbar_show_quick_settings(),
            window.get_toolbar_show_fullscreen(),
            window.get_toolbar_show_music_mode(),
            window.get_toolbar_show_subtitle_track(),
            window.get_toolbar_show_screenshot(),
            window.get_toolbar_show_live_text(),
        ]),
        toolbar_order: read_order(window.get_toolbar_order()).unwrap_or(defaults.toolbar_order),
    }
}

pub(crate) fn current_preferences(window: &PreferencesWindow) -> Result<Settings, String> {
    let timeout_secs = parse_timeout(window.get_control_auto_hide_timeout_text().as_str())?;
    let toolbar_order = read_order(window.get_toolbar_order())?;
    Settings {
        auto_hide_enabled: window.get_control_auto_hide_enabled(),
        auto_hide_timeout_secs: timeout_secs,
        toolbar_buttons: toolbar_mask([
            window.get_toolbar_show_plugins(),
            window.get_toolbar_show_pip(),
            window.get_toolbar_show_playlist(),
            window.get_toolbar_show_quick_settings(),
            window.get_toolbar_show_fullscreen(),
            window.get_toolbar_show_music_mode(),
            window.get_toolbar_show_subtitle_track(),
            window.get_toolbar_show_screenshot(),
            window.get_toolbar_show_live_text(),
        ]),
        toolbar_order,
    }
    .validate()
    .map_err(str::to_owned)
}

pub(crate) fn install_player(window: &PlayerWindow, settings: Settings) {
    window.set_control_auto_hide_enabled(settings.auto_hide_enabled);
    window.set_control_auto_hide_timeout_secs(settings.auto_hide_timeout_secs as f32);
    refresh_visible_order_for(window, settings);
    install_toolbar(window, settings);
}

pub(crate) fn refresh_visible_order(window: &PlayerWindow) {
    refresh_visible_order_for(window, current_player(window));
}

fn refresh_visible_order_for(window: &PlayerWindow, settings: Settings) {
    let live_text_enabled = window.get_live_text_enabled() && window.get_live_text_available();
    let visible = settings
        .toolbar_order
        .into_iter()
        .filter(|action| toolbar_action_visible(*action, settings.toolbar_buttons))
        .filter(|action| *action != 8 || live_text_enabled)
        .map(i32::from)
        .collect::<Vec<_>>();
    window.set_toolbar_visible_order(ModelRc::new(VecModel::from(visible)));
}

pub(crate) fn install_preferences(window: &PreferencesWindow, settings: Settings) {
    window.set_control_auto_hide_enabled(settings.auto_hide_enabled);
    window
        .set_control_auto_hide_timeout_text(format_timeout(settings.auto_hide_timeout_secs).into());
    install_toolbar(window, settings);
}

pub(crate) fn connect_preferences(window: &PreferencesWindow) {
    let weak = window.as_weak();
    window.on_toolbar_move(move |position, destination| {
        let Some(window) = weak.upgrade() else { return };
        let model = window.get_toolbar_order();
        let order: Vec<i32> = (0..model.row_count())
            .filter_map(|index| model.row_data(index))
            .collect();
        if let Some(order) = reordered_toolbar_order(&order, position, destination) {
            window.set_toolbar_order(ModelRc::new(VecModel::from(order)));
            window.invoke_settings_changed();
        }
    });
    let weak = window.as_weak();
    window.on_toolbar_drag_finished(move || {
        let Some(window) = weak.upgrade() else { return };
        window.invoke_toolbar_move(
            window.get_toolbar_drag_index(),
            window.get_toolbar_drop_index(),
        );
    });
    let weak = window.as_weak();
    window.on_toolbar_reset(move || {
        if let Some(window) = weak.upgrade() {
            install_toolbar(&window, Settings::default());
            window.invoke_settings_changed();
        }
    });
}

fn reordered_toolbar_order(order: &[i32], from: i32, to: i32) -> Option<Vec<i32>> {
    use std::collections::BTreeSet;

    if order.len() != 9 || order.iter().copied().collect::<BTreeSet<_>>() != (0..9).collect() {
        return None;
    }
    let from = usize::try_from(from)
        .ok()
        .filter(|index| *index < order.len())?;
    let to = usize::try_from(to)
        .ok()
        .filter(|index| *index < order.len())?;
    if from == to {
        return None;
    }
    let mut reordered = order.to_vec();
    let action = reordered.remove(from);
    reordered.insert(to, action);
    Some(reordered)
}

fn toolbar_mask(visible: [bool; 9]) -> u16 {
    use riina_core::osc_preferences::Settings;
    const BITS: [u16; 9] = [
        Settings::TOOLBAR_PLUGINS,
        Settings::TOOLBAR_PIP,
        Settings::TOOLBAR_PLAYLIST,
        Settings::TOOLBAR_QUICK_SETTINGS,
        Settings::TOOLBAR_FULLSCREEN,
        Settings::TOOLBAR_MUSIC_MODE,
        Settings::TOOLBAR_SUBTITLE_TRACK,
        Settings::TOOLBAR_SCREENSHOT,
        Settings::TOOLBAR_LIVE_TEXT,
    ];
    visible.into_iter().zip(BITS).fold(
        0,
        |mask, (shown, bit)| {
            if shown { mask | bit } else { mask }
        },
    )
}

fn toolbar_action_visible(action: u8, mask: u16) -> bool {
    use riina_core::osc_preferences::Settings;
    let bit = match action {
        0 => Settings::TOOLBAR_QUICK_SETTINGS,
        1 => Settings::TOOLBAR_PLAYLIST,
        2 => Settings::TOOLBAR_PIP,
        3 => Settings::TOOLBAR_FULLSCREEN,
        4 => Settings::TOOLBAR_MUSIC_MODE,
        5 => Settings::TOOLBAR_SUBTITLE_TRACK,
        6 => Settings::TOOLBAR_SCREENSHOT,
        7 => Settings::TOOLBAR_PLUGINS,
        8 => Settings::TOOLBAR_LIVE_TEXT,
        _ => return false,
    };
    mask & bit != 0
}

fn install_toolbar(window: &impl ToolbarVisibility, settings: Settings) {
    use riina_core::osc_preferences::Settings;
    let mask = settings.toolbar_buttons;
    window.set_toolbar_show_plugins(mask & Settings::TOOLBAR_PLUGINS != 0);
    window.set_toolbar_show_pip(mask & Settings::TOOLBAR_PIP != 0);
    window.set_toolbar_show_playlist(mask & Settings::TOOLBAR_PLAYLIST != 0);
    window.set_toolbar_show_quick_settings(mask & Settings::TOOLBAR_QUICK_SETTINGS != 0);
    window.set_toolbar_show_fullscreen(mask & Settings::TOOLBAR_FULLSCREEN != 0);
    window.set_toolbar_show_music_mode(mask & Settings::TOOLBAR_MUSIC_MODE != 0);
    window.set_toolbar_show_subtitle_track(mask & Settings::TOOLBAR_SUBTITLE_TRACK != 0);
    window.set_toolbar_show_screenshot(mask & Settings::TOOLBAR_SCREENSHOT != 0);
    window.set_toolbar_show_live_text(mask & Settings::TOOLBAR_LIVE_TEXT != 0);
    window.install_toolbar_order(settings.toolbar_order);
}

fn read_order(model: ModelRc<i32>) -> Result<[u8; 9], String> {
    if model.row_count() != 9 {
        return Err("Choose all nine toolbar actions in a valid order".to_owned());
    }
    let mut order = [0; 9];
    for (index, value) in order.iter_mut().enumerate() {
        *value = model
            .row_data(index)
            .and_then(|value| u8::try_from(value).ok())
            .ok_or_else(|| "Choose all nine toolbar actions in a valid order".to_owned())?;
    }
    Ok(order)
}

trait ToolbarVisibility {
    fn set_toolbar_show_plugins(&self, value: bool);
    fn set_toolbar_show_pip(&self, value: bool);
    fn set_toolbar_show_playlist(&self, value: bool);
    fn set_toolbar_show_quick_settings(&self, value: bool);
    fn set_toolbar_show_fullscreen(&self, value: bool);
    fn set_toolbar_show_music_mode(&self, value: bool);
    fn set_toolbar_show_subtitle_track(&self, value: bool);
    fn set_toolbar_show_screenshot(&self, value: bool);
    fn set_toolbar_show_live_text(&self, value: bool);
    fn install_toolbar_order(&self, order: [u8; 9]);
}

impl ToolbarVisibility for crate::PlayerWindow {
    fn set_toolbar_show_plugins(&self, value: bool) {
        self.set_toolbar_show_plugins(value);
    }
    fn set_toolbar_show_pip(&self, value: bool) {
        self.set_toolbar_show_pip(value);
    }
    fn set_toolbar_show_playlist(&self, value: bool) {
        self.set_toolbar_show_playlist(value);
    }
    fn set_toolbar_show_quick_settings(&self, value: bool) {
        self.set_toolbar_show_quick_settings(value);
    }
    fn set_toolbar_show_fullscreen(&self, value: bool) {
        self.set_toolbar_show_fullscreen(value);
    }
    fn set_toolbar_show_music_mode(&self, value: bool) {
        self.set_toolbar_show_music_mode(value);
    }
    fn set_toolbar_show_subtitle_track(&self, value: bool) {
        self.set_toolbar_show_subtitle_track(value);
    }
    fn set_toolbar_show_screenshot(&self, value: bool) {
        self.set_toolbar_show_screenshot(value);
    }
    fn set_toolbar_show_live_text(&self, value: bool) {
        self.set_toolbar_show_live_text(value);
    }
    fn install_toolbar_order(&self, order: [u8; 9]) {
        self.set_toolbar_order(ModelRc::new(VecModel::from(
            order.into_iter().map(i32::from).collect::<Vec<_>>(),
        )));
    }
}

impl ToolbarVisibility for crate::PreferencesWindow {
    fn set_toolbar_show_plugins(&self, value: bool) {
        self.set_toolbar_show_plugins(value);
    }
    fn set_toolbar_show_pip(&self, value: bool) {
        self.set_toolbar_show_pip(value);
    }
    fn set_toolbar_show_playlist(&self, value: bool) {
        self.set_toolbar_show_playlist(value);
    }
    fn set_toolbar_show_quick_settings(&self, value: bool) {
        self.set_toolbar_show_quick_settings(value);
    }
    fn set_toolbar_show_fullscreen(&self, value: bool) {
        self.set_toolbar_show_fullscreen(value);
    }
    fn set_toolbar_show_music_mode(&self, value: bool) {
        self.set_toolbar_show_music_mode(value);
    }
    fn set_toolbar_show_subtitle_track(&self, value: bool) {
        self.set_toolbar_show_subtitle_track(value);
    }
    fn set_toolbar_show_screenshot(&self, value: bool) {
        self.set_toolbar_show_screenshot(value);
    }
    fn set_toolbar_show_live_text(&self, value: bool) {
        self.set_toolbar_show_live_text(value);
    }
    fn install_toolbar_order(&self, order: [u8; 9]) {
        self.set_toolbar_order(ModelRc::new(VecModel::from(
            order.into_iter().map(i32::from).collect::<Vec<_>>(),
        )));
    }
}

fn format_timeout(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

fn parse_timeout(text: &str) -> Result<f64, String> {
    text.trim()
        .parse::<f64>()
        .map_err(|_| "Enter a numeric control-bar auto-hide time".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{format_timeout, parse_timeout, reordered_toolbar_order};
    use riina_core::osc_preferences::Settings;

    #[test]
    fn toolbar_drag_moves_an_action_to_the_drop_row() {
        assert_eq!(
            reordered_toolbar_order(&[7, 2, 1, 0, 3, 4, 5, 6, 8], 1, 4),
            Some(vec![7, 1, 0, 3, 2, 4, 5, 6, 8])
        );
        assert_eq!(
            reordered_toolbar_order(&[7, 2, 1, 0, 3, 4, 5, 6, 8], 4, 2),
            Some(vec![7, 2, 3, 1, 0, 4, 5, 6, 8])
        );
    }

    #[test]
    fn toolbar_reordering_rejects_invalid_positions_and_orders() {
        let order = [7, 2, 1, 0, 3, 4, 5, 6, 8];
        assert_eq!(reordered_toolbar_order(&order, 3, 3), None);
        assert_eq!(reordered_toolbar_order(&order, -1, 3), None);
        assert_eq!(reordered_toolbar_order(&order, 3, 9), None);
        assert_eq!(
            reordered_toolbar_order(&[0, 0, 1, 2, 3, 4, 5, 6, 7], 1, 3),
            None
        );
    }

    #[test]
    fn timeout_editor_preserves_fractional_seconds_and_rejects_bad_text() {
        assert_eq!(parse_timeout(" 2.5 ").unwrap(), 2.5);
        assert_eq!(format_timeout(3.0), "3");
        assert_eq!(format_timeout(2.5), "2.5");
        for input in ["", "two"] {
            assert!(parse_timeout(input).is_err());
        }
        for input in ["-1", "NaN", "infinity"] {
            let timeout = parse_timeout(input).unwrap();
            assert!(
                Settings {
                    auto_hide_timeout_secs: timeout,
                    ..Settings::default()
                }
                .validate()
                .is_err()
            );
        }
    }
}
