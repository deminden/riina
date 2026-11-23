use crate::{AboutWindow, PlayerWindow};
use riina_core::{Phase, Playback};
use riina_ui::PlayerMenuActions;
use slint::ComponentHandle;
use std::{cell::RefCell, rc::Rc, time::Duration};

pub(crate) fn connect(app: &PlayerWindow, playback: crate::PlaybackStore) {
    let about_slot = Rc::new(RefCell::new(None::<AboutWindow>));
    let refresh_timer = Rc::new(slint::Timer::default());
    let slot = about_slot.clone();
    let weak_app = app.as_weak();
    app.global::<PlayerMenuActions>().on_show_about(move || {
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
            crate::appearance_preferences::install_system_typography(&window);
            connect_links(&window, app.as_weak());
            crate::platform::center_auxiliary_window(&window, &app);
            *slot.borrow_mut() = Some(window.clone_strong());
            window
        };
        window.set_version_label(format!("Version {}", env!("CARGO_PKG_VERSION")).into());
        window.set_dark_theme(app.global::<riina_ui::PlayerAppearance>().get_dark_theme());
        let versions_ready = playback
            .lock()
            .map(|state| refresh_engine_versions(&window, &state))
            .unwrap_or_else(|_| refresh_engine_versions(&window, &Playback::default()));
        if let Err(error) = crate::platform::show_window(&window) {
            crate::diagnostics::report(&app, format!("Could not show About Riina: {error}"));
            return;
        }
        crate::platform::enable_about_acrylic(&window);
        refresh_timer.stop();
        let weak_app = app.as_weak();
        let weak_timer = Rc::downgrade(&refresh_timer);
        let playback = playback.clone();
        let mut versions_ready = versions_ready;
        super::visible_refresh::start(
            &window,
            &refresh_timer,
            Duration::from_millis(250),
            move |window| {
                let Some(app) = weak_app.upgrade() else {
                    if let Some(timer) = weak_timer.upgrade() {
                        timer.stop();
                    }
                    return;
                };
                let dark_theme = app.global::<riina_ui::PlayerAppearance>().get_dark_theme();
                if window.get_dark_theme() != dark_theme {
                    window.set_dark_theme(dark_theme);
                }
                if !versions_ready {
                    versions_ready =
                        refresh_engine_versions(window, &riina_core::sync::lock(&playback));
                }
            },
        );
    });
}

fn refresh_engine_versions(window: &AboutWindow, playback: &Playback) -> bool {
    let (engine, ffmpeg) = engine_version_labels(playback);
    window.set_engine_label(engine.into());
    window.set_ffmpeg_label(ffmpeg.into());
    playback.mpv_version.is_some() && playback.ffmpeg_version.is_some()
}

fn engine_version_labels(playback: &Playback) -> (String, String) {
    let engine_pending = if playback.phase == Phase::Stopped {
        "Checking libmpv version…"
    } else {
        "Reading libmpv version…"
    };
    let ffmpeg_pending = if playback.phase == Phase::Stopped {
        "Checking FFmpeg version…"
    } else {
        "Reading FFmpeg version…"
    };
    let engine = playback.mpv_version.as_ref().map_or_else(
        || engine_pending.to_owned(),
        |version| format!("libmpv {version}"),
    );
    let ffmpeg = playback.ffmpeg_version.as_ref().map_or_else(
        || ffmpeg_pending.to_owned(),
        |version| format!("FFmpeg {version}"),
    );
    (engine, ffmpeg)
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
        crate::diagnostics::report(&app, format!("Could not open the project link: {error}"));
    }
}

#[cfg(test)]
mod tests {
    use super::engine_version_labels;
    use riina_core::{Phase, Playback};

    #[test]
    fn about_explains_engine_version_is_still_pending() {
        assert_eq!(
            engine_version_labels(&Playback::default()),
            (
                "Checking libmpv version…".to_owned(),
                "Checking FFmpeg version…".to_owned()
            )
        );
    }

    #[test]
    fn about_shows_pending_versions_while_media_loads() {
        let playback = Playback {
            phase: Phase::Loading,
            ..Playback::default()
        };
        assert_eq!(
            engine_version_labels(&playback),
            (
                "Reading libmpv version…".to_owned(),
                "Reading FFmpeg version…".to_owned()
            )
        );
    }

    #[test]
    fn about_formats_reported_engine_versions() {
        let playback = Playback {
            mpv_version: Some("0.40.0".into()),
            ffmpeg_version: Some("7.1".into()),
            ..Playback::default()
        };
        assert_eq!(
            engine_version_labels(&playback),
            ("libmpv 0.40.0".to_owned(), "FFmpeg 7.1".to_owned())
        );
    }
}
