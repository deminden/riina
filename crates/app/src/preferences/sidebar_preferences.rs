//! Persist a completed resize, never every intermediate drag event.
use crate::PlayerWindow;
use riina_core::Preferences;
use riina_ui::{PlayerMenuActions, PlayerMusicModeState, PlayerSidebarActions, PlayerSidebarState};
use slint::ComponentHandle;
use std::{cell::Cell, path::PathBuf, rc::Rc};

pub(crate) fn show_plugins_panel(app: &PlayerWindow) {
    let (leading, trailing) = plugin_panel_destination(
        app.global::<riina_ui::PlayerSidebarState>()
            .get_plugins_at_leading(),
        app.global::<riina_ui::PlayerSidebarState>()
            .get_leading_sidebar(),
        app.global::<riina_ui::PlayerSidebarState>().get_sidebar(),
    );
    app.global::<riina_ui::PlayerSidebarState>()
        .set_leading_sidebar(leading);
    app.global::<riina_ui::PlayerSidebarState>()
        .set_sidebar(trailing);
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

fn choose_catalog_pane(app: &PlayerWindow, pane: i32, toggle: bool) {
    let sidebar = app.global::<PlayerSidebarState>();
    let music = app.global::<PlayerMusicModeState>();
    let close = toggle && sidebar.get_visible_catalog_pane() == pane;
    sidebar.set_remembered_playlist_pane(pane);
    if music.get_music_mode() {
        let visible = !close;
        if visible != music.get_music_mode_show_playlist() {
            music.invoke_set_catalog_visible(visible);
        }
    } else if sidebar.get_playlist_at_leading() {
        sidebar.set_leading_sidebar(if close { 0 } else { pane });
    } else {
        sidebar.set_sidebar(if close { 0 } else { pane });
    }
}

pub(crate) fn connect(app: &PlayerWindow, path: PathBuf, baseline: Preferences) {
    let weak_settings = app.as_weak();
    app.global::<PlayerMenuActions>()
        .on_toggle_settings_pane(move |kind| {
            let Some(app) = weak_settings.upgrade() else {
                return;
            };
            if app.global::<PlayerMusicModeState>().get_music_mode() {
                return;
            }
            app.global::<riina_ui::PlayerSidebarState>()
                .set_remembered_settings_pane(kind);
            let destination_leading = app
                .global::<riina_ui::PlayerSidebarState>()
                .get_settings_at_leading();
            let current = if destination_leading {
                app.global::<riina_ui::PlayerSidebarState>()
                    .get_leading_sidebar()
            } else {
                app.global::<riina_ui::PlayerSidebarState>().get_sidebar()
            };
            let next = if current == kind { 0 } else { kind };
            if destination_leading {
                app.global::<riina_ui::PlayerSidebarState>()
                    .set_leading_sidebar(next);
            } else {
                app.global::<riina_ui::PlayerSidebarState>()
                    .set_sidebar(next);
            }
            app.global::<riina_ui::PlayerControlActions>()
                .invoke_activity();
        });
    let weak_settings = app.as_weak();
    app.global::<riina_ui::PlayerSidebarActions>()
        .on_select_settings_pane(move |kind| {
            let Some(app) = weak_settings.upgrade() else {
                return;
            };
            if app
                .global::<riina_ui::PlayerSidebarState>()
                .get_settings_at_leading()
            {
                app.global::<riina_ui::PlayerSidebarState>()
                    .set_remembered_settings_pane(kind);
                app.global::<riina_ui::PlayerSidebarState>()
                    .set_leading_sidebar(kind);
            } else {
                app.global::<riina_ui::PlayerSidebarState>()
                    .set_remembered_settings_pane(kind);
                app.global::<riina_ui::PlayerSidebarState>()
                    .set_sidebar(kind);
            }
        });
    let weak_playlist = app.as_weak();
    app.global::<PlayerMenuActions>()
        .on_toggle_playlist_pane(move |kind| {
            if let Some(app) = weak_playlist.upgrade() {
                choose_catalog_pane(&app, kind, true);
            }
        });
    let weak_playlist = app.as_weak();
    app.global::<PlayerSidebarActions>()
        .on_select_playlist_pane(move |kind| {
            if let Some(app) = weak_playlist.upgrade() {
                choose_catalog_pane(&app, kind, false);
            }
        });
    let weak_plugins = app.as_weak();
    app.global::<PlayerMenuActions>()
        .on_toggle_plugins_pane(move || {
            let Some(app) = weak_plugins.upgrade() else {
                return;
            };
            if app
                .global::<riina_ui::PlayerSidebarState>()
                .get_leading_sidebar()
                == 7
            {
                app.global::<riina_ui::PlayerSidebarState>()
                    .set_leading_sidebar(0);
            } else if app.global::<riina_ui::PlayerSidebarState>().get_sidebar() == 7 {
                app.global::<riina_ui::PlayerSidebarState>().set_sidebar(0);
            } else {
                show_plugins_panel(&app);
            }
        });
    connect_width_persistence(app, path, baseline);
}

fn connect_width_persistence(app: &PlayerWindow, path: PathBuf, baseline: Preferences) {
    let saved = Rc::new(Cell::new((
        baseline.leading_sidebar_width,
        baseline.trailing_sidebar_width,
    )));
    let weak = app.as_weak();
    app.on_sidebar_resize_finished(move || {
        let Some(app) = weak.upgrade() else { return };
        let widths = (
            (app.global::<riina_ui::PlayerSidebarState>()
                .get_leading_sidebar_width() as u32)
                .clamp(300, 500),
            (app.global::<riina_ui::PlayerSidebarState>()
                .get_trailing_sidebar_width() as u32)
                .clamp(300, 500),
        );
        let result = crate::preference_store::update(&path, &baseline, |preferences| {
            preferences.leading_sidebar_width = app
                .global::<riina_ui::PlayerSidebarState>()
                .get_leading_sidebar_width() as u32;
            preferences.trailing_sidebar_width =
                app.global::<riina_ui::PlayerSidebarState>()
                    .get_trailing_sidebar_width() as u32;
            Ok(())
        });
        if let Err(error) = result {
            let previous = saved.get();
            app.global::<riina_ui::PlayerSidebarState>()
                .set_leading_sidebar_width(previous.0 as f32);
            app.global::<riina_ui::PlayerSidebarState>()
                .set_trailing_sidebar_width(previous.1 as f32);
            app.set_status(error.into());
        } else {
            saved.set(widths);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_panel_opens_on_the_saved_side() {
        assert_eq!(plugin_panel_destination(true, 0, 2), (7, 2));
        assert_eq!(plugin_panel_destination(false, 3, 0), (3, 7));
    }

    #[test]
    fn catalog_menu_toggles_the_active_pane_and_tabs_change_it_in_place() {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new().unwrap();
        connect(&app, PathBuf::new(), Preferences::default());
        let menu = app.global::<PlayerMenuActions>();
        let tabs = app.global::<PlayerSidebarActions>();
        let sidebar = app.global::<PlayerSidebarState>();
        let music = app.global::<PlayerMusicModeState>();
        let weak = app.as_weak();
        music.on_set_catalog_visible(move |visible| {
            if let Some(app) = weak.upgrade() {
                app.global::<PlayerMusicModeState>()
                    .set_music_mode_show_playlist(visible);
            }
        });
        music.set_music_mode_show_playlist(false);
        sidebar.set_playlist_at_leading(false);

        menu.invoke_toggle_playlist_pane(3);
        assert_eq!(sidebar.get_sidebar(), 3);
        tabs.invoke_select_playlist_pane(4);
        assert_eq!(sidebar.get_sidebar(), 4);
        menu.invoke_toggle_playlist_pane(4);
        assert_eq!(sidebar.get_visible_catalog_pane(), 0);

        music.set_music_mode(true);
        menu.invoke_toggle_playlist_pane(3);
        assert!(music.get_music_mode_show_playlist());
        assert_eq!(sidebar.get_visible_catalog_pane(), 3);
        assert_eq!(sidebar.get_sidebar(), 0);
        assert_eq!(sidebar.get_leading_sidebar(), 0);
        menu.invoke_toggle_playlist_pane(4);
        assert!(music.get_music_mode_show_playlist());
        assert_eq!(sidebar.get_visible_catalog_pane(), 4);
        tabs.invoke_select_playlist_pane(3);
        tabs.invoke_select_playlist_pane(3);
        assert_eq!(sidebar.get_visible_catalog_pane(), 3);
        menu.invoke_toggle_playlist_pane(3);
        assert!(!music.get_music_mode_show_playlist());
        assert_eq!(sidebar.get_remembered_playlist_pane(), 3);
        menu.invoke_toggle_settings_pane(6);
        assert_eq!(sidebar.get_sidebar(), 0);
    }
}
