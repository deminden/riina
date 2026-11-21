//! Keep editable cache values typed at the persistence and engine boundary.

use super::model::{PreferenceSection, SectionError};
use crate::{PlayerWindow, PreferencesWindow};
use riina_core::Preferences;
use riina_core::{Command, network_preferences::Cache};

impl PreferenceSection for Cache {
    fn get(all: &Preferences) -> Self {
        all.network_cache.clone()
    }

    fn put(self, all: &mut Preferences) {
        all.network_cache = self;
    }

    fn read_window(window: &PreferencesWindow) -> Result<Self, SectionError> {
        current_window(window).map_err(SectionError::from)
    }

    fn write_window(&self, window: &PreferencesWindow) {
        install_window(window, self);
    }

    fn write_player(&self, app: &PlayerWindow) {
        install_player(app, self);
    }

    fn live_commands(&self, previous: &Self) -> Vec<Command> {
        if self == previous {
            Vec::new()
        } else {
            vec![Command::NetworkCache(self.clone())]
        }
    }
}

fn parse_number<T: std::str::FromStr>(text: &str, label: &str) -> Result<T, String> {
    text.trim()
        .parse()
        .map_err(|_| format!("{label} must be a number"))
}

fn validate(cache: Cache) -> Result<Cache, String> {
    cache.validate().map_err(str::to_owned)?;
    Ok(cache)
}

pub(crate) fn current_player(app: &crate::PlayerWindow) -> Result<Cache, String> {
    validate(Cache {
        enabled: app.get_network_cache_enabled(),
        pause_initial: app.get_network_cache_pause_initial(),
        pause_wait_secs: parse_number(
            app.get_network_cache_pause_wait_text().as_str(),
            "Buffer-before-start duration",
        )?,
        prefetch_secs: parse_number(
            app.get_network_cache_prefetch_secs_text().as_str(),
            "Prefetch duration",
        )?,
        buffer_size_kib: parse_number(
            app.get_network_cache_size_kib_text().as_str(),
            "Cache buffer size",
        )?,
        show_buffering_throbber: app.get_network_show_buffering_throbber(),
        show_seeking_throbber: app.get_network_show_seeking_throbber(),
    })
}

pub(crate) fn current_window(window: &crate::PreferencesWindow) -> Result<Cache, String> {
    validate(Cache {
        enabled: window.get_network_cache_enabled(),
        pause_initial: window.get_network_cache_pause_initial(),
        pause_wait_secs: parse_number(
            window.get_network_cache_pause_wait_text().as_str(),
            "Buffer-before-start duration",
        )?,
        prefetch_secs: parse_number(
            window.get_network_cache_prefetch_secs_text().as_str(),
            "Prefetch duration",
        )?,
        buffer_size_kib: parse_number(
            window.get_network_cache_size_kib_text().as_str(),
            "Cache buffer size",
        )?,
        show_buffering_throbber: window.get_network_show_buffering_throbber(),
        show_seeking_throbber: window.get_network_show_seeking_throbber(),
    })
}

pub(crate) fn install_player(app: &crate::PlayerWindow, cache: &Cache) {
    app.set_network_cache_enabled(cache.enabled);
    app.set_network_cache_pause_initial(cache.pause_initial);
    app.set_network_cache_pause_wait_text(cache.pause_wait_secs.to_string().into());
    app.set_network_cache_prefetch_secs_text(cache.prefetch_secs.to_string().into());
    app.set_network_cache_size_kib_text(cache.buffer_size_kib.to_string().into());
    app.set_network_show_buffering_throbber(cache.show_buffering_throbber);
    app.set_network_show_seeking_throbber(cache.show_seeking_throbber);
}

pub(crate) fn install_window(window: &crate::PreferencesWindow, cache: &Cache) {
    window.set_network_cache_enabled(cache.enabled);
    window.set_network_cache_pause_initial(cache.pause_initial);
    window.set_network_cache_pause_wait_text(cache.pause_wait_secs.to_string().into());
    window.set_network_cache_prefetch_secs_text(cache.prefetch_secs.to_string().into());
    window.set_network_cache_size_kib_text(cache.buffer_size_kib.to_string().into());
    window.set_network_show_buffering_throbber(cache.show_buffering_throbber);
    window.set_network_show_seeking_throbber(cache.show_seeking_throbber);
}

pub(crate) fn apply(
    player: &riina_mpv::CommandSender,
    app: &crate::PlayerWindow,
    previous: &Cache,
    selected: &Cache,
) -> Result<(), String> {
    for command in selected.live_commands(previous) {
        player.send(command)?;
        install_player(app, selected);
    }
    Ok(())
}

pub(crate) fn restore(
    player: &riina_mpv::CommandSender,
    app: &crate::PlayerWindow,
    previous: &Cache,
    selected: &Cache,
) {
    for command in previous.live_commands(selected) {
        let _ = player.send(command);
        install_player(app, previous);
    }
}

pub(crate) fn refresh_window(window: &crate::PreferencesWindow, app: &crate::PlayerWindow) {
    if let Ok(cache) = current_player(app) {
        install_window(window, &cache);
    }
}
