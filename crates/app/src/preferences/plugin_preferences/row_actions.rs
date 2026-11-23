//! Persist plugin enablement and ordering from the installed package list.
#[cfg(all(test, any(target_os = "windows", target_os = "linux")))]
#[path = "settings_page_test.rs"]
mod settings_page_test;

use super::{
    PluginSettings,
    inventory::{load_plugin_order, refresh, save_enabled, save_plugin_order},
};
use crate::{PlayerWindow, PreferencesWindow, plugin_host::PluginHostHandle};
use riina_mpv::CommandSender;
use slint::ComponentHandle;

pub(super) fn register_settings_page(window: &PreferencesWindow, settings: &PluginSettings) {
    let pages = crate::platform::plugin_settings::Presenter::default();
    let weak = window.as_weak();
    let entries = settings.inventory.clone();
    let show = pages.clone();
    window.on_plugin_row_settings(move |index| {
        let (Some(window), Ok(index)) = (weak.upgrade(), usize::try_from(index)) else {
            return;
        };
        let Some(plugin) = entries.borrow().get(index).cloned() else {
            return;
        };
        if let Err(error) = show.open(&window, &plugin.directory, &plugin.manifest) {
            window.set_plugin_update_status(
                format!("Could not open plugin settings: {error}").into(),
            );
        }
    });
    let weak = window.as_weak();
    window.on_close_plugin_settings(move || {
        if let Err(error) = pages.close()
            && let Some(window) = weak.upgrade()
        {
            window.set_plugin_update_status(
                format!("Could not save plugin settings: {error}").into(),
            );
        }
    });
    let weak = window.as_weak();
    window.on_plugin_settings_pane_left(move || {
        let weak = weak.clone();
        // Let the pane change finish before tearing down the native browser.
        let _ = slint::spawn_local(async move {
            if let Some(window) = weak.upgrade()
                && window.get_selected_pane() != 9
            {
                window.invoke_close_plugin_settings();
            }
        });
    });
}

pub(super) fn register_reorder(window: &PreferencesWindow, settings: &PluginSettings) {
    let runtime_available = settings.runtime_available;
    let weak = window.as_weak();
    let entries = settings.inventory.clone();
    let enabled_plugins = settings.enabled.clone();
    let order_file = settings.plugins.with_file_name("plugin-order.json");
    let plugins = settings.plugins.clone();
    window.on_plugin_reordered(move |dragged, target| {
        let Some(window) = weak.upgrade() else { return };
        let installed = entries
            .borrow()
            .iter()
            .map(|plugin| plugin.manifest.identifier.clone())
            .collect::<Vec<_>>();
        let mut order =
            riina_core::plugin_order::normalize(&load_plugin_order(&order_file), &installed);
        if !riina_core::plugin_order::move_before(&mut order, dragged.as_str(), target.as_str()) {
            return;
        }
        if let Err(error) = save_plugin_order(&order_file, &order) {
            window.set_plugin_update_status(format!("Could not save plugin order: {error}").into());
            return;
        }
        refresh(
            &window,
            &plugins,
            &entries,
            &enabled_plugins.borrow(),
            runtime_available,
        );
        crate::player_ui::reorder_all_open_plugin_menus(&order);
        window.set_plugin_update_status("Plugin order saved.".into());
    });
}

pub(super) fn register_enabled(
    window: &PreferencesWindow,
    app: &PlayerWindow,
    settings: &PluginSettings,
    host: Option<&PluginHostHandle>,
    player: CommandSender,
) {
    let runtime_available = settings.runtime_available;
    let weak = window.as_weak();
    let entries = settings.inventory.clone();
    let enabled_plugins = settings.enabled.clone();
    let plugin_host = host.cloned();
    let enabled_file = settings.enabled_path.clone();
    let plugins = settings.plugins.clone();
    let plugin_state_parent = app.as_weak();
    window.on_plugin_enabled_changed(move |index, should_enable| {
        let Some(window) = weak.upgrade() else { return };
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let Some(plugin) = entries.borrow().get(index).cloned() else {
            return;
        };
        let mut updated = enabled_plugins.borrow().clone();
        let Some(plugin_host) = plugin_host.as_ref() else {
            window.set_plugin_update_status("Plugin host is unavailable.".into());
            refresh(
                &window,
                &plugins,
                &entries,
                &enabled_plugins.borrow(),
                false,
            );
            return;
        };
        if should_enable {
            updated.insert(plugin.manifest.identifier.clone());
            if let Err(error) = save_enabled(&enabled_file, &updated) {
                window.set_plugin_update_status(
                    format!("Could not save plugin state: {error}").into(),
                );
                return;
            }
            if let Err(error) =
                plugin_host.enable(plugin.manifest.clone(), plugin.directory.clone())
            {
                updated.remove(&plugin.manifest.identifier);
                if let Err(restore_error) = save_enabled(&enabled_file, &updated) {
                    crate::diagnostics::warn!(
                        "Could not restore the disabled plugin state: {restore_error}"
                    );
                }
                window.set_plugin_update_status(format!("Could not enable plugin: {error}").into());
                *enabled_plugins.borrow_mut() = updated;
                refresh(
                    &window,
                    &plugins,
                    &entries,
                    &enabled_plugins.borrow(),
                    runtime_available,
                );
                return;
            }
            *enabled_plugins.borrow_mut() = updated;
            window.set_plugin_update_status(
                format!(
                    "{} is enabled; its runtime is starting.",
                    plugin.manifest.name
                )
                .into(),
            );
        } else {
            updated.remove(&plugin.manifest.identifier);
            if let Err(error) = save_enabled(&enabled_file, &updated) {
                window.set_plugin_update_status(
                    format!("Could not save plugin state: {error}").into(),
                );
                return;
            }
            if let Err(error) = plugin_host.disable(plugin.manifest.identifier.clone()) {
                let previous = enabled_plugins.borrow().clone();
                if let Err(restore_error) = save_enabled(&enabled_file, &previous) {
                    crate::diagnostics::warn!(
                        "Could not restore the enabled plugin state: {restore_error}"
                    );
                }
                window.set_plugin_update_status(format!("Could not stop plugin: {error}").into());
                refresh(&window, &plugins, &entries, &previous, runtime_available);
                return;
            }
            *enabled_plugins.borrow_mut() = updated;
            window
                .set_plugin_update_status(format!("{} is disabled.", plugin.manifest.name).into());
        }
        refresh(
            &window,
            &plugins,
            &entries,
            &enabled_plugins.borrow(),
            runtime_available,
        );
        if plugin.manifest.identifier == "io.iina.ytdl"
            && let Some(app) = plugin_state_parent.upgrade()
        {
            app.global::<riina_ui::PlayerPreferences>()
                .set_network_ytdl_plugin_active(should_enable);
            if let Err(error) =
                crate::network_ytdl_preferences::apply_plugin_override(&player, &app, should_enable)
            {
                app.set_status(error.into());
            }
        }
    });
}

pub(super) fn register_open_folder(window: &PreferencesWindow, settings: &PluginSettings) {
    let weak = window.as_weak();
    let entries = settings.inventory.clone();
    window.on_plugin_row_actions(move |index| {
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let Some(plugin) = entries.borrow().get(index).cloned() else {
            return;
        };
        match crate::platform::open_directory(&plugin.directory) {
            Ok(()) => {
                if let Some(window) = weak.upgrade() {
                    window.set_plugin_update_status(
                        format!("Opened the package folder for {}.", plugin.manifest.name).into(),
                    );
                }
            }
            Err(error) => {
                if let Some(window) = weak.upgrade() {
                    window.set_plugin_update_status(error.to_string().into());
                }
            }
        }
    });
}
