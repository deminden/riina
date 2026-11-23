//! Present plugin metadata and user-driven documentation or contact actions.
use super::{
    InstalledPlugin, PluginSettings, permissions::permission_description, update_actions::Requests,
};
use crate::{
    PreferencesWindow,
    platform::{self, plugin_settings::Presenter},
};
use riina_core::plugin::{Permission, plugin_web_link};
use riina_ui::{PluginInformation, PluginInformationAction};
use slint::ComponentHandle;

pub(super) fn register(
    window: &PreferencesWindow,
    settings: &PluginSettings,
    requests: &Requests,
    pages: Presenter,
) {
    let weak = window.as_weak();
    let entries = settings.inventory.clone();
    let versions = requests.available.clone();
    window.on_plugin_row_about(move |index| {
        let (Some(window), Ok(index)) = (weak.upgrade(), usize::try_from(index)) else {
            return;
        };
        let entries = entries.borrow();
        let Some(plugin) = entries.get(index) else {
            return;
        };
        let versions = riina_core::sync::lock(&versions);
        window.set_plugin_information(information(
            plugin,
            versions
                .get(&plugin.manifest.identifier)
                .map(String::as_str),
        ));
        window.set_plugin_information_status("".into());
        window.set_plugin_information_open(true);
    });
    let weak = window.as_weak();
    let entries = settings.inventory.clone();
    window.on_plugin_information_action(move |action| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let information = window.get_plugin_information();
        let selected = entries
            .borrow()
            .iter()
            .find(|plugin| plugin.manifest.identifier == information.identifier.as_str())
            .cloned();
        let result = match selected {
            Some(plugin) => open_action(&window, &pages, &plugin, action),
            None => Err("The selected plugin is no longer installed.".into()),
        };
        window.set_plugin_information_status(result.err().unwrap_or_default().into());
    });
}

fn information(plugin: &InstalledPlugin, available: Option<&str>) -> PluginInformation {
    let manifest = &plugin.manifest;
    let permissions = if manifest.permissions.is_empty() {
        "No permissions requested".to_owned()
    } else {
        manifest
            .permissions
            .iter()
            .map(|name| match Permission::parse(name) {
                Some(permission) => permission_description(permission).to_owned(),
                None => format!("Unknown permission: {name}"),
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let can_update =
        plugin.can_update() && manifest.github_version.is_some() && manifest.github_repo.is_some();
    PluginInformation {
        identifier: manifest.identifier.as_str().into(),
        name: manifest.name.as_str().into(),
        version: manifest.version.as_str().into(),
        author: manifest.author.name.as_str().into(),
        description: manifest
            .description
            .as_deref()
            .unwrap_or("No description")
            .into(),
        permissions: permissions.into(),
        source: manifest
            .github_repo
            .as_deref()
            .unwrap_or(if plugin.is_development() {
                "Development package"
            } else {
                "Local package"
            })
            .into(),
        has_website: manifest
            .author
            .url
            .as_deref()
            .and_then(plugin_web_link)
            .is_some(),
        has_email: manifest
            .author
            .email
            .as_deref()
            .and_then(platform::plugin_email_link)
            .is_some(),
        has_source: manifest.github_repo.is_some(),
        has_help: manifest.help_page.is_some(),
        can_update,
        update_available: available.filter(|_| can_update).unwrap_or_default().into(),
    }
}

fn open_action(
    window: &PreferencesWindow,
    pages: &Presenter,
    plugin: &InstalledPlugin,
    action: PluginInformationAction,
) -> Result<(), String> {
    let manifest = &plugin.manifest;
    match action {
        PluginInformationAction::Website => {
            let url = manifest
                .author
                .url
                .as_deref()
                .and_then(plugin_web_link)
                .ok_or("This plugin has no author website")?;
            platform::open_http_url_in_browser(url.as_str()).map_err(|error| error.to_string())
        }
        PluginInformationAction::Email => {
            let address = manifest
                .author
                .email
                .as_deref()
                .ok_or("This plugin has no author email")?;
            platform::open_plugin_contact_email(address).map_err(|error| error.to_string())
        }
        PluginInformationAction::Source => {
            let repo = manifest
                .github_repo
                .as_deref()
                .ok_or("This plugin has no source repository")?;
            platform::open_http_url_in_browser(&format!("https://github.com/{repo}"))
                .map_err(|error| error.to_string())
        }
        PluginInformationAction::Help => pages.open_help(window, &plugin.directory, manifest),
    }
}
