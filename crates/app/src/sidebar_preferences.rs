//! Persist a completed resize, never every intermediate drag event.
use crate::{PlayerWindow, platform, screenshot_preferences};
use riina_core::Preferences;
use slint::ComponentHandle;
use std::{cell::Cell, path::PathBuf, rc::Rc};

pub(crate) fn connect(app: &PlayerWindow, path: PathBuf, baseline: Preferences) {
    let saved = Rc::new(Cell::new((
        baseline.leading_sidebar_width,
        baseline.trailing_sidebar_width,
    )));
    let weak = app.as_weak();
    app.on_sidebar_resize_finished(move || {
        let Some(app) = weak.upgrade() else { return };
        let widths = (
            (app.get_leading_sidebar_width() as u32).clamp(300, 500),
            (app.get_trailing_sidebar_width() as u32).clamp(300, 500),
        );
        let result = screenshot_preferences::current(&app).and_then(|screenshot| {
            // Other settings may have changed since this callback was installed.
            let preferences = Preferences {
                leading_sidebar_width: widths.0,
                trailing_sidebar_width: widths.1,
                screenshot,
                volume: f64::from(app.get_volume()).min(if app.get_volume_boost() {
                    200.0
                } else {
                    100.0
                }),
                volume_boost: app.get_volume_boost(),
                pause_when_open: app.get_pause_when_open(),
                fullscreen_when_open: app.get_fullscreen_when_open(),
                ..baseline.clone()
            };
            let text =
                serde_json::to_string_pretty(&preferences).map_err(|error| error.to_string())?;
            platform::save_preferences(&path, &text).map_err(|error| error.to_string())
        });
        if let Err(error) = result {
            let previous = saved.get();
            app.set_leading_sidebar_width(previous.0 as f32);
            app.set_trailing_sidebar_width(previous.1 as f32);
            app.set_status(error.into());
        } else {
            saved.set(widths);
        }
    });
}
