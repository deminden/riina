//! Shared fullscreen policy follows an accepted platform transition.
use crate::{NativeVideoHost, PlayerWindow, WelcomeWindow, platform};
use riina_ui::PlayerMenuActions;
use slint::ComponentHandle;

#[cfg(test)]
mod tests;

pub(super) fn install_lifecycle(app: &PlayerWindow, welcome: Option<&WelcomeWindow>) {
    let weak = app.as_weak();
    app.window().on_close_requested(move || {
        if weak.upgrade().is_some_and(|app| app.get_dialog_open()) {
            slint::CloseRequestResponse::KeepWindowShown
        } else {
            if let Some(app) = weak.upgrade() {
                app.invoke_dismiss_font_picker();
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
            && let Err(error) = platform::hide_application(app.window())
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
        let _ = slint::quit_event_loop();
    });

    if let Some(welcome) = welcome {
        let weak = app.as_weak();
        welcome.on_open_file(move || {
            if let Some(app) = weak.upgrade() {
                app.global::<PlayerMenuActions>().invoke_open_file();
            }
        });
        let weak = app.as_weak();
        welcome.on_open_url(move |value| {
            if let Some(app) = weak.upgrade() {
                app.invoke_open_url(value);
            }
        });
        let weak = app.as_weak();
        welcome.on_open_recent(move |index| {
            if let Some(app) = weak.upgrade() {
                app.invoke_open_recent(index);
            }
        });
    }
}

pub(super) fn install_fullscreen(
    app: &PlayerWindow,
    fullscreen_host: Option<slint::Weak<NativeVideoHost>>,
) {
    let weak = app.as_weak();
    app.global::<PlayerMenuActions>()
        .on_toggle_fullscreen(move || {
            if let Some(app) = weak.upgrade() {
                app.invoke_activity();
                let fullscreen = !app.get_fullscreen();
                match fullscreen_host.as_ref() {
                    Some(host) => {
                        let Some(host) = host.upgrade() else { return };
                        if !host.invoke_set_fullscreen_window(fullscreen) {
                            return;
                        }
                    }
                    None => app.window().set_fullscreen(fullscreen),
                }
                app.set_fullscreen(fullscreen);
                if fullscreen
                    && app.get_loaded()
                    && app.get_paused()
                    && app
                        .global::<riina_ui::PlayerPreferences>()
                        .get_play_when_entering_fullscreen()
                {
                    app.invoke_set_paused(false);
                } else if !fullscreen
                    && app.get_loaded()
                    && !app.get_paused()
                    && app
                        .global::<riina_ui::PlayerPreferences>()
                        .get_pause_when_leaving_fullscreen()
                {
                    app.invoke_set_paused(true);
                }
            }
        });
}
