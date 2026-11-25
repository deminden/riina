//! Own the preferences window and its platform dialogs and refresh timers.
mod changes;
mod construction;
mod folder;
pub(crate) mod general_policy;
mod key_bindings;
mod live;
mod music_mode;
mod owner;
pub(crate) mod profile_window;
mod refresh;
mod timers;

use super::model::PreferenceModel;
use crate::{PlayerWindow, platform::application_settings};
use refresh::refresh;
use riina_ui::PlayerMenuActions;
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{cell::RefCell, path::PathBuf, rc::Rc};

pub(crate) struct Session {
    pub(crate) decoder_player: riina_mpv::CommandSender,
    pub(crate) path: PathBuf,
    pub(crate) model: PreferenceModel,
    pub(crate) latest_playback: crate::PlaybackStore,
    pub(crate) plugin_host: Option<crate::plugin_host::PluginHostHandle>,
    pub(crate) updates: Rc<crate::updates::ApplicationUpdates>,
    pub(crate) power: Rc<crate::playback_power::ApplicationPower>,
    pub(crate) application_settings: application_settings::Owner,
    pub(crate) replay_gain: crate::player_ui::ReplayGainBinding,
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
        application_settings,
        replay_gain,
    }: Session,
    clear_history: impl Fn() -> Result<usize, String> + Clone + 'static,
) {
    let baseline = model.snapshot();
    let current = Rc::new(RefCell::new(None::<crate::PreferencesWindow>));
    music_mode::connect(app, current.clone(), path.clone(), model.clone());
    application_settings.bind_plugin_archive(app, &current);
    let timers = timers::VisibleSettings::default();
    connect_controller_options(app, current.clone());
    connect_subtitle_actions(app, current.clone());
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
            if application_settings.redirect_to_visible(&app) {
                return;
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
                        replay_gain: &replay_gain,
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

fn connect_subtitle_actions(
    app: &PlayerWindow,
    preferences_window: Rc<RefCell<Option<crate::PreferencesWindow>>>,
) {
    let subtitle_catalog_window = preferences_window.clone();
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
}

fn connect_controller_options(
    app: &PlayerWindow,
    window: Rc<RefCell<Option<crate::PreferencesWindow>>>,
) {
    let owner = app.as_weak();
    app.global::<riina_ui::PlayerControlActions>()
        .on_refresh_controller_options(move || {
            if let Some(app) = owner.upgrade()
                && let Some(window) = window.borrow().as_ref()
            {
                let options = crate::osc_preferences::player_controller_options(&app);
                crate::osc_preferences::install_form_controller_options(window, options);
            }
        });
}
