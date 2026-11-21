//! Keep the Settings window and its native folder dialog in one modality scope.
use crate::{PlayerWindow, PreferencesWindow};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};

pub(super) fn register(app: &PlayerWindow, window: &PreferencesWindow) {
    let parent = app.as_weak();
    let weak = window.as_weak();
    window.on_choose_folder(move || {
        let (Some(app), Some(window)) = (parent.upgrade(), weak.upgrade()) else {
            return;
        };
        if app
            .global::<riina_ui::PlayerDialogState>()
            .get_dialog_open()
            || app.global::<riina_ui::PlayerCropState>().get_crop_editing()
        {
            return;
        }
        app.global::<riina_ui::PlayerDialogState>()
            .set_dialog_open(true);
        window
            .global::<riina_ui::GeneralSettingsState>()
            .set_folder_dialog_open(true);
        let parent = app.as_weak();
        let weak = window.as_weak();
        let failure_parent = parent.clone();
        let failure_window = weak.clone();
        let started = slint::spawn_local(async move {
            let (Some(app), Some(window)) = (parent.upgrade(), weak.upgrade()) else {
                return;
            };
            let result = async {
                let owner = window
                    .window()
                    .winit_window()
                    .await
                    .map_err(|error| error.to_string())?;
                let parent = parent.clone();
                let weak = weak.clone();
                crate::platform::folder_dialog::pick(owner, move |result| {
                    let (Some(app), Some(window)) = (parent.upgrade(), weak.upgrade()) else {
                        return;
                    };
                    app.global::<riina_ui::PlayerDialogState>()
                        .set_dialog_open(false);
                    window
                        .global::<riina_ui::GeneralSettingsState>()
                        .set_folder_dialog_open(false);
                    match result {
                        Ok(Some(directory)) => {
                            window
                                .global::<riina_ui::GeneralSettingsState>()
                                .set_directory(directory.into());
                            window.set_custom_directory(true);
                            window.invoke_settings_changed();
                        }
                        Ok(None) => {}
                        Err(error) => app.set_status(error.into()),
                    }
                })
            }
            .await;
            if let Err(error) = result {
                app.global::<riina_ui::PlayerDialogState>()
                    .set_dialog_open(false);
                window
                    .global::<riina_ui::GeneralSettingsState>()
                    .set_folder_dialog_open(false);
                app.set_status(error.into());
            }
        });
        if let Err(error) = started {
            if let Some(app) = failure_parent.upgrade() {
                app.global::<riina_ui::PlayerDialogState>()
                    .set_dialog_open(false);
                app.set_status(error.to_string().into());
            }
            if let Some(window) = failure_window.upgrade() {
                window
                    .global::<riina_ui::GeneralSettingsState>()
                    .set_folder_dialog_open(false);
            }
        }
    });
}
