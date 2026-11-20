//! Map the reference slider's visible order to typed native option values.

use riina_core::subtitle_controls::AssOverrideLevel;

pub(crate) fn level(index: i32) -> Option<AssOverrideLevel> {
    AssOverrideLevel::from_slider_index(index)
}

pub(crate) const fn index(level: AssOverrideLevel) -> i32 {
    level.slider_index()
}

pub(crate) fn commands(
    style: riina_core::subtitle_controls::TextStylePreferences,
) -> [riina_core::Command; 9] {
    use riina_core::Command;

    [
        Command::SubtitleTextColor(style.text_color),
        Command::SubtitleBold(style.bold),
        Command::SubtitleItalic(style.italic),
        Command::SubtitleBlur(style.blur),
        Command::SubtitleSpacing(style.spacing),
        Command::SubtitleOutlineSize(style.outline_size),
        Command::SubtitleBorderColor(style.outline_color),
        Command::SubtitleShadowOffset(style.shadow_offset),
        Command::SubtitleShadowColor(style.shadow_color),
    ]
}

pub(crate) fn font_size(text: &str) -> Result<f64, &'static str> {
    let size = text
        .trim()
        .parse::<f64>()
        .map_err(|_| "Enter a numeric subtitle size")?;
    if riina_core::subtitle_controls::valid_preference_font_size(size) {
        Ok(round_source_precision(size))
    } else {
        Err("Subtitle size must be finite and positive")
    }
}

fn round_source_precision(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

pub(crate) fn format_number(value: f32) -> slint::SharedString {
    let rounded = round_source_precision(f64::from(value));
    let mut text = format!("{rounded:.3}");
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text.into()
}

pub(crate) fn current_font_size(app: &crate::PlayerWindow) -> f64 {
    round_source_precision(f64::from(app.get_subtitle_font_size_preference()))
}

pub(crate) fn style_color_to_ui(color: riina_core::subtitle_color::SubtitleColor) -> slint::Color {
    let channels = color.rgba().map(|channel| (channel * 255.0).round() as u8);
    slint::Color::from_argb_u8(channels[3], channels[0], channels[1], channels[2])
}

fn style_color_from_ui(
    color: slint::Color,
) -> Result<riina_core::subtitle_color::SubtitleColor, String> {
    let channels = [color.red(), color.green(), color.blue(), color.alpha()]
        .map(|channel| f64::from(channel) / 255.0);
    riina_core::subtitle_color::SubtitleColor::from_rgba(channels).map_err(str::to_owned)
}

pub(crate) fn install_style(
    app: &crate::PlayerWindow,
    style: riina_core::subtitle_controls::TextStylePreferences,
) {
    app.set_subtitle_text_preference_color(style_color_to_ui(style.text_color));
    app.set_subtitle_bold_preference(style.bold);
    app.set_subtitle_italic_preference(style.italic);
    app.set_subtitle_background_preference_color(style_color_to_ui(style.shadow_color));
    app.set_subtitle_outline_size_preference(style.outline_size as f32);
    app.set_subtitle_outline_preference_color(style_color_to_ui(style.outline_color));
    app.set_subtitle_shadow_offset_preference(style.shadow_offset as f32);
    app.set_subtitle_shadow_preference_color(style_color_to_ui(style.shadow_color));
    app.set_subtitle_blur_preference(style.blur as f32);
    app.set_subtitle_spacing_preference(style.spacing as f32);
}

pub(crate) fn current_style(
    app: &crate::PlayerWindow,
) -> Result<riina_core::subtitle_controls::TextStylePreferences, String> {
    let style = riina_core::subtitle_controls::TextStylePreferences {
        text_color: style_color_from_ui(app.get_subtitle_text_preference_color())?,
        bold: app.get_subtitle_bold_preference(),
        italic: app.get_subtitle_italic_preference(),
        blur: round_source_precision(f64::from(app.get_subtitle_blur_preference())),
        spacing: round_source_precision(f64::from(app.get_subtitle_spacing_preference())),
        outline_size: round_source_precision(f64::from(app.get_subtitle_outline_size_preference())),
        outline_color: style_color_from_ui(app.get_subtitle_outline_preference_color())?,
        shadow_offset: round_source_precision(f64::from(
            app.get_subtitle_shadow_offset_preference(),
        )),
        shadow_color: style_color_from_ui(app.get_subtitle_shadow_preference_color())?,
    };
    style.validate().map_err(str::to_owned)?;
    Ok(style)
}

pub(crate) fn current_window_style(
    window: &crate::PreferencesWindow,
) -> Result<riina_core::subtitle_controls::TextStylePreferences, String> {
    let parse = |text: slint::SharedString| {
        text.trim()
            .parse::<f64>()
            .map(round_source_precision)
            .map_err(|_| "Enter a numeric subtitle style value".to_owned())
    };
    let style = riina_core::subtitle_controls::TextStylePreferences {
        text_color: style_color_from_ui(window.get_subtitle_text_color())?,
        bold: window.get_subtitle_bold(),
        italic: window.get_subtitle_italic(),
        blur: parse(window.get_subtitle_blur_text())?,
        spacing: parse(window.get_subtitle_spacing_text())?,
        outline_size: parse(window.get_subtitle_outline_size_text())?,
        outline_color: style_color_from_ui(window.get_subtitle_outline_color())?,
        shadow_offset: parse(window.get_subtitle_shadow_offset_text())?,
        shadow_color: style_color_from_ui(window.get_subtitle_shadow_color())?,
    };
    style.validate().map_err(str::to_owned)?;
    Ok(style)
}

pub(crate) fn install_window_style(
    window: &crate::PreferencesWindow,
    style: riina_core::subtitle_controls::TextStylePreferences,
) {
    window.set_subtitle_text_color(style_color_to_ui(style.text_color));
    window.set_subtitle_bold(style.bold);
    window.set_subtitle_italic(style.italic);
    window.set_subtitle_background_color(style_color_to_ui(style.shadow_color));
    window.set_subtitle_outline_size_text(format_number(style.outline_size as f32));
    window.set_subtitle_outline_color(style_color_to_ui(style.outline_color));
    window.set_subtitle_shadow_offset_text(format_number(style.shadow_offset as f32));
    window.set_subtitle_shadow_color(style_color_to_ui(style.shadow_color));
    window.set_subtitle_blur_text(format_number(style.blur as f32));
    window.set_subtitle_spacing_text(format_number(style.spacing as f32));
}

#[cfg(test)]
mod tests {
    use super::{font_size, format_number};

    #[test]
    fn subtitle_font_size_entry_preserves_valid_fractional_values() {
        for (text, expected) in [("55", 55.0), (" 25.125 ", 25.125), ("1e3", 1000.0)] {
            assert_eq!(font_size(text), Ok(expected));
        }
        for text in ["", "NaN", "inf", "-1", "2.1e9", "large"] {
            assert!(font_size(text).is_err(), "{text}");
        }
    }

    #[test]
    fn numeric_preferences_follow_three_fractional_digit_source_format() {
        assert_eq!(font_size("25.1234"), Ok(25.123));
        assert_eq!(format_number(42.123_f32).as_str(), "42.123");
        assert_eq!(format_number(55.0_f32).as_str(), "55");
        assert_eq!(format_number(0.125_f32).as_str(), "0.125");
    }
}
