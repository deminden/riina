//! Run folder pickers under the actual Settings window's modality.
use super::owner::Owner;
use crate::{AdvancedSettingsState, PreferencesWindow};
use riina_ui::GeneralSettingsState;
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};

pub(super) fn register(window: &PreferencesWindow, owner: &Owner) {
    let screenshot = chooser(window, owner, |window, path| {
        window
            .global::<GeneralSettingsState>()
            .set_directory(path.into());
        window.set_custom_directory(true);
    });
    window.on_choose_folder(screenshot);
    let config = chooser(window, owner, |window, path| {
        let advanced = window.global::<AdvancedSettingsState>();
        advanced.set_advanced_config_directory_text(path.into());
        advanced.set_advanced_use_config_directory(true);
    });
    window.on_advanced_choose_config_directory(config);
}

fn chooser(
    window: &PreferencesWindow,
    owner: &Owner,
    select: fn(&PreferencesWindow, String),
) -> impl Fn() + 'static {
    let weak = window.as_weak();
    let owner = owner.clone();
    move || {
        let Some(window) = weak.upgrade() else { return };
        let Some(dialog) = owner.begin_dialog(&window) else {
            return;
        };
        let weak = weak.clone();
        let report = owner.clone();
        let started = slint::spawn_local(async move {
            let native = match window.window().winit_window().await {
                Ok(native) => native,
                Err(error) => {
                    report.report(error.to_string());
                    return;
                }
            };
            let complete = report.clone();
            if let Err(error) = crate::platform::folder_dialog::pick(native, move |result| {
                drop(dialog);
                match result {
                    Ok(Some(path)) => {
                        if let Some(window) = weak.upgrade() {
                            select(&window, path);
                            window.invoke_settings_changed();
                        }
                    }
                    Ok(None) => {}
                    Err(error) => complete.report(error),
                }
            }) {
                report.report(error);
            }
        });
        if let Err(error) = started {
            owner.report(error.to_string());
        }
    }
}
