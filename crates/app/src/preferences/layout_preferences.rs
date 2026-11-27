//! Save deliberate Layout-pane choices without writing from every animation frame.
use crate::PlayerWindow;
use riina_core::{Preferences, ThemeMaterial};
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
        baseline.theme_material.index(),
        baseline.liquid_glass.osd,
        baseline.liquid_glass.osc,
        baseline.liquid_glass.sidebar,
        baseline.control_bar_position_horizontal as f32,
        baseline.control_bar_position_vertical as f32,
        baseline.plugins_at_leading,
    )));
    let weak = app.as_weak();
    let path_for_window_level = path.clone();
    let baseline_for_window_level = baseline.clone();
    app.on_layout_settings_changed(move || {
        let Some(app) = weak.upgrade() else { return };
        let appearance = app.global::<riina_ui::PlayerAppearance>();
        let layout = app.global::<riina_ui::PlayerLayoutState>();
        let sidebar = app.global::<riina_ui::PlayerSidebarState>();
        let selected = (
            appearance.get_compact_ui(),
            layout.get_osc_position().clamp(0, 2),
            sidebar.get_settings_at_leading(),
            sidebar.get_playlist_at_leading(),
            layout.get_edge_to_edge_video(),
            layout.get_docked_controls(),
            layout.get_unlock_window_aspect_ratio(),
            appearance.get_appearance_mode(),
            appearance.get_liquid_glass_osd(),
            appearance.get_liquid_glass_osc(),
            appearance.get_liquid_glass_sidebar(),
            layout.get_control_bar_position_horizontal(),
            layout.get_control_bar_position_vertical(),
            sidebar.get_plugins_at_leading(),
        );
        let previous = saved.get();
        let result = crate::preference_store::update(&path, &baseline, |preferences| {
            preferences.compact_ui = selected.0;
            preferences.osc_position = selected.1 as u8;
            preferences.control_bar_position_horizontal = f64::from(selected.11);
            preferences.control_bar_position_vertical = f64::from(selected.12);
            preferences.settings_at_leading = selected.2;
            preferences.playlist_at_leading = selected.3;
            preferences.plugins_at_leading = selected.13;
            preferences.edge_to_edge_video = selected.4;
            preferences.docked_controls = selected.5;
            preferences.unlock_window_aspect_ratio = selected.6;
            preferences.theme_material =
                ThemeMaterial::from_index(selected.7).ok_or("Invalid theme selection")?;
            preferences.window_resize = crate::window_resize_preferences::from_player(&app)?;
            preferences.always_float_on_top = layout.get_always_float_on_top();
            preferences.always_show_on_top_icon = layout.get_always_show_on_top_icon();
            preferences.leading_sidebar_width = sidebar.get_leading_sidebar_width() as u32;
            preferences.trailing_sidebar_width = sidebar.get_trailing_sidebar_width() as u32;
            let controller = preferences.control_bar.controller;
            preferences.control_bar = riina_core::osc_preferences::Settings {
                controller,
                ..crate::osc_preferences::current_player(&app)
            };
            preferences.liquid_glass = crate::liquid_glass_preferences::current_player(&app);
            Ok(())
        });
        if let Err(error) = result {
            appearance.set_compact_ui(previous.0);
            layout.set_osc_position(previous.1);
            sidebar.set_settings_at_leading(previous.2);
            sidebar.set_playlist_at_leading(previous.3);
            sidebar.set_plugins_at_leading(previous.13);
            layout.set_edge_to_edge_video(previous.4);
            layout.set_docked_controls(previous.5);
            layout.set_unlock_window_aspect_ratio(previous.6);
            appearance.set_appearance_mode(previous.7);
            appearance.set_liquid_glass_osd(previous.8);
            appearance.set_liquid_glass_osc(previous.9);
            appearance.set_liquid_glass_sidebar(previous.10);
            layout.set_control_bar_position_horizontal(previous.11);
            layout.set_control_bar_position_vertical(previous.12);
            app.set_status(error.into());
        } else {
            // IINA relocates a changed pane and dismisses other open sidebars.
            // Store one pane per physical side, so a same-side collision replaces it.
            if selected.2 != previous.2 {
                let kind = if previous.2 {
                    sidebar.get_leading_sidebar()
                } else {
                    sidebar.get_sidebar()
                };
                if (1..=2).contains(&kind) || kind == 5 || kind == 6 {
                    sidebar.set_leading_sidebar(0);
                    sidebar.set_sidebar(0);
                    if selected.2 {
                        sidebar.set_leading_sidebar(kind);
                    } else {
                        sidebar.set_sidebar(kind);
                    }
                }
            } else if selected.3 != previous.3 {
                let kind = if previous.3 {
                    sidebar.get_leading_sidebar()
                } else {
                    sidebar.get_sidebar()
                };
                if kind == 3 || kind == 4 {
                    sidebar.set_leading_sidebar(0);
                    sidebar.set_sidebar(0);
                    if selected.3 {
                        sidebar.set_leading_sidebar(kind);
                    } else {
                        sidebar.set_sidebar(kind);
                    }
                }
            } else if selected.13 != previous.13 {
                let kind = if previous.13 {
                    sidebar.get_leading_sidebar()
                } else {
                    sidebar.get_sidebar()
                };
                if kind == 7 {
                    sidebar.set_leading_sidebar(0);
                    sidebar.set_sidebar(0);
                    if selected.13 {
                        sidebar.set_leading_sidebar(kind);
                    } else {
                        sidebar.set_sidebar(kind);
                    }
                }
            }
            saved.set(selected);
        }
    });

    connect_window_level(app, path_for_window_level, baseline_for_window_level);
}

fn connect_window_level(app: &PlayerWindow, path: PathBuf, baseline: Preferences) {
    let weak = app.as_weak();
    app.on_window_level_toggle_requested(move |requested| {
        let Some(app) = weak.upgrade() else { return };
        let result = crate::preference_store::update(&path, &baseline, |preferences| {
            preferences.always_float_on_top = requested;
            Ok(())
        });
        if let Err(error) = result {
            // The UI optimistically updates the native window; restore its last
            // saved level if atomic preference persistence fails.
            app.global::<riina_ui::PlayerLayoutState>()
                .set_always_float_on_top(!requested);
            app.set_status(error.to_string().into());
        }
    });
}
