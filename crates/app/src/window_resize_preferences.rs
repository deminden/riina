//! Keep the Interface > Window controls and shared resize policy in one mapping.
use crate::{PlayerWindow, PreferencesWindow};
use riina_core::window_resize::{
    GeometryUnit, HorizontalAnchor, InitialGeometry, InitialPosition, InitialSize, ResizeOption,
    ResizeTiming, Settings, SizeAxis, VerticalAnchor,
};

pub(crate) fn from_player(app: &PlayerWindow) -> Result<Settings, String> {
    Ok(Settings {
        option: ResizeOption::from_index(app.get_window_resize_option_index())
            .ok_or("Choose a valid window resize option")?,
        timing: ResizeTiming::from_index(app.get_window_resize_timing_index())
            .ok_or("Choose a valid window resize timing")?,
        use_physical_resolution: app.get_use_physical_resolution(),
        initial_geometry: geometry_from_player(app)?,
    })
}

pub(crate) fn from_preferences(window: &PreferencesWindow) -> Result<Settings, String> {
    Ok(Settings {
        option: ResizeOption::from_index(window.get_window_resize_option_index())
            .ok_or("Choose a valid window resize option")?,
        timing: ResizeTiming::from_index(window.get_window_resize_timing_index())
            .ok_or("Choose a valid window resize timing")?,
        use_physical_resolution: window.get_use_physical_resolution(),
        initial_geometry: geometry_from_preferences(window)?,
    })
}

pub(crate) fn install_player(app: &PlayerWindow, settings: Settings) {
    app.set_window_resize_option_index(settings.option.index());
    app.set_window_resize_timing_index(settings.timing.index());
    app.set_use_physical_resolution(settings.use_physical_resolution);
    app.set_initial_window_size_enabled(settings.initial_geometry.size.is_some());
    app.set_initial_window_position_enabled(settings.initial_geometry.position.is_some());
    if let Some(size) = settings.initial_geometry.size {
        app.set_initial_window_size_axis_index(match size.axis {
            SizeAxis::Width => 0,
            SizeAxis::Height => 1,
        });
        app.set_initial_window_size_value_text(size.value.to_string().into());
        app.set_initial_window_size_unit_index(unit_index(size.unit));
    }
    if let Some(position) = settings.initial_geometry.position {
        app.set_initial_window_x_anchor_index(match position.horizontal_anchor {
            HorizontalAnchor::Left => 0,
            HorizontalAnchor::Right => 1,
        });
        app.set_initial_window_x_offset_text(position.horizontal_offset.to_string().into());
        app.set_initial_window_x_unit_index(unit_index(position.horizontal_unit));
        app.set_initial_window_y_anchor_index(match position.vertical_anchor {
            VerticalAnchor::Bottom => 0,
            VerticalAnchor::Top => 1,
        });
        app.set_initial_window_y_offset_text(position.vertical_offset.to_string().into());
        app.set_initial_window_y_unit_index(unit_index(position.vertical_unit));
    }
}

pub(crate) fn install_preferences(window: &PreferencesWindow, settings: Settings) {
    window.set_window_resize_option_index(settings.option.index());
    window.set_window_resize_timing_index(settings.timing.index());
    window.set_use_physical_resolution(settings.use_physical_resolution);
    window.set_initial_window_size_enabled(settings.initial_geometry.size.is_some());
    window.set_initial_window_position_enabled(settings.initial_geometry.position.is_some());
    if let Some(size) = settings.initial_geometry.size {
        window.set_initial_window_size_axis_index(match size.axis {
            SizeAxis::Width => 0,
            SizeAxis::Height => 1,
        });
        window.set_initial_window_size_value_text(size.value.to_string().into());
        window.set_initial_window_size_unit_index(unit_index(size.unit));
    }
    if let Some(position) = settings.initial_geometry.position {
        window.set_initial_window_x_anchor_index(match position.horizontal_anchor {
            HorizontalAnchor::Left => 0,
            HorizontalAnchor::Right => 1,
        });
        window.set_initial_window_x_offset_text(position.horizontal_offset.to_string().into());
        window.set_initial_window_x_unit_index(unit_index(position.horizontal_unit));
        window.set_initial_window_y_anchor_index(match position.vertical_anchor {
            VerticalAnchor::Bottom => 0,
            VerticalAnchor::Top => 1,
        });
        window.set_initial_window_y_offset_text(position.vertical_offset.to_string().into());
        window.set_initial_window_y_unit_index(unit_index(position.vertical_unit));
    }
}

fn geometry_from_player(app: &PlayerWindow) -> Result<InitialGeometry, String> {
    geometry(
        app.get_initial_window_size_enabled(),
        app.get_initial_window_size_axis_index(),
        app.get_initial_window_size_value_text().as_str(),
        app.get_initial_window_size_unit_index(),
        app.get_initial_window_position_enabled(),
        app.get_initial_window_x_anchor_index(),
        app.get_initial_window_x_offset_text().as_str(),
        app.get_initial_window_x_unit_index(),
        app.get_initial_window_y_anchor_index(),
        app.get_initial_window_y_offset_text().as_str(),
        app.get_initial_window_y_unit_index(),
    )
}

fn geometry_from_preferences(window: &PreferencesWindow) -> Result<InitialGeometry, String> {
    geometry(
        window.get_initial_window_size_enabled(),
        window.get_initial_window_size_axis_index(),
        window.get_initial_window_size_value_text().as_str(),
        window.get_initial_window_size_unit_index(),
        window.get_initial_window_position_enabled(),
        window.get_initial_window_x_anchor_index(),
        window.get_initial_window_x_offset_text().as_str(),
        window.get_initial_window_x_unit_index(),
        window.get_initial_window_y_anchor_index(),
        window.get_initial_window_y_offset_text().as_str(),
        window.get_initial_window_y_unit_index(),
    )
}

#[allow(clippy::too_many_arguments)]
fn geometry(
    size_enabled: bool,
    size_axis: i32,
    size_value: &str,
    size_unit: i32,
    position_enabled: bool,
    x_anchor: i32,
    x_offset: &str,
    x_unit: i32,
    y_anchor: i32,
    y_offset: &str,
    y_unit: i32,
) -> Result<InitialGeometry, String> {
    let size = if size_enabled {
        Some(InitialSize {
            axis: match size_axis {
                0 => SizeAxis::Width,
                1 => SizeAxis::Height,
                _ => return Err("Choose width or height for the initial window size".into()),
            },
            value: geometry_value(size_value, size_unit, "initial window size")?,
            unit: geometry_unit(size_unit)?,
        })
    } else {
        None
    };
    let position = if position_enabled {
        Some(InitialPosition {
            horizontal_anchor: match x_anchor {
                0 => HorizontalAnchor::Left,
                1 => HorizontalAnchor::Right,
                _ => return Err("Choose the left or right screen edge".into()),
            },
            horizontal_offset: geometry_value(x_offset, x_unit, "horizontal window offset")?,
            horizontal_unit: geometry_unit(x_unit)?,
            vertical_anchor: match y_anchor {
                0 => VerticalAnchor::Bottom,
                1 => VerticalAnchor::Top,
                _ => return Err("Choose the top or bottom screen edge".into()),
            },
            vertical_offset: geometry_value(y_offset, y_unit, "vertical window offset")?,
            vertical_unit: geometry_unit(y_unit)?,
        })
    } else {
        None
    };
    Ok(InitialGeometry { size, position })
}

fn geometry_value(value: &str, unit_index: i32, label: &str) -> Result<u32, String> {
    let value = value
        .trim()
        .parse::<u32>()
        .map_err(|_| format!("Enter a whole number for {label}"))?;
    if value == 0 || (geometry_unit(unit_index)? == GeometryUnit::ScreenPercent && value > 100) {
        return Err(format!(
            "{label} must be between 1 and 100% or a positive point value"
        ));
    }
    Ok(value)
}

fn geometry_unit(index: i32) -> Result<GeometryUnit, String> {
    match index {
        0 => Ok(GeometryUnit::Points),
        1 => Ok(GeometryUnit::ScreenPercent),
        _ => Err("Choose points or percent of screen".into()),
    }
}

const fn unit_index(unit: GeometryUnit) -> i32 {
    match unit {
        GeometryUnit::Points => 0,
        GeometryUnit::ScreenPercent => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::geometry;

    #[test]
    fn disabled_geometry_does_not_validate_its_inactive_draft_fields() {
        let value = geometry(
            false,
            -1,
            "not a number",
            -1,
            false,
            -1,
            "x",
            -1,
            -1,
            "y",
            -1,
        )
        .unwrap();
        assert_eq!(value, riina_core::window_resize::InitialGeometry::default());
    }

    #[test]
    fn enabled_geometry_rejects_non_integer_and_out_of_range_percentages() {
        assert!(geometry(true, 0, "1.5", 0, false, 0, "20", 0, 1, "20", 0).is_err());
        assert!(geometry(true, 0, "101", 1, false, 0, "20", 0, 1, "20", 0).is_err());
        assert!(geometry(false, 0, "1280", 0, true, 0, "20", 0, 1, "101", 1).is_err());
    }
}
