use super::ApplicationPower;
use crate::{PlayerWindow, PreferencesWindow};
use riina_core::{Preferences, playback_power::Settings};
use riina_ui::{GeneralSettingsState, PlaybackPowerPreference};
use slint::ComponentHandle;
use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

impl ApplicationPower {
    pub(crate) fn connect_settings(
        self: &Rc<Self>,
        window: &PreferencesWindow,
        owner: &PlayerWindow,
        path: &Path,
        baseline: &Preferences,
    ) {
        self.windows.borrow_mut().push(window.as_weak());
        let power = Rc::downgrade(self);
        let window_weak = window.as_weak();
        let owner_weak = owner.as_weak();
        let path: PathBuf = path.to_owned();
        let baseline = baseline.clone();
        window
            .global::<GeneralSettingsState>()
            .on_playback_power_setting_changed(move |field| {
                let (Some(power), Some(window)) = (power.upgrade(), window_weak.upgrade()) else {
                    return;
                };
                if let Err(error) = power.save_setting(&window, &path, &baseline, field) {
                    power.refresh_settings(&window);
                    if let Some(owner) = owner_weak.upgrade() {
                        crate::diagnostics::report(
                            &owner,
                            format!("Could not save playback power preference: {error}"),
                        );
                    }
                }
            });
    }

    pub(crate) fn refresh_settings(&self, window: &PreferencesWindow) {
        project(window, self.settings.get());
    }

    fn save_setting(
        &self,
        window: &PreferencesWindow,
        path: &Path,
        baseline: &Preferences,
        field: PlaybackPowerPreference,
    ) -> Result<(), String> {
        let controls = window.global::<GeneralSettingsState>();
        let mut saved = self.settings.get();
        crate::preference_store::update(path, baseline, |preferences| {
            match field {
                PlaybackPowerPreference::PreventScreenSaver => {
                    preferences.playback_power.prevent_screen_saver =
                        controls.get_prevent_screen_saver();
                }
                PlaybackPowerPreference::AllowAudio => {
                    preferences.playback_power.allow_screen_saver_for_audio =
                        controls.get_allow_screen_saver_for_audio();
                }
            }
            saved = preferences.playback_power;
            Ok(())
        })?;
        self.settings.set(saved);
        self.windows.borrow_mut().retain(|weak| {
            let Some(window) = weak.upgrade() else {
                return false;
            };
            project(&window, saved);
            true
        });
        self.reconcile();
        Ok(())
    }
}

fn project(window: &PreferencesWindow, settings: Settings) {
    let controls = window.global::<GeneralSettingsState>();
    controls.set_prevent_screen_saver(settings.prevent_screen_saver);
    controls.set_allow_screen_saver_for_audio(settings.allow_screen_saver_for_audio);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_power_settings_refresh_other_open_windows_and_preserve_other_preferences() {
        i_slint_backend_testing::init_no_event_loop();
        let directory = riina_test_support::TempDir::new("power-preferences").unwrap();
        let path = directory.path().join("preferences.json");
        let baseline = Preferences {
            volume: 42.0,
            ..Preferences::default()
        };
        let app = PlayerWindow::new().unwrap();
        let first = PreferencesWindow::new().unwrap();
        let other = PreferencesWindow::new().unwrap();
        let service = Rc::new(ApplicationPower::default());
        service.connect_settings(&first, &app, &path, &baseline);
        service.connect_settings(&other, &app, &path, &baseline);
        let first = first.global::<GeneralSettingsState>();
        let other = other.global::<GeneralSettingsState>();

        first.set_allow_screen_saver_for_audio(true);
        first.invoke_playback_power_setting_changed(PlaybackPowerPreference::AllowAudio);
        assert!(other.get_allow_screen_saver_for_audio());
        other.set_prevent_screen_saver(false);
        other.invoke_playback_power_setting_changed(PlaybackPowerPreference::PreventScreenSaver);
        assert!(!first.get_prevent_screen_saver());

        let saved = crate::preference_load::read_valid(&path).unwrap().unwrap();
        assert_eq!(saved.volume, 42.0);
        assert!(saved.playback_power.allow_screen_saver_for_audio);
        assert!(!saved.playback_power.prevent_screen_saver);
    }
}
