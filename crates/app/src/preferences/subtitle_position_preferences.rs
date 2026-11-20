//! Translate editable Preferences values into portable subtitle placement intent.

use riina_core::subtitle_controls::{HorizontalAlignment, PositionPreferences, VerticalAlignment};

fn horizontal(index: i32) -> Option<HorizontalAlignment> {
    match index {
        0 => Some(HorizontalAlignment::Left),
        1 => Some(HorizontalAlignment::Center),
        2 => Some(HorizontalAlignment::Right),
        _ => None,
    }
}

fn vertical(index: i32) -> Option<VerticalAlignment> {
    match index {
        0 => Some(VerticalAlignment::Top),
        1 => Some(VerticalAlignment::Center),
        2 => Some(VerticalAlignment::Bottom),
        _ => None,
    }
}

fn number(text: &str, field: &str) -> Result<f64, String> {
    let value = text
        .trim()
        .parse::<f64>()
        .map_err(|_| format!("Enter a numeric {field}"))?;
    if !value.is_finite() {
        return Err(format!("Enter a finite {field}"));
    }
    Ok(value)
}

pub(crate) fn current_player(app: &crate::PlayerWindow) -> Result<PositionPreferences, String> {
    let preferences = PositionPreferences {
        horizontal: horizontal(app.get_subtitle_align_x_index())
            .ok_or_else(|| "Invalid horizontal subtitle alignment".to_owned())?,
        vertical: vertical(app.get_subtitle_align_y_index())
            .ok_or_else(|| "Invalid vertical subtitle alignment".to_owned())?,
        position: number(
            app.get_subtitle_position_text().as_str(),
            "subtitle position",
        )?,
        display_in_letterbox: app.get_subtitle_display_in_letterbox(),
        scale_with_window: app.get_subtitle_scale_with_window(),
        margin_x: number(
            app.get_subtitle_margin_x_text().as_str(),
            "horizontal subtitle margin",
        )?,
        margin_y: number(
            app.get_subtitle_margin_y_text().as_str(),
            "vertical subtitle margin",
        )?,
    };
    preferences.validate().map_err(str::to_owned)?;
    Ok(preferences)
}

pub(crate) fn current_window(
    window: &crate::PreferencesWindow,
) -> Result<PositionPreferences, String> {
    let preferences = PositionPreferences {
        horizontal: horizontal(window.get_subtitle_align_x_index())
            .ok_or_else(|| "Invalid horizontal subtitle alignment".to_owned())?,
        vertical: vertical(window.get_subtitle_align_y_index())
            .ok_or_else(|| "Invalid vertical subtitle alignment".to_owned())?,
        position: number(
            window.get_subtitle_position_text().as_str(),
            "subtitle position",
        )?,
        display_in_letterbox: window.get_subtitle_display_in_letterbox(),
        scale_with_window: window.get_subtitle_scale_with_window(),
        margin_x: number(
            window.get_subtitle_margin_x_text().as_str(),
            "horizontal subtitle margin",
        )?,
        margin_y: number(
            window.get_subtitle_margin_y_text().as_str(),
            "vertical subtitle margin",
        )?,
    };
    preferences.validate().map_err(str::to_owned)?;
    Ok(preferences)
}

pub(crate) fn install_player(app: &crate::PlayerWindow, preferences: PositionPreferences) {
    app.set_subtitle_align_x_index(match preferences.horizontal {
        HorizontalAlignment::Left => 0,
        HorizontalAlignment::Center => 1,
        HorizontalAlignment::Right => 2,
    });
    app.set_subtitle_align_y_index(match preferences.vertical {
        VerticalAlignment::Top => 0,
        VerticalAlignment::Center => 1,
        VerticalAlignment::Bottom => 2,
    });
    app.set_subtitle_position_text(format_number(preferences.position));
    app.set_subtitle_display_in_letterbox(preferences.display_in_letterbox);
    app.set_subtitle_scale_with_window(preferences.scale_with_window);
    app.set_subtitle_margin_x_text(format_number(preferences.margin_x));
    app.set_subtitle_margin_y_text(format_number(preferences.margin_y));
}

pub(crate) fn install_window(window: &crate::PreferencesWindow, preferences: PositionPreferences) {
    window.set_subtitle_align_x_index(match preferences.horizontal {
        HorizontalAlignment::Left => 0,
        HorizontalAlignment::Center => 1,
        HorizontalAlignment::Right => 2,
    });
    window.set_subtitle_align_y_index(match preferences.vertical {
        VerticalAlignment::Top => 0,
        VerticalAlignment::Center => 1,
        VerticalAlignment::Bottom => 2,
    });
    window.set_subtitle_position_text(format_number(preferences.position));
    window.set_subtitle_display_in_letterbox(preferences.display_in_letterbox);
    window.set_subtitle_scale_with_window(preferences.scale_with_window);
    window.set_subtitle_margin_x_text(format_number(preferences.margin_x));
    window.set_subtitle_margin_y_text(format_number(preferences.margin_y));
}

fn format_number(value: f64) -> slint::SharedString {
    let mut text = format!("{value:.3}");
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text.into()
}

pub(crate) fn apply(
    player: &riina_mpv::CommandSender,
    previous: PositionPreferences,
    selected: PositionPreferences,
) -> Result<(), String> {
    selected.validate().map_err(str::to_owned)?;
    let old = previous.commands();
    let new = selected.commands();
    let mut applied = Vec::<usize>::new();
    for (index, (old, new)) in old.into_iter().zip(new).enumerate() {
        if old == new {
            continue;
        }
        if let Err(error) = player.send(new) {
            let rollback = previous.commands();
            for index in applied.into_iter().rev() {
                let _ = player.send(rollback[index].clone());
            }
            return Err(error.to_string());
        }
        applied.push(index);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{format_number, horizontal, number, vertical};
    use riina_core::subtitle_controls::{HorizontalAlignment, VerticalAlignment};

    #[test]
    fn alignment_indices_follow_reference_menu_order() {
        assert_eq!(horizontal(0), Some(HorizontalAlignment::Left));
        assert_eq!(horizontal(1), Some(HorizontalAlignment::Center));
        assert_eq!(horizontal(2), Some(HorizontalAlignment::Right));
        assert_eq!(vertical(0), Some(VerticalAlignment::Top));
        assert_eq!(vertical(1), Some(VerticalAlignment::Center));
        assert_eq!(vertical(2), Some(VerticalAlignment::Bottom));
        assert_eq!(horizontal(-1), None);
        assert_eq!(vertical(3), None);
    }

    #[test]
    fn numeric_fields_retain_reference_precision_and_reject_invalid_input() {
        assert_eq!(number(" 22.125 ", "margin"), Ok(22.125));
        assert_eq!(format_number(25.0).as_str(), "25");
        assert_eq!(format_number(22.125).as_str(), "22.125");
        assert!(number("NaN", "margin").is_err());
        assert!(number("1e999", "margin").is_err());
        assert!(number("not a number", "margin").is_err());
    }
}
