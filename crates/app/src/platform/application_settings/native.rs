//! An application-owned Settings window that is usable before playback starts.
mod fields;
use crate::preferences_window::profile_window;
use crate::{PreferencesWindow, windows::close_observer::CloseObserver};
use riina_core::{
    Preferences, audio_preferences::ReplayGainOptions, osc_preferences::ControllerOptions,
};
use slint::{CloseRequestResponse, ComponentHandle, winit_030::WinitWindowAccessor};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
};

#[derive(Default)]
pub(crate) struct ApplicationSettings {
    window: RefCell<Option<PreferencesWindow>>,
    pub(crate) closed: CloseObserver,
}

pub(crate) struct ProfileChanges {
    pub(crate) controller: Option<ControllerOptions>,
    pub(crate) replay_gain: Option<ReplayGainOptions>,
    pub(crate) subtitle_font: Option<String>,
    #[cfg(target_os = "macos")]
    pub(crate) subtitle_border_style: Option<riina_core::subtitle_controls::BorderStyle>,
    #[cfg(target_os = "macos")]
    pub(crate) subtitle_auto_load: Option<riina_core::subtitle_controls::AutoLoadPreferences>,
    pub(crate) key_bindings: Option<riina_core::key_bindings::Settings>,
}

struct Profile {
    path: PathBuf,
    saved: Preferences,
    displayed: Preferences,
    credentials: Credentials,
}

type StoreCredential = fn(&str) -> Result<(), String>;

struct Credentials {
    open_subtitles: StoreCredential,
    assrt: StoreCredential,
}

impl Credentials {
    #[cfg(target_os = "macos")]
    fn system() -> Self {
        Self {
            open_subtitles: crate::online_subtitles_credentials::store,
            assrt: crate::online_subtitles_credentials::store_assrt,
        }
    }
}

impl Profile {
    fn connect(
        window: &PreferencesWindow,
        path: PathBuf,
        saved: Preferences,
        credentials: Credentials,
        committed: impl Fn(ProfileChanges) + 'static,
    ) -> Result<(), String> {
        let displayed = fields::read(window, &saved)?;
        let state = Rc::new(RefCell::new(Self {
            path,
            saved,
            displayed,
            credentials,
        }));
        let adopted = Rc::clone(&state);
        let weak = window.as_weak();
        window.on_controller_options_adopted(move || {
            if let Some(window) = weak.upgrade()
                && let Ok(options) = crate::osc_preferences::form_controller_options(&window)
            {
                let mut state = adopted.borrow_mut();
                state.displayed.control_bar.controller = options;
                state.saved.control_bar.controller = options;
            }
        });
        let weak = window.as_weak();
        window.on_settings_changed(move || {
            let Some(window) = weak.upgrade() else { return };
            if window.get_font_picker_open() {
                return;
            }
            let result = state.borrow_mut().save(&window, &committed);
            match result {
                Ok(()) => window.set_profile_status("".into()),
                Err(error) => report(&window, error),
            }
        });
        Ok(())
    }

    fn save(
        &mut self,
        window: &PreferencesWindow,
        committed: impl FnOnce(ProfileChanges),
    ) -> Result<(), String> {
        let edit = fields::read(window, &self.displayed).and_then(|edit| {
            crate::preference_store::save_form_edit(
                &self.path,
                &self.saved,
                &self.displayed,
                &edit,
            )?;
            Ok(edit)
        });
        let mut edit = match edit {
            Ok(edit) => edit,
            Err(error) => {
                profile_window::install_profile_controls(window, &self.displayed, &[])?;
                return Err(error);
            }
        };
        let options = edit.control_bar.controller;
        let gain = edit.audio.replay_gain_options();
        committed(ProfileChanges {
            controller: (options != self.displayed.control_bar.controller).then_some(options),
            replay_gain: (gain != self.displayed.audio.replay_gain_options()).then_some(gain),
            subtitle_font: (edit.subtitle_font != self.displayed.subtitle_font)
                .then(|| edit.subtitle_font.clone()),
            #[cfg(target_os = "macos")]
            subtitle_border_style: (edit.subtitle_text_style.border_style
                != self.displayed.subtitle_text_style.border_style)
                .then_some(edit.subtitle_text_style.border_style),
            #[cfg(target_os = "macos")]
            subtitle_auto_load: (edit.subtitle_auto_load != self.displayed.subtitle_auto_load)
                .then(|| edit.subtitle_auto_load.clone()),
            key_bindings: (edit.key_bindings != self.displayed.key_bindings)
                .then(|| edit.key_bindings.clone()),
        });
        let previous = &self.displayed.online_subtitles;
        let selected = &mut edit.online_subtitles;
        // Commit the profile before writing credentials to the separate key store.
        let api_key = store_credential(
            self.credentials.open_subtitles,
            &mut selected.api_key,
            &previous.api_key,
        );
        let assrt = store_credential(
            self.credentials.assrt,
            &mut selected.assrt_token,
            &previous.assrt_token,
        );
        if api_key.is_err() || assrt.is_err() {
            let subtitles = window.global::<crate::SubtitleSettingsState>();
            subtitles.set_open_subtitles_api_key(selected.api_key.clone().into());
            subtitles.set_assrt_token(selected.assrt_token.clone().into());
        }
        self.displayed = edit;
        api_key.and(assrt)
    }
}

fn store_credential(
    store: StoreCredential,
    selected: &mut String,
    previous: &str,
) -> Result<(), String> {
    if selected != previous {
        store(selected).inspect_err(|_| *selected = previous.to_owned())?;
    }
    Ok(())
}

fn report(window: &PreferencesWindow, error: String) {
    crate::diagnostics::warn!("{error}");
    window.set_profile_status(error.into());
}

#[cfg(target_os = "macos")]
pub(super) fn bind_plugin_archive(
    app: &crate::PlayerWindow,
    local: &Rc<RefCell<Option<PreferencesWindow>>>,
    application: &Rc<ApplicationSettings>,
) {
    use super::PLUGINS_PANE;
    use riina_ui::PlayerMenuActions;

    let settings = local.clone();
    let application = application.clone();
    let parent = app.as_weak();
    app.on_open_plugin_archive(move |archive| {
        if application.is_visible() {
            return application.install_plugin_archive(archive).is_ok();
        }
        let Some(app) = parent.upgrade() else {
            return false;
        };
        let previous_pane = app.get_preferences_target_pane();
        app.set_preferences_target_pane(PLUGINS_PANE);
        app.global::<PlayerMenuActions>().invoke_open_preferences();
        app.set_preferences_target_pane(previous_pane);
        let window = settings
            .borrow()
            .as_ref()
            .map(ComponentHandle::clone_strong);
        if let Some(window) = window.filter(|window| window.window().is_visible()) {
            window.set_selected_pane(PLUGINS_PANE);
            window.invoke_plugin_install_archive(archive);
            true
        } else {
            false
        }
    });
}

#[cfg(target_os = "macos")]
pub(crate) fn application_status(window: &PreferencesWindow) -> impl Fn(String) + Clone + 'static {
    let weak = window.as_weak();
    move |message| {
        if let Some(window) = weak.upgrade() {
            report(&window, message);
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn configure_application_actions(
    window: &PreferencesWindow,
    path: &Path,
    open_log_viewer: impl Fn() + 'static,
) {
    profile_window::connect_actions(window, None, path, open_log_viewer);
    let weak = window.as_weak();
    let probe = crate::platform::text_recognition::check_availability(move |available| {
        let _ = crate::background::to_ui(weak, move |window| {
            window.set_live_text_available(available);
        });
    });
    if let Err(error) = probe {
        crate::diagnostics::warn!("Could not check Live Text availability: {error}");
    }
}

impl ApplicationSettings {
    pub(crate) fn is_visible(&self) -> bool {
        self.window
            .borrow()
            .as_ref()
            .is_some_and(|window| window.window().is_visible())
    }

    pub(crate) fn show_visible(&self, pane: Option<i32>) -> Result<bool, slint::PlatformError> {
        let existing = self
            .window
            .borrow()
            .as_ref()
            .map(ComponentHandle::clone_strong);
        let Some(window) = existing.filter(|window| window.window().is_visible()) else {
            return Ok(false);
        };
        if window.get_font_picker_open() {
            window.invoke_raise_font_picker();
            return Ok(true);
        }
        if let Some(pane) = pane {
            window.set_selected_pane(pane);
        }
        show(&window)?;
        Ok(true)
    }

    pub(crate) fn hide(&self) {
        if let Some(window) = self.window.borrow_mut().take() {
            close(&window);
        }
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn install_plugin_archive(&self, path: slint::SharedString) -> Result<(), String> {
        let active = self
            .window
            .borrow()
            .as_ref()
            .map(ComponentHandle::clone_strong);
        let window = active.ok_or("The Settings window could not be opened")?;
        show(&window).map_err(|error| error.to_string())?;
        window.set_selected_pane(super::PLUGINS_PANE);
        window.invoke_plugin_install_archive(path);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn show(
        self: &Rc<Self>,
        path: &Path,
        saved: Preferences,
        clear_history: impl Fn() -> Result<usize, String> + 'static,
        committed: impl Fn(ProfileChanges) + 'static,
        configure: impl FnOnce(&PreferencesWindow),
    ) -> Result<(), String> {
        if self.show_visible(None).map_err(|error| error.to_string())? {
            return Ok(());
        }
        let window = self.create(
            path,
            saved,
            Credentials::system(),
            clear_history,
            move |changes| {
                committed(changes);
                crate::platform::browser_activation::refresh_idle_menu();
            },
        )?;
        configure(&window);
        show(&window).map_err(|error| error.to_string())?;
        self.window.replace(Some(window));
        Ok(())
    }

    fn create(
        self: &Rc<Self>,
        path: &Path,
        saved: Preferences,
        credentials: Credentials,
        clear_history: impl Fn() -> Result<usize, String> + 'static,
        committed: impl Fn(ProfileChanges) + 'static,
    ) -> Result<PreferencesWindow, String> {
        let window = profile_window::create(path, &saved, &[], clear_history)?;
        Profile::connect(&window, path.to_owned(), saved, credentials, committed)?;
        let weak = window.as_weak();
        let service = Rc::downgrade(self);
        window.window().on_close_requested(move || {
            if let Some(window) = weak.upgrade()
                && !window
                    .global::<riina_ui::GeneralSettingsState>()
                    .get_folder_dialog_open()
            {
                close(&window);
                if let Some(service) = service.upgrade() {
                    service.window.borrow_mut().take();
                    service.closed.notify();
                }
            }
            CloseRequestResponse::KeepWindowShown
        });
        Ok(window)
    }
}

fn show(window: &PreferencesWindow) -> Result<(), slint::PlatformError> {
    window.invoke_refresh_utility_sizes_if_selected();
    crate::platform::show_window(window)?;
    let weak = window.as_weak();
    let _ = slint::spawn_local(async move {
        if let Some(window) = weak.upgrade()
            && let Ok(native) = window.window().winit_window().await
        {
            native.focus_window();
        }
    });
    Ok(())
}

fn close(window: &PreferencesWindow) {
    window.invoke_plugin_cancel_import();
    window.invoke_close_profile_font_picker();
    window.invoke_close_plugin_settings();
    window.invoke_dismiss_utility_dialogs();
    window.invoke_dismiss_native_subtitle_color();
    let _ = window.hide();
}

#[cfg(test)]
mod tests;
