//! Route plugin window changes through the owner and the player's panel policies.
use super::select_plugin_sidebar_tab;
use crate::{
    NativeVideoHost, PlayerWindow,
    platform::plugin_window::{self, Frame},
    plugin_files::WindowProperties,
};
use riina_core::plugin::WindowAction;
use riina_ui::{PlayerLayoutState, PlayerMenuActions, PlayerSidebarActions, PlayerSidebarState};
use serde_json::json;
use slint::{ComponentHandle, Model};
use std::collections::BTreeMap;

pub(super) fn properties(
    app: &PlayerWindow,
    native_host: Option<&slint::Weak<NativeVideoHost>>,
) -> Option<WindowProperties> {
    let host = native_host.and_then(slint::Weak::upgrade);
    let state = plugin_window::observe(
        host.as_ref()
            .map_or_else(|| app.window(), |host| host.window()),
    )?;
    Some(WindowProperties {
        current_screen: state.current_screen,
        values: BTreeMap::from([
            ("loaded".into(), json!(true)),
            ("frame".into(), json!(state.frame)),
            ("visible".into(), json!(state.visible)),
            ("main".into(), json!(state.main)),
            ("miniaturized".into(), json!(state.minimized)),
            ("screens".into(), json!(state.screens)),
            ("sidebar".into(), json!(settings_tab(app))),
            (
                "ontop".into(),
                json!(app.global::<PlayerLayoutState>().get_always_float_on_top()),
            ),
            (
                "window-size-adjusted-event".into(),
                json!(app.get_window_size_adjusted_event()),
            ),
        ]),
    })
}

fn settings_tab(app: &PlayerWindow) -> Option<&'static str> {
    let sidebar = app.global::<PlayerSidebarState>();
    if !sidebar.get_quick_settings_visible() {
        return None;
    }
    [sidebar.get_sidebar(), sidebar.get_leading_sidebar()]
        .into_iter()
        .find_map(|pane| match pane {
            1 => Some("audio"),
            2 => Some("sub"),
            5 => Some("layout"),
            6 => Some("video"),
            _ => None,
        })
}

pub(super) fn apply(
    app: &PlayerWindow,
    native_host: Option<&slint::Weak<NativeVideoHost>>,
    action: WindowAction,
) {
    let host = native_host.and_then(slint::Weak::upgrade);
    let owner = host
        .as_ref()
        .map_or_else(|| app.window(), |host| host.window());
    match action {
        WindowAction::Quit => app.global::<PlayerMenuActions>().invoke_quit_riina(),
        WindowAction::SetFrame {
            x,
            y,
            width,
            height,
        } => {
            if let Err(error) =
                plugin_window::set_frame(owner, Frame::positioned(x, y, width, height))
            {
                crate::diagnostics::report(
                    app,
                    format!("Could not apply the plugin window frame: {error}"),
                );
            }
        }
        WindowAction::SetFullscreen(active) => {
            app.global::<PlayerMenuActions>()
                .invoke_request_fullscreen(active);
        }
        WindowAction::SetPictureInPicture(active) => {
            if app.get_picture_in_picture_active() != active {
                app.global::<PlayerMenuActions>()
                    .invoke_toggle_picture_in_picture();
            }
        }
        WindowAction::SetOnTop(active) => {
            app.global::<PlayerLayoutState>()
                .set_always_float_on_top(active);
        }
        WindowAction::SetSidebar(name) => set_sidebar(app, name.as_deref()),
        WindowAction::SetMiniaturized(active) => plugin_window::set_minimized(owner, active),
    }
}

fn set_sidebar(app: &PlayerWindow, name: Option<&str>) {
    let sidebar = app.global::<PlayerSidebarState>();
    let actions = app.global::<PlayerSidebarActions>();
    match name {
        None => {
            sidebar.set_sidebar(0);
            sidebar.set_leading_sidebar(0);
        }
        Some("audio") => actions.invoke_select_settings_pane(1),
        Some("sub" | "subtitles") => actions.invoke_select_settings_pane(2),
        Some("layout") => actions.invoke_select_settings_pane(5),
        Some("video") => actions.invoke_select_settings_pane(6),
        Some("chapter" | "chapters") => actions.invoke_select_playlist_pane(3),
        Some("playlist") => actions.invoke_select_playlist_pane(4),
        Some("plugins") => {
            if sidebar.get_sidebar() != 7 && sidebar.get_leading_sidebar() != 7 {
                crate::sidebar_preferences::show_plugins_panel(app);
            }
        }
        Some(identifier) => {
            let identifiers = sidebar.get_plugin_sidebar_identifiers();
            if identifiers.iter().any(|entry| entry.as_str() == identifier) {
                select_plugin_sidebar_tab(app, identifier);
            } else {
                app.set_status("Plugin requested an unsupported sidebar name".into());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_sidebar_uses_preferred_sides_and_only_reads_quick_settings() {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new().unwrap();
        crate::sidebar_preferences::connect(&app, Default::default(), Default::default());
        let sidebar = app.global::<PlayerSidebarState>();
        sidebar.set_settings_at_leading(true);
        sidebar.set_playlist_at_leading(false);
        set_sidebar(&app, Some("chapters"));
        assert_eq!(sidebar.get_sidebar(), 3);
        assert_eq!(settings_tab(&app), None);
        set_sidebar(&app, Some("sub"));
        set_sidebar(&app, Some("sub"));
        assert_eq!(sidebar.get_leading_sidebar(), 2);
        assert_eq!(sidebar.get_sidebar(), 3);
        assert_eq!(settings_tab(&app), Some("sub"));
        sidebar.set_plugin_sidebar_identifiers(slint::ModelRc::new(slint::VecModel::from(vec![
            "org.example.pane".into(),
        ])));
        set_sidebar(&app, Some("org.example.pane"));
        assert_eq!(sidebar.get_leading_sidebar(), 7);
        assert_eq!(sidebar.get_plugin_sidebar_selected_index(), 0);
        assert_eq!(settings_tab(&app), None);
        set_sidebar(&app, None);
        assert_eq!(
            (sidebar.get_leading_sidebar(), sidebar.get_sidebar()),
            (0, 0)
        );
    }
}
