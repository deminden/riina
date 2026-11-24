//! Route AppKit document opens and idle launch actions into the player manager.
use super::{PlayerManager, open_new_player, schedule_quit, schedule_welcome, show_welcome};
use crate::{PlayerWindow, media_open, preference_load, ui_dialogs};
use riina_core::ActionAfterLaunch;
use slint::ComponentHandle;
use std::rc::Rc;

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
            ActionAfterLaunch::OpenPanel => self.open_application_panel(),
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

    pub(crate) fn open_application_panel(&self) {
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
                Ok(Some(paths)) => manager.open_application_documents(paths),
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
        if paths.is_empty() || self.is_quitting() || self.open_with_existing_player(&paths) {
            return;
        }
        let manager = self.clone();
        self.state.borrow_mut().opening_documents += 1;
        if let Err(error) = slint::spawn_local(async move {
            let result = manager.open_without_player(paths).await;
            manager.state.borrow_mut().opening_documents -= 1;
            if let Err(error) = result {
                crate::diagnostics::warn!("Could not open Finder documents: {error}");
            }
        }) {
            self.state.borrow_mut().opening_documents -= 1;
            crate::diagnostics::warn!("Could not schedule Finder documents: {error}");
        }
    }

    fn open_with_existing_player(&self, paths: &[String]) -> bool {
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
        let result = display
            .reveal(&app)
            .map_err(|error| error.to_string())
            .and_then(|()| {
                display.focus(&app);
                ui_dialogs::dispatch_selection(
                    &app,
                    ui_dialogs::Purpose::Media,
                    paths.to_vec(),
                    sender,
                )
            });
        if let Err(error) = result {
            crate::diagnostics::report(&app, error);
        }
        true
    }

    async fn open_without_player(&self, paths: Vec<String>) -> Result<(), String> {
        if self.is_quitting() || self.open_with_existing_player(&paths) {
            return Ok(());
        }
        let path = crate::platform::preferences_path().map_err(|error| error.to_string())?;
        let loaded = preference_load::load(&path).map_err(|error| error.to_string())?;
        let preferences = loaded.preferences.clone();
        let (ready, selection) = tokio::sync::oneshot::channel();
        crate::background::spawn_named("finder-media-selection", move || {
            let _ = ready.send(media_open::prepare_application_documents(
                paths,
                &preferences,
            ));
        })
        .map_err(|error| error.to_string())?;
        let players = selection.await.map_err(|error| error.to_string())??;
        if self.is_quitting() {
            return Ok(());
        }
        let owner = PlayerWindow::new().map_err(|error| error.to_string())?;
        for media in players {
            open_new_player(&self.state, &owner, media, loaded.clone())?;
        }
        Ok(())
    }
}
