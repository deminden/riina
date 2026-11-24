//! Own the preferences window and its platform dialogs and refresh timers.
#[cfg(any(target_os = "macos", test))]
mod application;
#[cfg(target_os = "macos")]
pub(crate) use application::{
    ApplicationSettings, application_status, configure_application_actions,
};
mod changes;
mod construction;
mod folder;
mod general_policy;
mod key_bindings;
mod live;
mod owner;
mod profile_window;
mod refresh;
mod timers;

use super::model::PreferenceModel;
use crate::PlayerWindow;
use refresh::refresh;
use riina_ui::PlayerMenuActions;
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{cell::RefCell, path::PathBuf, rc::Rc};

#[cfg(target_os = "macos")]
const PLUGINS_PANE: i32 = 9;

pub(crate) struct Session {
    pub(crate) decoder_player: riina_mpv::CommandSender,
    pub(crate) path: PathBuf,
    pub(crate) model: PreferenceModel,
    pub(crate) latest_playback: crate::PlaybackStore,
    pub(crate) plugin_host: Option<crate::plugin_host::PluginHostHandle>,
    pub(crate) updates: Rc<crate::updates::ApplicationUpdates>,
    pub(crate) power: Rc<crate::playback_power::ApplicationPower>,
    #[cfg(target_os = "macos")]
    pub(crate) application_settings: Rc<ApplicationSettings>,
}

pub(crate) fn connect(
    app: &PlayerWindow,
    Session {
        decoder_player,
        path,
        model,
        latest_playback,
        plugin_host,
        updates,
        power,
        #[cfg(target_os = "macos")]
        application_settings,
    }: Session,
    clear_history: impl Fn() -> Result<usize, String> + Clone + 'static,
) {
    let baseline = model.snapshot();
    let current = Rc::new(RefCell::new(None::<crate::PreferencesWindow>));
    #[cfg(target_os = "macos")]
    {
        let settings = current.clone();
        let application = application_settings.clone();
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
    let timers = timers::VisibleSettings::default();
    let preferences_window = current.clone();
    let subtitle_catalog_window = current.clone();
    let owner = app.as_weak();
    app.on_plugin_subtitle_catalog_changed(move || {
        if let Some(app) = owner.upgrade()
            && let Some(window) = subtitle_catalog_window.borrow().as_ref()
        {
            crate::online_subtitles_preferences::refresh_catalog(window, &app);
        }
    });
    app.on_subtitle_font_selected(move |font| {
        if let Some(window) = preferences_window
            .borrow()
            .as_ref()
            .map(ComponentHandle::clone_strong)
        {
            window
                .global::<crate::SubtitleSettingsState>()
                .set_subtitle_font(font);
            window.invoke_settings_changed();
        }
    });
    let parent = app.as_weak();
    app.global::<PlayerMenuActions>()
        .on_open_preferences(move || {
            let Some(app) = parent.upgrade() else { return };
            if app.global::<riina_ui::PlayerCropState>().get_crop_editing()
                || app
                    .global::<riina_ui::PlayerDialogState>()
                    .get_dialog_open()
            {
                return;
            }
            #[cfg(target_os = "macos")]
            {
                let pane = app.get_preferences_target_pane();
                match application_settings.show_visible((pane >= 0).then_some(pane)) {
                    Ok(false) => {}
                    Ok(true) => {
                        app.set_preferences_target_pane(-1);
                        return;
                    }
                    Err(error) => {
                        app.set_status(error.to_string().into());
                        return;
                    }
                }
            }
            let existing = current.borrow().as_ref().map(ComponentHandle::clone_strong);
            let window = if let Some(window) = existing {
                window
            } else {
                let Some(window) = construction::create(
                    &app,
                    construction::Inputs {
                        baseline: &baseline,
                        path: &path,
                        model: &model,
                        player: &decoder_player,
                        playback: &latest_playback,
                        plugin: plugin_host.as_ref(),
                    },
                    clear_history.clone(),
                ) else {
                    return;
                };
                let owner = app.as_weak();
                let report = move |message| {
                    if let Some(app) = owner.upgrade() {
                        crate::diagnostics::report(&app, message);
                    }
                };
                updates.connect_settings(&window, report.clone());
                power.connect_settings(&window, &path, &baseline, report);
                timers.install_close(&window);
                crate::platform::center_auxiliary_window(&window, &app);
                *current.borrow_mut() = Some(window.clone_strong());
                window
            };
            let target_pane = app.get_preferences_target_pane();
            if target_pane >= 0 {
                window.set_selected_pane(target_pane);
                app.set_preferences_target_pane(-1);
            }
            let stored_audio = crate::preference_load::read_valid(&path)
                .ok()
                .flatten()
                .map(|preferences| preferences.audio)
                .unwrap_or_else(|| baseline.audio.clone());
            let available_devices = latest_playback
                .lock()
                .map(|playback| playback.audio_devices.clone())
                .unwrap_or_default();
            crate::audio_preferences::install_window(&window, &stored_audio, &available_devices);
            refresh(&window, &app);
            window.invoke_refresh_utility_sizes_if_selected();
            updates.refresh_settings(&window);
            power.refresh_settings(&window);
            if let Err(error) = crate::platform::show_window(&window) {
                app.set_status(error.to_string().into());
                return;
            }
            timers.start(
                &window,
                latest_playback.clone(),
                stored_audio,
                available_devices,
            );
            let weak = window.as_weak();
            // Settings is already visible. Deferred focus can expire with the window.
            let _ = slint::spawn_local(async move {
                if let Some(window) = weak.upgrade()
                    && let Ok(native) = window.window().winit_window().await
                {
                    native.focus_window();
                }
            });
        });
}
