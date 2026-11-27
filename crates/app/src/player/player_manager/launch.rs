//! Route AppKit document opens and idle launch actions into the player manager.
use super::{
    PlayerManager, document_open::ApplicationOpen, lifecycle::OpenWork, schedule_quit,
    schedule_welcome, show_welcome,
};
use crate::preference_load;
use riina_core::ActionAfterLaunch;
use slint::ComponentHandle;
use std::rc::Rc;

impl PlayerManager {
    pub(crate) fn prepare_application_plugins(&self, path: &std::path::Path) -> Result<(), String> {
        let plugins = self.services().plugins;
        let global = plugins.host(path)?;
        let state = Rc::downgrade(&self.state);
        plugins.bind_windowless(global.clone(), move || {
            state.upgrade().is_some_and(|state| {
                let state = state.borrow();
                !state.quitting && !state.has_open_players()
            })
        });
        global.load_enabled_globals()
    }

    pub(crate) fn is_quitting(&self) -> bool {
        self.state.borrow().quitting
    }

    pub(crate) fn is_empty(&self) -> bool {
        !self.state.borrow().has_open_players()
    }

    pub(crate) fn request_quit(&self) {
        schedule_quit(Rc::downgrade(&self.state));
    }

    pub(crate) fn request_welcome(&self) {
        schedule_welcome(Rc::downgrade(&self.state), None);
    }

    pub(crate) fn launch_action(&self, action: ActionAfterLaunch) {
        if self.is_quitting() {
            return;
        }
        match action {
            ActionAfterLaunch::WelcomeWindow => {
                if let Err(error) = show_welcome(&self.state, None) {
                    crate::diagnostics::warn!("Could not open Welcome: {error}");
                }
            }
            ActionAfterLaunch::OpenPanel => self.open_application_panel(false),
            ActionAfterLaunch::None => {}
        }
    }

    pub(crate) fn reopen(&self) {
        if self.state.borrow().lifecycle.opening() {
            return;
        }
        if let Ok(path) = crate::platform::preferences_path() {
            match preference_load::load(&path) {
                Ok(loaded) => self.launch_action(loaded.preferences.action_after_launch),
                Err(error) => {
                    crate::diagnostics::warn!("Could not read the launch action: {error}")
                }
            }
        }
    }

    pub(crate) fn show_application_open_url(&self) {
        if self.is_quitting() {
            return;
        }
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let loaded = preference_load::load(&crate::platform::preferences_path()?)?;
            self.services()
                .open_url
                .show_from_application(loaded.preferences.theme_material)?;
            Ok(())
        })();
        if let Err(error) = result {
            crate::diagnostics::warn!("Could not show Open URL: {error}");
        }
    }

    pub(crate) fn show_application_logs(&self) {
        if self.is_quitting() {
            return;
        }
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let path = crate::platform::preferences_path()?;
            let loaded = preference_load::load(&path)?;
            self.services()
                .show_application_logs(&path, loaded.preferences.advanced.log_level)?;
            Ok(())
        })();
        if let Err(error) = result {
            crate::diagnostics::warn!("Could not show Log Viewer: {error}");
        }
    }

    pub(crate) fn show_application_updates(&self) {
        if self.is_quitting() {
            return;
        }
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let path = crate::platform::preferences_path()?;
            let loaded = preference_load::load(&path)?;
            self.services()
                .show_application_updates(&path, &loaded.preferences)?;
            Ok(())
        })();
        if let Err(error) = result {
            crate::diagnostics::warn!("Could not open Software Update: {error}");
        }
    }

    pub(crate) fn show_application_about(&self) {
        if self.is_quitting() {
            return;
        }
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let path = crate::platform::preferences_path()?;
            let loaded = preference_load::load(&path)?;
            crate::platform::application_lifecycle::show_about(
                &self.services(),
                &loaded.preferences,
            )?;
            Ok(())
        })();
        if let Err(error) = result {
            crate::diagnostics::warn!("Could not show About Riina: {error}");
        }
    }

    pub(crate) fn show_application_highlights(&self) {
        if self.is_quitting() {
            return;
        }
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let loaded = preference_load::load(&crate::platform::preferences_path()?)?;
            crate::platform::application_lifecycle::show_highlights(
                &self.services(),
                &loaded.preferences,
            )?;
            Ok(())
        })();
        if let Err(error) = result {
            crate::diagnostics::warn!("Could not show Release Highlights: {error}");
        }
    }

    pub(crate) fn show_application_history(&self) {
        if self.is_quitting() {
            return;
        }
        let result = crate::platform::preferences_path()
            .map_err(|error| error.to_string())
            .and_then(|path| self.services().history.show_from_application(path));
        if let Err(error) = result {
            crate::diagnostics::warn!("Could not open Playback History: {error}");
        }
    }

    pub(crate) fn show_application_settings(&self, pane: Option<i32>) {
        if self.is_quitting() {
            return;
        }
        if let Some((app, _)) = self.services().history.browser_destination() {
            // Welcome can own a configured player even before the first file.
            let previous_pane = app.get_preferences_target_pane();
            if let Some(pane) = pane {
                app.set_preferences_target_pane(pane);
            }
            app.global::<riina_ui::PlayerMenuActions>()
                .invoke_open_preferences();
            app.set_preferences_target_pane(previous_pane);
            return;
        }
        if let Err(error) = self.open_application_settings(pane) {
            crate::diagnostics::warn!("Could not open Settings: {error}");
        }
    }

    fn open_application_settings(&self, pane: Option<i32>) -> Result<(), String> {
        let path = crate::platform::preferences_path().map_err(|error| error.to_string())?;
        let loaded = preference_load::load(&path).map_err(|error| error.to_string())?;
        self.services()
            .show_application_settings(&path, loaded, pane)
    }

    pub(crate) fn clear_application_recents(&self) {
        if self.is_quitting() {
            return;
        }
        let result = (|| -> Result<(), String> {
            let path = crate::platform::preferences_path().map_err(|error| error.to_string())?;
            let loaded = preference_load::load(&path).map_err(|error| error.to_string())?;
            self.services()
                .clear_application_documents(&path, &loaded.preferences)
        })();
        if let Err(error) = result {
            crate::diagnostics::warn!("{error}");
        }
    }

    pub(crate) fn open_application_panel(&self, alternative: bool) {
        let Some(pending) = self.begin_open(OpenWork::FilePicker) else {
            return;
        };
        let manager = self.clone();
        let started = slint::spawn_local(async move {
            let _pending = pending;
            if manager.is_quitting() {
                return;
            }
            let files = crate::platform::choose_application_media("Open File", None).await;
            match files {
                Ok(Some(paths)) => {
                    manager
                        .open_application_selection(paths, ApplicationOpen::Panel { alternative });
                }
                Ok(None) => {}
                Err(error) => crate::diagnostics::warn!("Could not open the file picker: {error}"),
            }
        });
        if let Err(error) = started {
            crate::diagnostics::warn!("Could not schedule Open File: {error}");
        }
    }

    pub(crate) fn open_application_documents(&self, paths: Vec<String>) {
        if self.is_quitting() {
            return;
        }
        if let Some(archive) = crate::plugin_packages::first_archive(&paths) {
            if let Some((app, _)) = self.services().history.browser_destination()
                && app.invoke_open_plugin_archive(archive.into())
            {
                return;
            }
            if self.is_empty() {
                crate::platform::browser_activation::show_idle_menu();
            }
            let result = self
                .open_application_settings(Some(
                    crate::platform::application_settings::PLUGINS_PANE,
                ))
                .and_then(|()| self.services().open_plugin_archive(archive.to_owned()));
            if let Err(error) = result {
                crate::diagnostics::warn!("Could not open the plugin package: {error}");
            }
            return;
        }
        self.open_application_selection(paths, ApplicationOpen::Desktop);
    }

    pub(crate) fn open_application_recent(&self, path: String) {
        self.open_application_selection(vec![path], ApplicationOpen::Recent);
    }
}
