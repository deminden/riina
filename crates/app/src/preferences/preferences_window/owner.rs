//! The real Settings window owns dialogs. A player, when present, follows its modality.
use crate::{PlayerWindow, PreferencesWindow};
use riina_ui::{GeneralSettingsState, PlayerControlActions, PlayerCropState, PlayerDialogState};
use slint::{ComponentHandle, Weak};

#[derive(Clone)]
pub(super) struct Owner {
    window: Weak<PreferencesWindow>,
    player: Option<Weak<PlayerWindow>>,
}

impl Owner {
    pub(super) fn new(window: &PreferencesWindow, player: Option<&PlayerWindow>) -> Self {
        Self {
            window: window.as_weak(),
            player: player.map(ComponentHandle::as_weak),
        }
    }

    pub(super) fn report(&self, error: String) {
        if let Some(player) = self.player.as_ref().and_then(Weak::upgrade) {
            player.set_status(error.into());
        } else if let Some(window) = self.window.upgrade() {
            crate::diagnostics::warn!("{error}");
            window.set_profile_status(error.into());
        }
    }

    pub(super) fn begin_dialog(&self, window: &PreferencesWindow) -> Option<Dialog> {
        let general = window.global::<GeneralSettingsState>();
        if general.get_folder_dialog_open() {
            return None;
        }
        if let Some(player) = self.player.as_ref() {
            let player = player.upgrade()?;
            let dialogs = player.global::<PlayerDialogState>();
            if dialogs.get_dialog_open() || player.global::<PlayerCropState>().get_crop_editing() {
                return None;
            }
            dialogs.set_dialog_open(true);
            player.global::<PlayerControlActions>().invoke_activity();
        }
        general.set_folder_dialog_open(true);
        Some(Dialog(self.clone()))
    }
}

pub(super) struct Dialog(Owner);

impl Drop for Dialog {
    fn drop(&mut self) {
        if let Some(window) = self.0.window.upgrade() {
            window
                .global::<GeneralSettingsState>()
                .set_folder_dialog_open(false);
        }
        if let Some(player) = self.0.player.as_ref().and_then(Weak::upgrade) {
            player.global::<PlayerDialogState>().set_dialog_open(false);
            player.global::<PlayerControlActions>().invoke_activity();
        }
    }
}
