//! Typed construction and removal for the interactive logo-removal filter.
use crate::crop::Rectangle;

pub const FILTER_LABEL: &str = "riina-delogo";

pub fn rectangle_filter(
    rectangle: Rectangle,
    source_width: u32,
    source_height: u32,
) -> Result<Vec<Vec<String>>, String> {
    if rectangle.width == 0
        || rectangle.height == 0
        || rectangle.x.saturating_add(rectangle.width) > source_width
        || rectangle.y.saturating_add(rectangle.height) > source_height
    {
        return Err("The delogo selection is outside the source video".into());
    }
    Ok(vec![vec![
        "vf".into(),
        "add".into(),
        format!(
            "@{FILTER_LABEL}:lavfi=[delogo=x={}:y={}:w={}:h={}]",
            rectangle.x, rectangle.y, rectangle.width, rectangle.height
        ),
    ]])
}

pub fn remove_filter() -> Vec<Vec<String>> {
    vec![vec![
        "vf".into(),
        "remove".into(),
        format!("@{FILTER_LABEL}"),
    ]]
}
