use crate::{PlayerWindow, PreferencesWindow};
use riina_core::control_preferences::{MouseAction, PinchAction, ScrollAction, SeekType, Settings};

const SINGLE_CLICK_CHOICES: [MouseAction; 3] =
    [MouseAction::None, MouseAction::Pause, MouseAction::HideOsc];
const RIGHT_CLICK_CHOICES: [MouseAction; 6] = [
    MouseAction::None,
    MouseAction::Pause,
    MouseAction::HideOsc,
    MouseAction::PictureInPicture,
    MouseAction::AbLoop,
    MouseAction::ResetSpeed,
];
const FORCE_TOUCH_CHOICES: [MouseAction; 4] = [
    MouseAction::None,
    MouseAction::Fullscreen,
    MouseAction::Pause,
    MouseAction::HideOsc,
];

fn choice_index(action: MouseAction, choices: &[MouseAction]) -> i32 {
    choices
        .iter()
        .position(|choice| *choice == action)
        .unwrap_or(0) as i32
}

fn choice(index: i32, choices: &[MouseAction], label: &str) -> Result<MouseAction, String> {
    usize::try_from(index)
        .ok()
        .and_then(|index| choices.get(index))
        .copied()
        .ok_or_else(|| format!("Invalid {label} action"))
}

pub(crate) fn install_player(app: &PlayerWindow, settings: &Settings) {
    app.set_control_pinch_index(settings.pinch_action.index());
    app.set_control_force_touch_index(settings.force_touch_action.index());
    app.set_control_single_click_index(settings.single_click_action.index());
    app.set_control_double_click_index(settings.double_click_action.index());
    app.set_control_right_click_index(settings.right_click_action.index());
    app.set_control_middle_click_index(settings.middle_click_action.index());
    app.set_control_accepts_first_mouse(settings.accepts_first_mouse);
    app.set_control_vertical_scroll_index(settings.vertical_scroll_action.index());
    app.set_control_horizontal_scroll_index(settings.horizontal_scroll_action.index());
    app.set_control_seek_type_index(settings.seek_type.index());
    app.set_control_seek_sensitivity(i32::from(settings.seek_sensitivity));
    app.set_control_volume_sensitivity(i32::from(settings.volume_sensitivity));
    app.set_control_speed_sensitivity(i32::from(settings.speed_sensitivity));
}

pub(crate) fn current_player(app: &PlayerWindow) -> Result<Settings, String> {
    let settings = Settings {
        pinch_action: PinchAction::from_index(app.get_control_pinch_index())
            .ok_or_else(|| "Invalid pinch action".to_owned())?,
        force_touch_action: MouseAction::from_index(app.get_control_force_touch_index())
            .ok_or_else(|| "Invalid Force Touch action".to_owned())?,
        single_click_action: MouseAction::from_index(app.get_control_single_click_index())
            .ok_or_else(|| "Invalid single-click action".to_owned())?,
        double_click_action: MouseAction::from_index(app.get_control_double_click_index())
            .ok_or_else(|| "Invalid double-click action".to_owned())?,
        right_click_action: MouseAction::from_index(app.get_control_right_click_index())
            .ok_or_else(|| "Invalid right-click action".to_owned())?,
        middle_click_action: MouseAction::from_index(app.get_control_middle_click_index())
            .ok_or_else(|| "Invalid middle-click action".to_owned())?,
        accepts_first_mouse: app.get_control_accepts_first_mouse(),
        vertical_scroll_action: ScrollAction::from_index(app.get_control_vertical_scroll_index())
            .ok_or_else(|| "Invalid vertical-scroll action".to_owned())?,
        horizontal_scroll_action: ScrollAction::from_index(
            app.get_control_horizontal_scroll_index(),
        )
        .ok_or_else(|| "Invalid horizontal-scroll action".to_owned())?,
        seek_type: SeekType::from_index(app.get_control_seek_type_index())
            .ok_or_else(|| "Invalid seek type".to_owned())?,
        seek_sensitivity: u8::try_from(app.get_control_seek_sensitivity())
            .map_err(|_| "Invalid seek sensitivity")?,
        volume_sensitivity: u8::try_from(app.get_control_volume_sensitivity())
            .map_err(|_| "Invalid volume sensitivity")?,
        speed_sensitivity: u8::try_from(app.get_control_speed_sensitivity())
            .map_err(|_| "Invalid speed sensitivity")?,
    };
    settings.validate().map_err(str::to_owned)?;
    Ok(settings)
}

pub(crate) fn install_window(window: &PreferencesWindow, settings: &Settings) {
    window.set_control_pinch_index(settings.pinch_action.index());
    window.set_control_force_touch_choice(choice_index(
        settings.force_touch_action,
        &FORCE_TOUCH_CHOICES,
    ));
    window.set_control_single_click_choice(choice_index(
        settings.single_click_action,
        &SINGLE_CLICK_CHOICES,
    ));
    window.set_control_double_click_choice(settings.double_click_action.index());
    window.set_control_right_click_choice(choice_index(
        settings.right_click_action,
        &RIGHT_CLICK_CHOICES,
    ));
    window.set_control_middle_click_choice(settings.middle_click_action.index());
    window.set_control_accepts_first_mouse(settings.accepts_first_mouse);
    window.set_control_vertical_scroll_index(settings.vertical_scroll_action.index());
    window.set_control_horizontal_scroll_index(settings.horizontal_scroll_action.index());
    window.set_control_seek_type_index(settings.seek_type.index());
    window.set_control_seek_sensitivity(i32::from(settings.seek_sensitivity));
    window.set_control_volume_sensitivity(i32::from(settings.volume_sensitivity));
    window.set_control_speed_sensitivity(i32::from(settings.speed_sensitivity));
}

pub(crate) fn current_window(window: &PreferencesWindow) -> Result<Settings, String> {
    let settings = Settings {
        pinch_action: PinchAction::from_index(window.get_control_pinch_index())
            .ok_or_else(|| "Invalid pinch action".to_owned())?,
        force_touch_action: choice(
            window.get_control_force_touch_choice(),
            &FORCE_TOUCH_CHOICES,
            "Force Touch",
        )?,
        single_click_action: choice(
            window.get_control_single_click_choice(),
            &SINGLE_CLICK_CHOICES,
            "single-click",
        )?,
        double_click_action: MouseAction::from_index(window.get_control_double_click_choice())
            .ok_or_else(|| "Invalid double-click action".to_owned())?,
        right_click_action: choice(
            window.get_control_right_click_choice(),
            &RIGHT_CLICK_CHOICES,
            "right-click",
        )?,
        middle_click_action: MouseAction::from_index(window.get_control_middle_click_choice())
            .ok_or_else(|| "Invalid middle-click action".to_owned())?,
        accepts_first_mouse: window.get_control_accepts_first_mouse(),
        vertical_scroll_action: ScrollAction::from_index(
            window.get_control_vertical_scroll_index(),
        )
        .ok_or_else(|| "Invalid vertical-scroll action".to_owned())?,
        horizontal_scroll_action: ScrollAction::from_index(
            window.get_control_horizontal_scroll_index(),
        )
        .ok_or_else(|| "Invalid horizontal-scroll action".to_owned())?,
        seek_type: SeekType::from_index(window.get_control_seek_type_index())
            .ok_or_else(|| "Invalid seek type".to_owned())?,
        seek_sensitivity: u8::try_from(window.get_control_seek_sensitivity())
            .map_err(|_| "Invalid seek sensitivity")?,
        volume_sensitivity: u8::try_from(window.get_control_volume_sensitivity())
            .map_err(|_| "Invalid volume sensitivity")?,
        speed_sensitivity: u8::try_from(window.get_control_speed_sensitivity())
            .map_err(|_| "Invalid speed sensitivity")?,
    };
    settings.validate().map_err(str::to_owned)?;
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popup_choices_match_iina_available_tags() {
        assert_eq!(choice_index(MouseAction::Pause, &SINGLE_CLICK_CHOICES), 1);
        assert_eq!(choice_index(MouseAction::HideOsc, &RIGHT_CLICK_CHOICES), 2);
        assert_eq!(
            choice(3, &FORCE_TOUCH_CHOICES, "Force Touch").unwrap(),
            MouseAction::HideOsc
        );
    }
}
