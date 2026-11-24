//! Own the release check and its single window for the application lifetime.
mod releases;

use crate::{NativeVideoHost, PlayerWindow, UpdateWindow, platform::NativePresentation};
use riina_ui::{PlayerAppearance, PlayerMenuActions};
use slint::{ComponentHandle, Weak, winit_030::WinitWindowAccessor};
use std::{cell::RefCell, rc::Rc};

#[derive(Default)]
pub(crate) struct ApplicationUpdates {
    window: RefCell<Option<UpdateWindow>>,
}

impl ApplicationUpdates {
    pub(crate) fn connect(
        self: &Rc<Self>,
        app: &PlayerWindow,
        presentation: Option<&NativePresentation>,
    ) {
        let updates = Rc::downgrade(self);
        let player = app.as_weak();
        let host = presentation.map(NativePresentation::host);
        app.global::<PlayerMenuActions>()
            .on_check_for_updates(move || {
                if let (Some(updates), Some(app)) = (updates.upgrade(), player.upgrade())
                    && let Err(error) = updates.show(&app, host.as_ref().and_then(Weak::upgrade))
                {
                    crate::diagnostics::report(
                        &app,
                        format!("Could not open Software Update: {error}"),
                    );
                }
            });
    }

    fn show(
        &self,
        app: &PlayerWindow,
        host: Option<NativeVideoHost>,
    ) -> Result<(), slint::PlatformError> {
        let window = self.window()?;
        let was_visible = window.window().is_visible();
        window.set_dark_theme(app.global::<PlayerAppearance>().get_dark_theme());
        if !was_visible {
            if let Some(host) = host.as_ref() {
                crate::platform::center_auxiliary_window(&window, host);
            } else {
                crate::platform::center_auxiliary_window(&window, app);
            }
            crate::platform::show_window(&window)?;
            start_check(&window);
        }
        window
            .window()
            .with_winit_window(|native| native.focus_window());
        window.invoke_focus_dismiss();
        Ok(())
    }

    fn window(&self) -> Result<UpdateWindow, slint::PlatformError> {
        let mut slot = self.window.borrow_mut();
        if let Some(window) = slot.as_ref() {
            return Ok(window.clone_strong());
        }
        let window = UpdateWindow::new()?;
        crate::appearance_preferences::install_system_typography(&window);
        let weak = window.as_weak();
        window.on_retry(move || {
            if let Some(window) = weak.upgrade() {
                start_check(&window);
            }
        });
        let weak = window.as_weak();
        window.on_dismiss(move || {
            if let Some(window) = weak.upgrade() {
                let _ = window.hide();
            }
        });
        *slot = Some(window.clone_strong());
        Ok(window)
    }
}

fn start_check(window: &UpdateWindow) {
    if window.get_checking() {
        return;
    }
    window.set_checking(true);
    window.set_release_available(false);
    window.set_headline("Checking for updates…".into());
    window.set_detail(format!("Installed version: {}", env!("CARGO_PKG_VERSION")).into());
    let weak = window.as_weak();
    if let Err(error) = crate::background::spawn_named("app-release-check", move || {
        let result = releases::check();
        let _ = crate::background::to_ui(weak, move |window| present(&window, result));
    }) {
        present(window, Err(error.to_string()));
    }
}

fn present(window: &UpdateWindow, result: Result<releases::Outcome, String>) {
    window.set_checking(false);
    window.set_release_available(false);
    match result {
        Ok(releases::Outcome::Available(release)) => {
            window.set_headline(format!("Riina {} is available", release.version).into());
            window.set_detail(format!("Installed version: {}", env!("CARGO_PKG_VERSION")).into());
            let weak = window.as_weak();
            window.on_open_release(move || {
                if let Some(window) = weak.upgrade()
                    && window.get_release_available()
                    && let Err(error) = crate::platform::open_http_url_in_browser(&release.url)
                {
                    window.set_detail(format!("Could not open the release page: {error}").into());
                }
            });
            window.set_release_available(true);
        }
        Ok(releases::Outcome::Current(latest)) => {
            window.set_headline("No newer Riina release was found".into());
            window.set_detail(
                format!(
                    "Installed: {}. Latest release: {latest}.",
                    env!("CARGO_PKG_VERSION")
                )
                .into(),
            );
        }
        Err(error) => {
            window.set_headline("Could not check for updates".into());
            window.set_detail(error.into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semver::Version;
    use slint::platform::{Key, WindowEvent};

    #[test]
    fn update_window_reflects_each_result_and_escape_dismisses_it() {
        i_slint_backend_testing::init_no_event_loop();
        let window = ApplicationUpdates::default().window().unwrap();
        window.show().unwrap();
        window.invoke_focus_dismiss();

        present(
            &window,
            Ok(releases::Outcome::Available(releases::Release {
                version: Version::new(5, 0, 0),
                url: "https://github.com/deminden/riina/releases/tag/v5.0.0".into(),
            })),
        );
        assert!(window.get_release_available());
        assert_eq!(window.get_headline(), "Riina 5.0.0 is available");

        present(
            &window,
            Ok(releases::Outcome::Current(Version::new(4, 2, 1))),
        );
        assert!(!window.get_release_available());
        assert!(window.get_detail().contains("Latest release: 4.2.1"));

        window.set_checking(true);
        present(&window, Err("Test feed offline".into()));
        assert!(!window.get_checking());
        assert_eq!(window.get_detail(), "Test feed offline");
        window.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Escape.into(),
        });
        assert!(!window.window().is_visible());
    }
}
