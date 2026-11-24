#[cfg(target_os = "macos")]
mod version_probe;

use super::close_observer::CloseObserver;
use crate::{AboutWindow, PlayerWindow};
use riina_core::{Phase, Playback};
use riina_ui::{EngineVersionStatus, PlayerMenuActions};
use slint::{CloseRequestResponse, ComponentHandle, winit_030::WinitWindowAccessor};
use std::{cell::RefCell, rc::Rc, time::Duration};

#[derive(Default)]
pub(crate) struct ApplicationAbout {
    window: RefCell<Option<AboutWindow>>,
    timer: Rc<slint::Timer>,
    pub(crate) closed: CloseObserver,
    #[cfg(target_os = "macos")]
    probe: version_probe::Probe,
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
        self: &Rc<Self>,
        app: &PlayerWindow,
        playback: &crate::PlaybackStore,
    ) -> Result<(), slint::PlatformError> {
        #[cfg(target_os = "macos")]
        self.probe.application_view.set(false);
        let window = self.window_for(Some(app))?;
        window.set_dark_theme(app.global::<riina_ui::PlayerAppearance>().get_dark_theme());
        let mut versions_ready =
            refresh_engine_versions(&window, &riina_core::sync::lock(playback));
        reveal(&window)?;
        let source = app.as_weak();
        let playback = playback.clone();
        super::visible_refresh::run(
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

    fn window_for(
        self: &Rc<Self>,
        app: Option<&PlayerWindow>,
    ) -> Result<AboutWindow, slint::PlatformError> {
        if let Some(window) = self.window.borrow().as_ref() {
            return Ok(window.clone_strong());
        }
        let window = AboutWindow::new()?;
        crate::appearance_preferences::install_system_typography(&window);
        window.set_app_version(env!("CARGO_PKG_VERSION").into());
        connect_links(&window);
        if let Some(app) = app {
            crate::platform::center_auxiliary_window(&window, app);
        }
        let owner = Rc::downgrade(self);
        window.window().on_close_requested(move || {
            if let Some(owner) = owner.upgrade() {
                owner.timer.stop();
                owner.closed.notify();
            }
            CloseRequestResponse::HideWindow
        });
        self.window.replace(Some(window.clone_strong()));
        Ok(window)
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn is_visible(&self) -> bool {
        self.window
            .borrow()
            .as_ref()
            .is_some_and(|window| window.window().is_visible())
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn hide(&self) {
        self.timer.stop();
        if let Some(window) = self.window.borrow().as_ref() {
            let _ = window.hide();
        }
    }
}

fn reveal(window: &AboutWindow) -> Result<(), slint::PlatformError> {
    crate::platform::show_window(window)?;
    crate::platform::enable_about_acrylic(window);
    window.window().with_winit_window(|window| {
        window.set_minimized(false);
        window.focus_window();
    });
    Ok(())
}

fn refresh_engine_versions(window: &AboutWindow, playback: &Playback) -> bool {
    let status = if playback.phase == Phase::Stopped {
        EngineVersionStatus::Checking
    } else {
        EngineVersionStatus::Reading
    };
    window.set_engine_version_status(status);
    window.set_engine_version(playback.mpv_version.as_deref().unwrap_or_default().into());
    window.set_ffmpeg_version(
        playback
            .ffmpeg_version
            .as_deref()
            .unwrap_or_default()
            .into(),
    );
    playback.mpv_version.is_some() && playback.ffmpeg_version.is_some()
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
    use super::*;
    use slint::platform::WindowEvent;
    use std::{
        cell::Cell,
        sync::{Arc, Mutex},
    };

    #[test]
    fn application_and_player_reuse_about_and_observe_native_close()
    -> Result<(), slint::PlatformError> {
        i_slint_backend_testing::init_no_event_loop();
        let about = Rc::new(ApplicationAbout::default());
        let opened_without_player = about.window_for(None)?;
        reveal(&opened_without_player)?;
        let was_hidden = Rc::new(Cell::new(false));
        let observed = was_hidden.clone();
        let window = opened_without_player.as_weak();
        about.closed.bind(Rc::new(move || {
            observed.set(
                window
                    .upgrade()
                    .is_some_and(|window| !window.window().is_visible()),
            );
        }));

        let player = PlayerWindow::new()?;
        let playback = Arc::new(Mutex::new(Arc::new(Playback {
            mpv_version: Some("test-mpv".into()),
            ffmpeg_version: Some("test-ffmpeg".into()),
            ..Playback::default()
        })));
        about.connect(&player, playback);
        player.global::<PlayerMenuActions>().invoke_show_about();
        assert_eq!(opened_without_player.get_engine_label(), "libmpv test-mpv");
        assert_eq!(
            opened_without_player.get_ffmpeg_label(),
            "FFmpeg test-ffmpeg"
        );
        assert!(about.timer.running());
        opened_without_player
            .window()
            .dispatch_event(WindowEvent::CloseRequested);
        assert!(!about.timer.running());
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(1));
        assert!(was_hidden.get());
        Ok(())
    }

    #[test]
    fn about_shows_available_versions_and_explains_pending_ones()
    -> Result<(), Box<dyn std::error::Error>> {
        i_slint_backend_testing::init_no_event_loop();
        let window = AboutWindow::new()?;
        slint::select_bundled_translation("en")?;
        assert!(!refresh_engine_versions(&window, &Playback::default()));
        assert_eq!(window.get_engine_label(), "Checking libmpv version…");
        assert_eq!(window.get_ffmpeg_label(), "Checking FFmpeg version…");

        let mut playback = Playback {
            phase: Phase::Loading,
            mpv_version: Some("0.40.0".into()),
            ..Playback::default()
        };
        assert!(!refresh_engine_versions(&window, &playback));
        assert_eq!(window.get_engine_label(), "libmpv 0.40.0");
        assert_eq!(window.get_ffmpeg_label(), "Reading FFmpeg version…");

        playback.ffmpeg_version = Some("7.1".into());
        assert!(refresh_engine_versions(&window, &playback));
        assert_eq!(window.get_ffmpeg_label(), "FFmpeg 7.1");
        Ok(())
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
