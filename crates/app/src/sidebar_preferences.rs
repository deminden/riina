//! Persist a completed resize, never every intermediate drag event.
use crate::PlayerWindow;
use riina_core::Preferences;
use slint::ComponentHandle;
use std::{cell::Cell, path::PathBuf, rc::Rc};

pub(crate) fn show_plugins_panel(app: &PlayerWindow) {
    let (leading, trailing) = plugin_panel_destination(
        app.get_plugins_at_leading(),
        app.get_leading_sidebar(),
        app.get_sidebar(),
    );
    app.set_leading_sidebar(leading);
    app.set_sidebar(trailing);
}

fn plugin_panel_destination(
    plugins_at_leading: bool,
    leading_sidebar: i32,
    trailing_sidebar: i32,
) -> (i32, i32) {
    if plugins_at_leading {
        (7, trailing_sidebar)
    } else {
        (leading_sidebar, 7)
    }
}

pub(crate) fn connect(app: &PlayerWindow, path: PathBuf, baseline: Preferences) {
    let saved = Rc::new(Cell::new((
        baseline.leading_sidebar_width,
        baseline.trailing_sidebar_width,
    )));
    let weak = app.as_weak();
    let weak_settings = app.as_weak();
    app.on_toggle_settings_pane(move |kind| {
        let Some(app) = weak_settings.upgrade() else {
            return;
        };
        app.set_remembered_settings_pane(kind);
        let destination_leading = app.get_settings_at_leading();
        let current = if destination_leading {
            app.get_leading_sidebar()
        } else {
            app.get_sidebar()
        };
        let next = if current == kind { 0 } else { kind };
        if destination_leading {
            app.set_leading_sidebar(next);
        } else {
            app.set_sidebar(next);
        }
        app.invoke_activity();
    });
    let weak_settings = app.as_weak();
    app.on_select_settings_pane(move |kind| {
        let Some(app) = weak_settings.upgrade() else {
            return;
        };
        if app.get_settings_at_leading() {
            app.set_remembered_settings_pane(kind);
            app.set_leading_sidebar(kind);
        } else {
            app.set_remembered_settings_pane(kind);
            app.set_sidebar(kind);
        }
    });
    let weak_playlist = app.as_weak();
    app.on_toggle_playlist_pane(move |kind| {
        let Some(app) = weak_playlist.upgrade() else {
            return;
        };
        app.set_remembered_playlist_pane(kind);
        let destination_leading = app.get_playlist_at_leading();
        let current = if destination_leading {
            app.get_leading_sidebar()
        } else {
            app.get_sidebar()
        };
        let next = if current == kind { 0 } else { kind };
        if destination_leading {
            app.set_leading_sidebar(next);
        } else {
            app.set_sidebar(next);
        }
    });
    let weak_playlist = app.as_weak();
    app.on_select_playlist_pane(move |kind| {
        let Some(app) = weak_playlist.upgrade() else {
            return;
        };
        if app.get_playlist_at_leading() {
            app.set_remembered_playlist_pane(kind);
            app.set_leading_sidebar(kind);
        } else {
            app.set_remembered_playlist_pane(kind);
            app.set_sidebar(kind);
        }
    });
    let weak_plugins = app.as_weak();
    app.on_toggle_plugins_pane(move || {
        let Some(app) = weak_plugins.upgrade() else {
            return;
        };
        if app.get_leading_sidebar() == 7 {
            app.set_leading_sidebar(0);
        } else if app.get_sidebar() == 7 {
            app.set_sidebar(0);
        } else {
            show_plugins_panel(&app);
        }
    });
    app.on_sidebar_resize_finished(move || {
        let Some(app) = weak.upgrade() else { return };
        let widths = (
            (app.get_leading_sidebar_width() as u32).clamp(300, 500),
            (app.get_trailing_sidebar_width() as u32).clamp(300, 500),
        );
        let result = crate::preference_store::update(&path, &baseline, |preferences| {
            preferences.leading_sidebar_width = app.get_leading_sidebar_width() as u32;
            preferences.trailing_sidebar_width = app.get_trailing_sidebar_width() as u32;
            Ok(())
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

#[cfg(test)]
mod tests {
    use super::plugin_panel_destination;

    #[test]
    fn plugin_panel_opens_on_the_saved_side() {
        assert_eq!(plugin_panel_destination(true, 0, 2), (7, 2));
        assert_eq!(plugin_panel_destination(false, 3, 0), (3, 7));
    }
}
