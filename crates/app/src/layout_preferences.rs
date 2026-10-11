//! Save deliberate Layout-pane choices without writing from every animation frame.
use crate::{PlayerWindow, platform, screenshot_preferences};
use riina_core::Preferences;
use slint::ComponentHandle;
use std::{cell::Cell, path::PathBuf, rc::Rc};

pub(crate) fn connect(app: &PlayerWindow, path: PathBuf, baseline: Preferences) {
    let saved = Rc::new(Cell::new((
        baseline.compact_ui,
        i32::from(baseline.osc_position),
        baseline.settings_at_leading,
        baseline.playlist_at_leading,
        baseline.edge_to_edge_video,
        baseline.docked_controls,
        baseline.unlock_window_aspect_ratio,
    )));
    let weak = app.as_weak();
    let path_for_window_level = path.clone();
    let baseline_for_window_level = baseline.clone();
    app.on_layout_settings_changed(move || {
        let Some(app) = weak.upgrade() else { return };
        let selected = (
            app.get_compact_ui(),
            app.get_osc_position().clamp(0, 2),
            app.get_settings_at_leading(),
            app.get_playlist_at_leading(),
            app.get_edge_to_edge_video(),
            app.get_docked_controls(),
            app.get_unlock_window_aspect_ratio(),
        );
        let previous = saved.get();
        let result = screenshot_preferences::current(&app).and_then(|screenshot| {
            let mut preferences = Preferences {
                compact_ui: selected.0,
                osc_position: selected.1 as u8,
                settings_at_leading: selected.2,
                playlist_at_leading: selected.3,
                edge_to_edge_video: selected.4,
                docked_controls: selected.5,
                unlock_window_aspect_ratio: selected.6,
                window_resize: crate::window_resize_preferences::from_player(&app)?,
                always_float_on_top: app.get_always_float_on_top(),
                always_show_on_top_icon: app.get_always_show_on_top_icon(),
                leading_sidebar_width: app.get_leading_sidebar_width() as u32,
                trailing_sidebar_width: app.get_trailing_sidebar_width() as u32,
                volume: f64::from(app.get_volume()).min(if app.get_volume_boost() {
                    200.0
                } else {
                    100.0
                }),
                volume_boost: app.get_volume_boost(),
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
                screenshot,
                enable_osd: app.get_osd_enabled(),
                disable_osd_file_start: app.get_disable_osd_file_start(),
                disable_osd_pause_resume: app.get_disable_osd_pause_resume(),
                disable_osd_speed: app.get_disable_osd_speed(),
                disable_osd_seek: app.get_disable_osd_seek(),
                osd_timeout_secs: f64::from(app.get_osd_timeout_secs()),
                osd_text_size: f64::from(app.get_osd_text_size()),
                display_time_and_battery_in_fullscreen: app
                    .get_display_time_and_battery_in_fullscreen(),
                liquid_glass: crate::liquid_glass_preferences::current_player(&app),
                ..baseline.clone()
            };
            crate::utilities_preferences::preserve_saved_history(&mut preferences, &path);
            crate::advanced_preferences::preserve_latest(&mut preferences, &path);
            let text =
                serde_json::to_string_pretty(&preferences).map_err(|error| error.to_string())?;
            platform::save_preferences(&path, &text).map_err(|error| error.to_string())
        });
        if let Err(error) = result {
            app.set_compact_ui(previous.0);
            app.set_osc_position(previous.1);
            app.set_settings_at_leading(previous.2);
            app.set_playlist_at_leading(previous.3);
            app.set_edge_to_edge_video(previous.4);
            app.set_docked_controls(previous.5);
            app.set_unlock_window_aspect_ratio(previous.6);
            app.set_status(error.into());
        } else {
            // IINA relocates a changed pane and dismisses other open sidebars.
            // Store one pane per physical side, so a same-side collision replaces it.
            if selected.2 != previous.2 {
                let kind = if previous.2 {
                    app.get_leading_sidebar()
                } else {
                    app.get_sidebar()
                };
                if (1..=2).contains(&kind) || kind == 5 || kind == 6 {
                    app.set_leading_sidebar(0);
                    app.set_sidebar(0);
                    if selected.2 {
                        app.set_leading_sidebar(kind);
                    } else {
                        app.set_sidebar(kind);
                    }
                }
            } else if selected.3 != previous.3 {
                let kind = if previous.3 {
                    app.get_leading_sidebar()
                } else {
                    app.get_sidebar()
                };
                if kind == 3 || kind == 4 {
                    app.set_leading_sidebar(0);
                    app.set_sidebar(0);
                    if selected.3 {
                        app.set_leading_sidebar(kind);
                    } else {
                        app.set_sidebar(kind);
                    }
                }
            }
            saved.set(selected);
        }
    });

    let weak = app.as_weak();
    app.on_window_level_toggle_requested(move |requested| {
        let Some(app) = weak.upgrade() else { return };
        let stored = match std::fs::read_to_string(&path_for_window_level) {
            Ok(text) => Preferences::parse(&text).map_err(std::io::Error::other),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(baseline_for_window_level.clone())
            }
            Err(error) => Err(error),
        };
        let result = stored.and_then(|mut preferences| {
            preferences.always_float_on_top = requested;
            let text = serde_json::to_string_pretty(&preferences).map_err(std::io::Error::other)?;
            platform::save_preferences(&path_for_window_level, &text)
        });
        if let Err(error) = result {
            // The UI optimistically updates the native window; restore its last
            // saved level if atomic preference persistence fails.
            app.set_always_float_on_top(!requested);
            app.set_status(error.to_string().into());
        }
    });
}
