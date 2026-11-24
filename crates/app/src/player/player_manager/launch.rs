//! Route AppKit document opens and idle launch actions into the player manager.
use super::{PlayerManager, open_new_player, schedule_quit, schedule_welcome, show_welcome};
use crate::{PlayerWindow, media_open, preference_load};
use riina_core::ActionAfterLaunch;
use slint::ComponentHandle;
use std::rc::Rc;

#[derive(Clone, Copy)]
enum ApplicationOpen {
    Finder,
    Panel { alternative: bool },
    Recent,
}

impl ApplicationOpen {
    fn alternative(self) -> bool {
        matches!(self, Self::Panel { alternative: true })
    }

    fn records_selection(self) -> bool {
        // Finder files follow the record-on-playback preference, while explicit
        // choices in the File menu count as Open Recent selections themselves.
        !matches!(self, Self::Finder)
    }
}

impl PlayerManager {
    pub(crate) fn is_quitting(&self) -> bool {
        self.state.borrow().quitting
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.state.borrow().players.is_empty()
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
        if self.state.borrow().opening_documents != 0 {
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
            self.services().open_url.show_from_application(
                loaded.preferences.effective_theme_material(),
                loaded.preferences.dark,
            )?;
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
            self.services()
                .show_application_about(&loaded.preferences)?;
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
            self.services()
                .show_application_highlights(&loaded.preferences)?;
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

    pub(crate) fn show_application_settings(&self) {
        if self.is_quitting() {
            return;
        }
        if let Some((app, _)) = self.services().history.browser_destination() {
            // Welcome can own a configured player even before the first file.
            app.global::<riina_ui::PlayerMenuActions>()
                .invoke_open_preferences();
            return;
        }
        let result = (|| -> Result<(), String> {
            let path = crate::platform::preferences_path().map_err(|error| error.to_string())?;
            let loaded = preference_load::load(&path).map_err(|error| error.to_string())?;
            self.services().show_application_settings(&path, loaded)
        })();
        if let Err(error) = result {
            crate::diagnostics::warn!("Could not open Settings: {error}");
        }
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
        {
            let mut state = self.state.borrow_mut();
            if state.quitting || state.open_panel_visible {
                return;
            }
            state.open_panel_visible = true;
        }
        let manager = self.clone();
        let started = slint::spawn_local(async move {
            if manager.is_quitting() {
                return;
            }
            let files = crate::platform::choose_application_media("Open File", None).await;
            manager.state.borrow_mut().open_panel_visible = false;
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
            self.state.borrow_mut().open_panel_visible = false;
            crate::diagnostics::warn!("Could not schedule Open File: {error}");
        }
    }

    pub(crate) fn open_application_documents(&self, paths: Vec<String>) {
        self.open_application_selection(paths, ApplicationOpen::Finder);
    }

    pub(crate) fn open_application_recent(&self, path: String) {
        self.open_application_selection(vec![path], ApplicationOpen::Recent);
    }

    fn open_application_selection(&self, paths: Vec<String>, origin: ApplicationOpen) {
        if paths.is_empty() || self.is_quitting() || self.open_with_existing_player(&paths, origin)
        {
            return;
        }
        let manager = self.clone();
        self.state.borrow_mut().opening_documents += 1;
        if let Err(error) = slint::spawn_local(async move {
            let result = manager.open_without_player(paths, origin).await;
            manager.state.borrow_mut().opening_documents -= 1;
            if let Err(error) = result {
                crate::diagnostics::warn!("Could not open the selected media: {error}");
            }
        }) {
            self.state.borrow_mut().opening_documents -= 1;
            crate::diagnostics::warn!("Could not schedule the selected media: {error}");
        }
    }

    fn open_with_existing_player(&self, paths: &[String], origin: ApplicationOpen) -> bool {
        let destination = self.services().history.browser_destination();
        let Some((app, display)) = destination else {
            return false;
        };
        let sender = self
            .state
            .borrow()
            .players
            .iter()
            .find(|player| std::ptr::eq(player.app.window(), app.window()))
            .and_then(|player| player.session.player())
            .map(riina_mpv::Player::command_sender);
        let Some(sender) = sender else { return false };
        let result = display.reveal(&app).map(|()| {
            display.focus(&app);
            if origin.records_selection() {
                for path in paths {
                    app.invoke_remember_recent_path(path.clone().into());
                }
            }
            media_open::file_dialog_selection(&app, paths.to_vec(), origin.alternative(), sender);
        });
        if let Err(error) = result {
            crate::diagnostics::report(&app, error.to_string());
        }
        true
    }

    async fn open_without_player(
        &self,
        paths: Vec<String>,
        origin: ApplicationOpen,
    ) -> Result<(), String> {
        if self.is_quitting() || self.open_with_existing_player(&paths, origin) {
            return Ok(());
        }
        let path = crate::platform::preferences_path().map_err(|error| error.to_string())?;
        let loaded = preference_load::load(&path).map_err(|error| error.to_string())?;
        if origin.records_selection() {
            self.services()
                .record_application_documents(&path, &loaded.preferences, &paths);
        }
        let preferences = loaded.preferences.clone();
        let (ready, selection) = tokio::sync::oneshot::channel();
        crate::background::spawn_named("application-media-selection", move || {
            let _ = ready.send(media_open::prepare_application_documents(
                paths,
                &preferences,
                origin.alternative(),
            ));
        })
        .map_err(|error| error.to_string())?;
        let players = selection.await.map_err(|error| error.to_string())??;
        if self.is_quitting() {
            return Ok(());
        }
        for media in players {
            open_new_player::<PlayerWindow>(&self.state, None, media, loaded.clone())?;
        }
        Ok(())
    }
}
