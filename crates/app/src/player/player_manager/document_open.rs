//! Route file manager and application selections through the current player policy.
use super::{
    ManagerState, PlayerManager, existing_player, finish_after_auxiliary_close,
    lifecycle::OpenWork, open_new_player, show_and_focus,
};
use crate::{PlayerWindow, media_open, preference_load};
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

pub(crate) struct OpenGuard {
    state: Weak<RefCell<ManagerState>>,
    work: OpenWork,
}

impl Drop for OpenGuard {
    fn drop(&mut self) {
        let Some(owner) = self.state.upgrade() else {
            return;
        };
        let reconsider = {
            let mut owner = owner.borrow_mut();
            let deferred = owner.lifecycle.finish_open(self.work);
            // The Linux application can start with a document and no windows yet.
            let cold_open_finished = cfg!(target_os = "linux") && !owner.lifecycle.opening();
            !owner.quitting && (deferred || cold_open_finished)
        };
        if reconsider {
            let state = self.state.clone();
            // Finish after the shell callbacks and any file picker return.
            if let Err(error) = slint::spawn_local(async move {
                if let Some(state) = state.upgrade() {
                    finish_after_auxiliary_close(&state);
                }
            }) {
                crate::diagnostics::warn!("Could not reconsider closing the application: {error}");
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum ApplicationOpen {
    Desktop,
    #[cfg(target_os = "macos")]
    Panel {
        alternative: bool,
    },
    #[cfg(target_os = "macos")]
    Recent,
}

impl ApplicationOpen {
    fn alternative(self) -> bool {
        match self {
            #[cfg(target_os = "macos")]
            Self::Panel { alternative } => alternative,
            _ => false,
        }
    }

    fn records_selection(self) -> bool {
        // Finder files follow the record-on-playback preference, while explicit
        // choices in the File menu count as Open Recent selections themselves.
        !matches!(self, Self::Desktop)
    }
}

impl PlayerManager {
    pub(crate) fn hold_document_open(&self) -> Option<OpenGuard> {
        self.begin_open(OpenWork::Documents)
    }

    pub(super) fn begin_open(&self, work: OpenWork) -> Option<OpenGuard> {
        let mut owner = self.state.borrow_mut();
        (!owner.quitting && owner.lifecycle.begin_open(work)).then(|| OpenGuard {
            state: Rc::downgrade(&self.state),
            work,
        })
    }

    pub(super) fn open_application_selection(&self, paths: Vec<String>, origin: ApplicationOpen) {
        if paths.is_empty()
            || self.state.borrow().quitting
            || self.open_with_existing_player(&paths, origin)
        {
            return;
        }
        let manager = self.clone();
        let Some(pending) = self.hold_document_open() else {
            return;
        };
        if let Err(error) = slint::spawn_local(async move {
            let _pending = pending;
            if let Err(error) = manager.open_without_player(paths, origin).await {
                crate::diagnostics::warn!("Could not open the selected media: {error}");
            }
        }) {
            crate::diagnostics::warn!("Could not schedule the selected media: {error}");
        }
    }

    fn document_target(
        &self,
    ) -> Option<(
        PlayerWindow,
        crate::player::surface::PlayerDisplay,
        riina_mpv::CommandSender,
    )> {
        let (app, display) = self.services().history.browser_destination()?;
        let sender = self
            .state
            .borrow()
            .players
            .iter()
            .find(|player| std::ptr::eq(player.app.window(), app.window()))
            .and_then(|player| player.session.player())
            .map(riina_mpv::Player::command_sender)?;
        Some((app, display, sender))
    }

    fn open_with_existing_player(&self, paths: &[String], origin: ApplicationOpen) -> bool {
        let Some((app, display, sender)) = self.document_target() else {
            return false;
        };
        let result = display.reveal(&app).map(|()| {
            display.focus(&app);
            if origin.records_selection() {
                crate::ui_dialogs::remember_selected_paths(&app, paths);
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
        if self.state.borrow().quitting || self.open_with_existing_player(&paths, origin) {
            return Ok(());
        }
        let path = crate::platform::preferences_path().map_err(|error| error.to_string())?;
        let loaded = preference_load::load(&path).map_err(|error| error.to_string())?;
        #[cfg(target_os = "macos")]
        if origin.records_selection() {
            self.services()
                .record_application_documents(&path, &loaded.preferences, &paths);
        }
        let preferences = loaded.preferences.clone();
        let into_current = preferences.allow_multiple_windows == origin.alternative();
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
        if self.state.borrow().quitting {
            return Ok(());
        }
        if into_current
            && let [selection] = players.as_slice()
            && let Some((app, display, sender)) = self.document_target()
        {
            display.reveal(&app).map_err(|error| error.to_string())?;
            display.focus(&app);
            return media_open::prepared_documents(&app, selection.paths.clone(), &sender);
        }
        for selection in players {
            if let Some(reference) = selection.duplicate_reference.as_deref()
                && let Some(player) = existing_player(&self.state, reference, &loaded.preferences)
            {
                show_and_focus(&player, &player);
                continue;
            }
            let media = crate::player_ui::InitialMedia {
                files: selection
                    .paths
                    .into_iter()
                    .map(std::ffi::OsString::from)
                    .collect(),
                ..Default::default()
            };
            open_new_player::<PlayerWindow>(&self.state, None, media, loaded.clone())?;
        }
        Ok(())
    }
}
