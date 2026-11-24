use crate::{AboutWindow, PlayerWindow};
use riina_core::{Phase, Playback};
use riina_ui::PlayerMenuActions;
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{cell::RefCell, rc::Rc, time::Duration};

#[derive(Default)]
pub(crate) struct ApplicationAbout {
    window: RefCell<Option<AboutWindow>>,
    timer: Rc<slint::Timer>,
}

impl ApplicationAbout {
    pub(crate) fn connect(self: &Rc<Self>, app: &PlayerWindow, playback: crate::PlaybackStore) {
        let owner = self.clone();
        let weak = app.as_weak();
        app.global::<PlayerMenuActions>().on_show_about(move || {
            if let Some(app) = weak.upgrade()
                && let Err(error) = owner.show(&app, &playback)
            {
                crate::diagnostics::report(&app, format!("Could not open About Riina: {error}"));
            }
        });
    }

    fn show(
        &self,
        app: &PlayerWindow,
        playback: &crate::PlaybackStore,
    ) -> Result<(), slint::PlatformError> {
        let existing = self
            .window
            .borrow()
            .as_ref()
            .map(ComponentHandle::clone_strong);
        let window = if let Some(window) = existing {
            window
        } else {
            let window = AboutWindow::new()?;
            crate::appearance_preferences::install_system_typography(&window);
            window.set_version_label(format!("Version {}", env!("CARGO_PKG_VERSION")).into());
            connect_links(&window);
            crate::platform::center_auxiliary_window(&window, app);
            self.window.borrow_mut().replace(window.clone_strong());
            window
        };
        window.set_dark_theme(app.global::<riina_ui::PlayerAppearance>().get_dark_theme());
        let mut versions_ready =
            refresh_engine_versions(&window, &riina_core::sync::lock(playback));
        crate::platform::show_window(&window)?;
        crate::platform::enable_about_acrylic(&window);
        window.window().with_winit_window(|window| {
            window.set_minimized(false);
            window.focus_window();
        });
        self.timer.stop();
        let source = app.as_weak();
        let playback = playback.clone();
        super::visible_refresh::start(
            &window,
            &self.timer,
            Duration::from_millis(250),
            move |window| {
                if let Some(app) = source.upgrade() {
                    let dark_theme = app.global::<riina_ui::PlayerAppearance>().get_dark_theme();
                    if window.get_dark_theme() != dark_theme {
                        window.set_dark_theme(dark_theme);
                    }
                }
                if !versions_ready {
                    versions_ready =
                        refresh_engine_versions(window, &riina_core::sync::lock(&playback));
                }
            },
        );
        Ok(())
    }
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

fn connect_links(window: &AboutWindow) {
    let license = window.as_weak();
    window.on_open_license(move || {
        open_link(
            &license,
            "https://github.com/deminden/riina/blob/main/LICENSE",
        );
    });
    let project = window.as_weak();
    window.on_open_project(move || {
        open_link(&project, "https://github.com/deminden/riina");
    });
    let contributors = window.as_weak();
    window.on_open_contributors(move || {
        open_link(
            &contributors,
            "https://github.com/deminden/riina/graphs/contributors",
        );
    });
}

fn open_link(window: &slint::Weak<AboutWindow>, url: &str) {
    if let Some(window) = window.upgrade() {
        let status = match crate::platform::open_http_url_in_browser(url) {
            Ok(()) => String::new(),
            Err(error) => format!("Could not open the project link: {error}"),
        };
        window.set_link_status(status.into());
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

#[cfg(test)]
mod ownership_tests {
    use super::*;
    use slint::platform::WindowEvent;

    #[test]
    fn players_share_about_navigation_and_update_the_reported_versions()
    -> Result<(), Box<dyn std::error::Error>> {
        i_slint_backend_testing::init_no_event_loop();
        let first = PlayerWindow::new()?;
        let second = PlayerWindow::new()?;
        let owner = Rc::new(ApplicationAbout::default());
        let state = crate::PlaybackStore::default();
        owner.connect(&first, state);
        let state = crate::PlaybackStore::default();
        *riina_core::sync::lock(&state) = std::sync::Arc::new(Playback {
            mpv_version: Some("0.41.0".into()),
            ffmpeg_version: Some("8.0".into()),
            ..Default::default()
        });
        owner.connect(&second, state);

        first.global::<PlayerMenuActions>().invoke_show_about();
        let window = owner
            .window
            .borrow()
            .as_ref()
            .ok_or("About did not open")?
            .clone_strong();
        window.set_selected_tab(2);
        second.global::<PlayerMenuActions>().invoke_show_about();
        drop(first);
        assert!(window.window().is_visible());
        assert_eq!(window.get_selected_tab(), 2);
        assert_eq!(window.get_engine_label(), "libmpv 0.41.0");
        assert_eq!(window.get_ffmpeg_label(), "FFmpeg 8.0");
        window.window().dispatch_event(WindowEvent::CloseRequested);
        assert!(!owner.timer.running());
        Ok(())
    }
}
