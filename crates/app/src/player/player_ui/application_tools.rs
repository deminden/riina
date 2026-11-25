//! AppKit menu actions and tools that can outlive a player.
use super::ApplicationServices;
use std::rc::Rc;

impl ApplicationServices {
    pub(crate) fn bind_auxiliary_close(&self, callback: Rc<dyn Fn()>) {
        self.about.closed.bind(callback.clone());
        self.logs.closed.bind(callback.clone());
        self.open_url.closed.bind(callback.clone());
        self.updates.closed.bind(callback.clone());
        self.settings.closed.bind(callback.clone());
        self.plugins.bind_application_close(callback.clone());
        self.filters.closed.bind(callback.clone());
        self.highlights.closed.bind(callback);
    }

    pub(crate) fn auxiliary_is_visible(&self) -> bool {
        self.history.is_visible()
            || self.about.is_visible()
            || self.logs.is_visible()
            || self.open_url.is_visible()
            || self.updates.is_visible()
            || self.highlights.is_visible()
            || self.settings.is_visible()
            || self.plugins.auxiliary_is_visible()
            || self.filters.is_visible()
    }

    pub(crate) fn hide_auxiliary_tools(&self) {
        self.about.hide();
        self.logs.hide();
        self.open_url.hide_for_quit();
        self.updates.hide_for_quit();
        self.highlights.hide();
        self.settings.hide();
        self.plugins.hide_for_quit();
    }

    pub(crate) fn show_application_settings(
        &self,
        path: &std::path::Path,
        loaded: crate::preference_load::LoadedPreferences,
        pane: Option<i32>,
    ) -> Result<(), String> {
        if self
            .settings
            .show_visible(pane)
            .map_err(|error| error.to_string())?
        {
            return Ok(());
        }
        let mut saved = loaded.preferences;
        let subtitle_credentials = crate::online_subtitles_credentials::restore_preferences(
            &mut saved,
            loaded.source_text.as_deref(),
            path,
            loaded.recovery.is_none(),
        );
        let status = loaded
            .recovery
            .map(|recovery| recovery.notice())
            .or_else(|| subtitle_credentials.err());
        let history = Rc::downgrade(&self.history);
        let recent = self.recents.for_profile(path, &saved, true);
        let history_path = path.to_owned();
        let clear = move || {
            let history = history.upgrade().ok_or("Playback History is unavailable")?;
            let removed = history.clear_from_settings(&history_path)?;
            let cleared = recent.clear_for_history();
            let synced = history.sync_plugins(&[]);
            crate::platform::browser_activation::refresh_idle_menu();
            cleared.and(synced)?;
            Ok(removed)
        };
        self.configure_application_updates(path, &saved);
        self.power.initialize(saved.playback_power);
        let baseline = saved.clone();
        let clocks = Rc::clone(&self.clocks);
        let controller_path = path.to_owned();
        let controller_committed = move |options| clocks.publish_players(&controller_path, options);
        self.settings
            .show(path, saved, clear, controller_committed, |window| {
                self.clocks.form.register(path, window);
                if let Some(pane) = pane {
                    window.set_selected_pane(pane);
                }
                let report = crate::preferences_window::application_status(window);
                self.updates.connect_settings(window, report.clone());
                self.updates.refresh_settings(window);
                self.power
                    .connect_settings(window, path, &baseline, report.clone());
                self.power.refresh_settings(window);
                if let Some(status) = status {
                    report(status);
                }
                let plugin_host = self
                    .plugins
                    .host(path)
                    .inspect_err(|error| report(error.clone()))
                    .ok();
                crate::plugin_preferences::connect(window, path, plugin_host, None);
                let logs = Rc::downgrade(&self.logs);
                let path_for_logs = path.to_owned();
                crate::preferences_window::configure_application_actions(window, path, move || {
                    let Some(logs) = logs.upgrade() else { return };
                    let level = crate::preference_load::read_valid(&path_for_logs)
                        .ok()
                        .flatten()
                        .map_or(baseline.advanced.log_level, |saved| {
                            saved.advanced.log_level
                        });
                    if let Err(error) = logs.show(None, &path_for_logs, level) {
                        report(error.to_string());
                    }
                });
            })
    }

    pub(crate) fn open_plugin_archive(&self, path: String) -> Result<(), String> {
        self.settings.install_plugin_archive(path.into())
    }

    pub(crate) fn configure_application_updates(
        &self,
        path: &std::path::Path,
        preferences: &riina_core::Preferences,
    ) {
        self.updates.configure_application(path, preferences);
    }

    pub(crate) fn show_application_updates(
        &self,
        path: &std::path::Path,
        preferences: &riina_core::Preferences,
    ) -> Result<(), slint::PlatformError> {
        self.configure_application_updates(path, preferences);
        self.updates.show_from_application()
    }

    pub(crate) fn show_application_highlights(
        &self,
        preferences: &riina_core::Preferences,
    ) -> Result<(), slint::PlatformError> {
        self.highlights.show_from_application(preferences)
    }

    pub(crate) fn dump_application_debug_information(&self) {
        self.debug_information.dump_from_application();
    }

    pub(crate) fn show_application_about(
        &self,
        preferences: &riina_core::Preferences,
    ) -> Result<(), slint::PlatformError> {
        self.about.show_from_application(preferences)
    }

    pub(crate) fn record_application_documents(
        &self,
        profile: &std::path::Path,
        preferences: &riina_core::Preferences,
        paths: &[String],
    ) {
        if !preferences.record_recent_files {
            return;
        }
        let recents = self.recents.for_profile(profile, preferences, true);
        let documents = paths.iter().map(|path| {
            let title = std::path::Path::new(path)
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .unwrap_or(path)
                .to_owned();
            (path.clone(), title)
        });
        if let Err(error) = recents.record_documents(documents) {
            crate::diagnostics::warn!("{error}");
        }
    }

    pub(crate) fn clear_application_documents(
        &self,
        profile: &std::path::Path,
        preferences: &riina_core::Preferences,
    ) -> Result<(), String> {
        self.recents
            .for_profile(profile, preferences, true)
            .clear_documents()
    }

    pub(crate) fn show_application_logs(
        &self,
        path: &std::path::Path,
        level: riina_core::advanced_preferences::LogLevel,
    ) -> Result<(), slint::PlatformError> {
        self.logs.show(None, path, level)
    }
}
