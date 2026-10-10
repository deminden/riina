//! Save deliberate Layout-pane choices without writing from every animation frame.
use crate::{PlayerWindow, platform, screenshot_preferences};
use riina_core::Preferences;
use slint::ComponentHandle;
use std::{cell::Cell, path::PathBuf, rc::Rc};

pub(crate) fn connect(app: &PlayerWindow, path: PathBuf, baseline: Preferences) {
    let saved = Rc::new(Cell::new((
        baseline.compact_ui,
        i32::from(baseline.osc_position),
    )));
    let weak = app.as_weak();
    app.on_layout_settings_changed(move || {
        let Some(app) = weak.upgrade() else { return };
        let selected = (app.get_compact_ui(), app.get_osc_position().clamp(0, 2));
        let result = screenshot_preferences::current(&app).and_then(|screenshot| {
            let preferences = Preferences {
                compact_ui: selected.0,
                osc_position: selected.1 as u8,
                leading_sidebar_width: app.get_leading_sidebar_width() as u32,
                trailing_sidebar_width: app.get_trailing_sidebar_width() as u32,
                volume: f64::from(app.get_volume()).min(if app.get_volume_boost() {
                    200.0
                } else {
                    100.0
                }),
                volume_boost: app.get_volume_boost(),
                pause_when_open: app.get_pause_when_open(),
                fullscreen_when_open: app.get_fullscreen_when_open(),
                screenshot,
                ..baseline.clone()
            };
            let text =
                serde_json::to_string_pretty(&preferences).map_err(|error| error.to_string())?;
            platform::save_preferences(&path, &text).map_err(|error| error.to_string())
        });
        if let Err(error) = result {
            let previous = saved.get();
            app.set_compact_ui(previous.0);
            app.set_osc_position(previous.1);
            app.set_status(error.into());
        } else {
            saved.set(selected);
        }
    });
}
