//! Bind the player and Welcome window lifecycle.
use crate::{PlayerWindow, WelcomeWindow, platform};
use riina_ui::PlayerMenuActions;
use slint::ComponentHandle;

#[cfg(test)]
mod tests;

pub(super) fn install_lifecycle(app: &PlayerWindow, welcome: Option<&WelcomeWindow>) {
    let weak = app.as_weak();
    app.window().on_close_requested(move || {
        if weak.upgrade().is_some_and(|app| {
            app.global::<riina_ui::PlayerDialogState>()
                .get_dialog_open()
        }) {
            slint::CloseRequestResponse::KeepWindowShown
        } else {
            if let Some(app) = weak.upgrade() {
                app.invoke_window_will_close();
                app.invoke_dismiss_font_picker();
                app.global::<riina_ui::PlayerSidebarActions>()
                    .invoke_dismiss_native_subtitle_color();
                // Remove the borrowed texture before hide suspends its context.
                // Teardown runs inside the backend's window borrow and cannot
                // safely dirty a property that schedules another redraw.
                app.global::<riina_ui::PlayerRenderedVideo>()
                    .set_video_frame(slint::Image::default());
                app.set_engine_ready(false);
                app.global::<PlayerMenuActions>().invoke_close_player();
            }
            slint::CloseRequestResponse::HideWindow
        }
    });
    let minimize_window = app.as_weak();
    let hide_app = app.as_weak();
    app.global::<PlayerMenuActions>().on_hide_riina(move || {
        if let Some(app) = hide_app.upgrade()
            && let Err(error) = platform::hide_application()
        {
            app.set_status(error.into());
        }
    });
    app.global::<PlayerMenuActions>()
        .on_minimize_window(move || {
            if let Some(app) = minimize_window.upgrade() {
                platform::minimize_window(app.window());
            }
        });
    let zoom_window = app.as_weak();
    app.global::<PlayerMenuActions>().on_zoom_window(move || {
        if let Some(app) = zoom_window.upgrade() {
            platform::toggle_zoom_window(app.window());
        }
    });
    let close_player = app.as_weak();
    app.global::<PlayerMenuActions>().on_close_player(move || {
        if let Some(app) = close_player.upgrade() {
            app.invoke_window_will_close();
            app.global::<PlayerMenuActions>().invoke_stop();
        }
        // Shutdown may already have ended the event loop.
        let _ = slint::quit_event_loop();
    });

    if let Some(welcome) = welcome {
        let weak = app.as_weak();
        welcome.on_show_playback_history(move || {
            if let Some(app) = weak.upgrade() {
                app.global::<PlayerMenuActions>()
                    .invoke_show_playback_history();
            }
        });
        let weak = app.as_weak();
        welcome.on_open_file(move || {
            if let Some(app) = weak.upgrade() {
                app.global::<PlayerMenuActions>().invoke_open_file();
            }
        });
        let weak = app.as_weak();
        welcome.on_open_recent(move |token, resume| {
            if let Some(app) = weak.upgrade() {
                app.invoke_open_recent(token, resume);
            }
        });
    }
}
