//! Coalesce menu and scheduled checks and deliver their results on the UI thread.
use super::{
    ApplicationUpdates, present, releases,
    schedule::{self, Channel, Permission},
};
use crate::UpdateWindow;
use slint::ComponentHandle;
use std::rc::Rc;
use tokio::sync::oneshot;

#[derive(Clone, Copy)]
pub(super) struct Running {
    channel: Channel,
    manual: bool,
}

impl Running {
    pub(super) fn automatic(channel: Channel) -> Self {
        Self {
            channel,
            manual: false,
        }
    }
}

impl ApplicationUpdates {
    pub(super) fn request_manual(self: &Rc<Self>, window: &UpdateWindow) {
        window.set_asking_permission(false);
        window.set_checking(true);
        window.set_release_available(false);
        window.set_headline("Checking for updates…".into());
        window.set_detail(format!("Installed version: {}", env!("CARGO_PKG_VERSION")).into());
        if let Some(mut running) = self.running.get() {
            running.manual = true;
            self.running.set(Some(running));
            return;
        }
        let channel = self.state().unwrap_or_default().channel();
        if let Err(error) = self.change(|state| state.mark_attempt(channel, schedule::now())) {
            crate::diagnostics::warn!("Could not record manual update check: {error}");
        }
        self.start(Running {
            channel,
            manual: true,
        });
    }

    pub(super) fn start(self: &Rc<Self>, running: Running) {
        self.timer.stop();
        self.running.set(Some(running));
        let (send, response) = oneshot::channel();
        if let Err(error) = crate::background::spawn_named("app-release-check", move || {
            let _ = send.send(releases::check(running.channel));
        }) {
            self.finish(Err(error.to_string()));
            return;
        }
        let updates = Rc::downgrade(self);
        if let Err(error) = slint::spawn_local(async move {
            let result = response
                .await
                .unwrap_or_else(|_| Err("The update check ended early.".into()));
            if let Some(updates) = updates.upgrade() {
                updates.finish(result);
            }
        }) {
            self.finish(Err(error.to_string()));
        }
    }

    fn finish(self: &Rc<Self>, result: Result<releases::Outcome, String>) {
        let Some(running) = self.running.take() else {
            return;
        };
        let state = self.state().unwrap_or_default();
        if running.channel != state.channel() {
            if running.manual
                && let Ok(window) = self.window()
                && window.window().is_visible()
            {
                self.request_manual(&window);
                return;
            }
        } else if running.manual
            || (state.permission == Permission::Enabled
                && matches!(result, Ok(releases::Outcome::Available(_))))
        {
            self.show_result(result, running.manual);
        }
        self.refresh_schedule();
    }

    fn show_result(self: &Rc<Self>, result: Result<releases::Outcome, String>, manual: bool) {
        if self.install_in_progress() {
            return;
        }
        let window = match self.window() {
            Ok(window) => window,
            Err(error) => {
                crate::diagnostics::warn!("Could not open Software Update: {error}");
                return;
            }
        };
        present(self, &window, result);
        if !manual && let Err(error) = self.reveal(&window, self.owner().as_ref()) {
            crate::diagnostics::warn!("Could not show the available update: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::updates::schedule::Store;
    use riina_core::{Preferences, ThemeMaterial};
    use semver::Version;

    #[test]
    fn idle_update_schedule_prompts_once_and_manual_check_reuses_the_window()
    -> Result<(), Box<dyn std::error::Error>> {
        i_slint_backend_testing::init_no_event_loop();
        let profile = riina_test_support::TempDir::new("idle-update-schedule")?;
        let path = profile.path().join("preferences.json");
        let preferences = Preferences {
            theme_material: ThemeMaterial::Dark,
            ..Preferences::default()
        };
        let first_launch = Rc::new(ApplicationUpdates::default());
        first_launch.configure_application(&path, &preferences);
        first_launch.configure_application(&path, &preferences);
        assert!(!first_launch.timer.running());
        assert!(first_launch.window.borrow().is_none());

        let updates = Rc::new(ApplicationUpdates::default());
        updates.configure_application(&path, &preferences);
        assert!(updates.timer.running());
        i_slint_backend_testing::mock_elapsed_time(super::super::STARTUP_DELAY);
        let window = updates.window()?;
        assert!(window.window().is_visible());
        assert!(window.get_dark_theme());
        assert!(window.get_asking_permission());
        window.invoke_set_permission(false);
        assert_eq!(updates.state()?.permission, Permission::Disabled);
        assert!(!window.window().is_visible());
        assert!(!updates.timer.running());

        // Provide the in-flight result locally so the test never contacts the release service.
        updates
            .running
            .set(Some(Running::automatic(Channel::Stable)));
        updates.show_from_application()?;
        assert!(window.window().is_visible());
        assert!(window.get_checking());
        assert!(updates.running.get().is_some_and(|running| running.manual));
        let player = crate::PlayerWindow::new()?;
        updates.connect(&player, None, &path);
        player
            .global::<riina_ui::PlayerMenuActions>()
            .invoke_check_for_updates();
        updates.finish(Ok(releases::Outcome::Current(Version::parse(env!(
            "CARGO_PKG_VERSION"
        ))?)));
        assert!(!window.get_checking());
        assert!(!window.get_release_available());
        assert_eq!(updates.state()?.permission, Permission::Disabled);
        window.invoke_dismiss();
        assert!(!window.window().is_visible());
        Ok(())
    }

    #[test]
    fn automatic_results_stay_hidden_unless_a_new_release_is_available() {
        i_slint_backend_testing::init_no_event_loop();
        let profile = riina_test_support::TempDir::new("scheduled-update-result").unwrap();
        let updates = Rc::new(ApplicationUpdates::default());
        assert!(
            updates
                .store
                .set(Store::for_preferences(
                    &profile.path().join("preferences.json")
                ))
                .is_ok()
        );
        updates
            .change(|state| state.permission = Permission::Enabled)
            .unwrap();

        let running = Running::automatic(Channel::Stable);
        updates.running.set(Some(running));
        updates.finish(Err("Offline".into()));
        assert!(updates.window.borrow().is_none());
        updates.running.set(Some(running));
        updates.finish(Ok(releases::Outcome::Current(Version::new(4, 2, 1))));
        assert!(updates.window.borrow().is_none());

        updates.running.set(Some(running));
        updates.finish(Ok(releases::Outcome::Available(releases::Release {
            version: Version::new(5, 0, 0),
            url: "https://github.com/deminden/riina/releases/tag/v5.0.0".into(),
            installer: None,
        })));
        let window = updates.window().unwrap();
        assert!(window.window().is_visible());
        assert!(window.get_release_available());
        window.hide().unwrap();

        updates.running.set(Some(running));
        updates.request_manual(&window);
        updates.finish(Err("Offline".into()));
        assert!(!window.window().is_visible());
        assert_eq!(window.get_detail(), "Offline");
    }
}
