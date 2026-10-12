use crate::{AboutWindow, PlayerWindow};
use riina_core::Playback;
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

pub(crate) fn connect(app: &PlayerWindow, playback: Arc<Mutex<Playback>>) {
    let about_slot = Rc::new(RefCell::new(None::<AboutWindow>));
    let slot = about_slot.clone();
    let weak_app = app.as_weak();
    app.on_show_about(move || {
        let Some(app) = weak_app.upgrade() else {
            return;
        };
        let window = if let Some(window) = slot.borrow().as_ref() {
            window.clone_strong()
        } else {
            let Ok(window) = AboutWindow::new() else {
                app.set_status("Could not open About Riina".into());
                return;
            };
            connect_links(&window, app.as_weak());
            *slot.borrow_mut() = Some(window.clone_strong());
            window
        };
        window.set_version_label(format!("Version {}", env!("CARGO_PKG_VERSION")).into());
        window.set_dark_theme(app.get_dark_theme());
        let engine = playback
            .lock()
            .ok()
            .and_then(|state| state.mpv_version.clone())
            .map_or_else(
                || "libmpv version unavailable".to_owned(),
                |version| format!("libmpv {version}"),
            );
        window.set_engine_label(engine.into());
        let ffmpeg = playback
            .lock()
            .ok()
            .and_then(|state| state.ffmpeg_version.clone())
            .map_or_else(
                || "FFmpeg version unavailable".to_owned(),
                |version| format!("FFmpeg {version}"),
            );
        window.set_ffmpeg_label(ffmpeg.into());
        if let Err(error) = window.show() {
            app.set_status(format!("Could not show About Riina: {error}").into());
        }
    });
}

fn connect_links(window: &AboutWindow, weak_app: slint::Weak<PlayerWindow>) {
    let app = weak_app.clone();
    window.on_open_license(move || {
        open_link(&app, "https://github.com/deminden/riina/blob/main/LICENSE");
    });
    let app = weak_app.clone();
    window.on_open_project(move || {
        open_link(&app, "https://github.com/deminden/riina");
    });
    window.on_open_contributors(move || {
        open_link(
            &weak_app,
            "https://github.com/deminden/riina/graphs/contributors",
        );
    });
}

fn open_link(app: &slint::Weak<PlayerWindow>, url: &str) {
    let Some(app) = app.upgrade() else { return };
    if let Err(error) = crate::platform::open_http_url_in_browser(url) {
        app.set_status(format!("Could not open the project link: {error}").into());
    }
}
