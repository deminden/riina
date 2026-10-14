//! Persist a completed resize, never every intermediate drag event.
use crate::{PlayerWindow, platform, screenshot_preferences};
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
        let result = screenshot_preferences::current(&app).and_then(|screenshot| {
            // Other settings may have changed since this callback was installed.
            let mut preferences = Preferences {
                leading_sidebar_width: widths.0,
                trailing_sidebar_width: widths.1,
                settings_at_leading: app.get_settings_at_leading(),
                playlist_at_leading: app.get_playlist_at_leading(),
                plugins_at_leading: app.get_plugins_at_leading(),
                edge_to_edge_video: app.get_edge_to_edge_video(),
                docked_controls: app.get_docked_controls(),
                unlock_window_aspect_ratio: app.get_unlock_window_aspect_ratio(),
                window_resize: crate::window_resize_preferences::from_player(&app)?,
                compact_ui: app.get_compact_ui(),
                osc_position: app.get_osc_position() as u8,
                control_bar_position_horizontal: f64::from(
                    app.get_control_bar_position_horizontal(),
                ),
                control_bar_position_vertical: f64::from(app.get_control_bar_position_vertical()),
                screenshot,
                volume: f64::from(app.get_volume()).min(if app.get_volume_boost() {
                    200.0
                } else {
                    100.0
                }),
                volume_boost: app.get_volume_boost(),
                enable_osd: app.get_osd_enabled(),
                disable_osd_file_start: app.get_disable_osd_file_start(),
                disable_osd_pause_resume: app.get_disable_osd_pause_resume(),
                disable_osd_speed: app.get_disable_osd_speed(),
                disable_osd_seek: app.get_disable_osd_seek(),
                osd_timeout_secs: f64::from(app.get_osd_timeout_secs()),
                osd_text_size: f64::from(app.get_osd_text_size()),
                control_bar: crate::osc_preferences::current_player(&app),
                display_time_and_battery_in_fullscreen: app
                    .get_display_time_and_battery_in_fullscreen(),
                liquid_glass: crate::liquid_glass_preferences::current_player(&app),
                hardware_decoder: crate::codec_preferences::hardware_decoder(
                    app.get_hardware_decoder_index(),
                )
                .ok_or("Invalid hardware decoder selection")?,
                subtitle_ass_override: crate::subtitle_preferences::level(
                    app.get_subtitle_ass_override_index(),
                )
                .ok_or("Invalid ASS override selection")?,
                secondary_subtitle_ass_override: crate::subtitle_preferences::level(
                    app.get_secondary_subtitle_ass_override_index(),
                )
                .ok_or("Invalid secondary ASS override selection")?,
                subtitle_font: app.get_subtitle_font().to_string(),
                subtitle_font_size: crate::subtitle_preferences::current_font_size(&app),
                subtitle_text_style: crate::subtitle_preferences::current_style(&app)?,
                subtitle_other: crate::subtitle_other_preferences::current_player(&app)?,
                subtitle_auto_load: crate::subtitle_auto_load_preferences::current_player(&app)?,
                pause_when_open: app.get_pause_when_open(),
                fullscreen_when_open: app.get_fullscreen_when_open(),
                ..baseline.clone()
            };
            crate::utilities_preferences::preserve_saved_history(&mut preferences, &path);
            crate::advanced_preferences::preserve_latest(&mut preferences, &path);
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

#[cfg(test)]
mod tests {
    use super::plugin_panel_destination;

    #[test]
    fn plugin_panel_opens_on_the_saved_side() {
        assert_eq!(plugin_panel_destination(true, 0, 2), (7, 2));
        assert_eq!(plugin_panel_destination(false, 3, 0), (3, 7));
    }
}
