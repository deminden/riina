//! Route updater settings directly to the application service.
use super::{
    ApplicationUpdates,
    schedule::{Interval, Permission, State},
};
use crate::PreferencesWindow;
use riina_ui::{GeneralSettingsState, UpdatePreference};
use slint::{ComponentHandle, Weak};
use std::{io, rc::Rc};

impl ApplicationUpdates {
    pub(crate) fn connect_settings(
        self: &Rc<Self>,
        window: &PreferencesWindow,
        report: impl Fn(String) + 'static,
    ) {
        self.settings.borrow_mut().push(window.as_weak());
        let weak = window.as_weak();
        let updates = Rc::downgrade(self);
        window
            .global::<GeneralSettingsState>()
            .on_update_setting_changed(move |field| {
                if let (Some(updates), Some(window)) = (updates.upgrade(), weak.upgrade())
                    && let Err(error) = updates.save_setting(&window, field)
                {
                    updates.refresh_settings(&window);
                    report(format!("Could not save the update preference: {error}"));
                }
            });
    }

    pub(crate) fn refresh_settings(&self, window: &PreferencesWindow) {
        project(window, self.state().unwrap_or_default());
    }

    fn save_setting(
        self: &Rc<Self>,
        window: &PreferencesWindow,
        field: UpdatePreference,
    ) -> io::Result<()> {
        let controls = window.global::<GeneralSettingsState>();
        match field {
            UpdatePreference::Automatic => {
                self.set_permission(controls.get_automatic_update_checks())?;
                if let Some(window) = self.window.borrow().as_ref()
                    && window.get_asking_permission()
                {
                    window.set_asking_permission(false);
                    let _ = window.hide();
                }
                Ok(())
            }
            UpdatePreference::Interval => {
                let interval = usize::try_from(controls.get_update_interval_index())
                    .ok()
                    .and_then(|index| Interval::ALL.get(index).copied())
                    .ok_or_else(|| io::Error::other("Unknown update interval"))?;
                self.apply_setting(|state| state.interval = interval)
            }
            UpdatePreference::Prereleases => {
                let include = controls.get_receive_prerelease_updates();
                self.apply_setting(|state| state.include_prereleases = include)
            }
        }
    }

    pub(super) fn set_permission(self: &Rc<Self>, enabled: bool) -> io::Result<()> {
        self.apply_setting(|state| {
            state.permission = if enabled {
                Permission::Enabled
            } else {
                Permission::Disabled
            };
        })
    }

    fn apply_setting(self: &Rc<Self>, edit: impl FnOnce(&mut State)) -> io::Result<()> {
        let saved = self.change(|state| {
            edit(state);
            *state
        })?;
        if saved.permission != Permission::Undecided {
            self.consent_due.set(false);
        }
        self.settings.borrow_mut().retain(|weak| {
            let Some(window) = Weak::upgrade(weak) else {
                return false;
            };
            project(&window, saved);
            true
        });
        self.refresh_schedule();
        Ok(())
    }
}

fn project(window: &PreferencesWindow, state: State) {
    let controls = window.global::<GeneralSettingsState>();
    controls.set_automatic_update_checks(state.permission == Permission::Enabled);
    controls.set_update_interval_index(state.interval.index());
    controls.set_receive_prerelease_updates(state.include_prereleases);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::updates::schedule::Store;

    #[test]
    fn editing_one_update_field_refreshes_all_settings_windows() {
        i_slint_backend_testing::init_no_event_loop();
        let profile = riina_test_support::TempDir::new("application-update-preferences").unwrap();
        let path = profile.path().join("preferences.json");
        let updates = Rc::new(ApplicationUpdates::default());
        assert!(updates.store.set(Store::for_preferences(&path)).is_ok());
        let first = PreferencesWindow::new().unwrap();
        let second = PreferencesWindow::new().unwrap();
        updates.connect_settings(&first, |error| panic!("{error}"));
        updates.connect_settings(&second, |error| panic!("{error}"));
        updates.refresh_settings(&first);
        updates.refresh_settings(&second);

        let first = first.global::<GeneralSettingsState>();
        let second = second.global::<GeneralSettingsState>();
        first.set_receive_prerelease_updates(true);
        first.invoke_update_setting_changed(UpdatePreference::Prereleases);
        second.set_update_interval_index(Interval::Weekly.index());
        second.invoke_update_setting_changed(UpdatePreference::Interval);
        assert!(second.get_receive_prerelease_updates());
        assert_eq!(first.get_update_interval_index(), Interval::Weekly.index());
        let saved = Store::for_preferences(&path).read().unwrap();
        assert!(saved.include_prereleases);
        assert_eq!(saved.interval, Interval::Weekly);
        assert_eq!(saved.permission, Permission::Undecided);
    }
}
