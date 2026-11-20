//! Size and count limits for effects leaving a plugin realm.
use super::{
    HostEvent, MAX_HOST_EVENTS, MAX_HOST_OUTPUT_BYTES, MAX_MESSAGE_CHARS, MAX_OVERLAY_INTENT_BYTES,
    MAX_PLUGIN_MESSAGE_BYTES, UiEffect, validate_plugin_message,
};
use riina_core::{
    plugin::{OverlayEffect, OverlayIntent, OverlayState},
    plugin_menu::PluginMenuItem,
    plugin_sidebar::{PluginSidebarEffect, PluginSidebarIntent, PluginSidebarState},
};
use std::sync::Mutex;

pub(super) fn record_plugin_message(
    events: &Mutex<Vec<HostEvent>>,
    name: String,
    data: serde_json::Value,
) -> Result<(), String> {
    validate_plugin_message(&name, &data)?;
    let mut events = riina_core::sync::lock(events);
    if events.len() >= MAX_HOST_EVENTS {
        return Err("Plugin event queue is full".into());
    }
    let message_size = name
        .len()
        .saturating_add(serde_json::to_vec(&data).map_or(usize::MAX, |bytes| bytes.len()));
    let pending_size = events.iter().map(host_event_size).sum::<usize>();
    if pending_size.saturating_add(message_size) > MAX_HOST_OUTPUT_BYTES {
        return Err("Plugin event output exceeded the 4 MiB queue limit".into());
    }
    events.push(HostEvent::PluginMessage(name, data));
    Ok(())
}

pub(super) fn record_overlay(
    events: &Mutex<Vec<HostEvent>>,
    state: &Mutex<OverlayState>,
    intent: OverlayIntent,
) -> Result<(), String> {
    let effects = state
        .lock()
        .map_err(|_| "Plugin overlay state was poisoned".to_owned())?
        .apply(intent)?;
    let mut events = riina_core::sync::lock(events);
    let current_bytes = events.iter().map(host_event_size).sum::<usize>();
    let requested_bytes = effects.iter().map(overlay_effect_size).sum::<usize>();
    if effects
        .iter()
        .any(|effect| overlay_effect_size(effect) > MAX_OVERLAY_INTENT_BYTES)
    {
        return Err("Overlay request exceeds the 1 MiB limit".into());
    }
    if events.len().saturating_add(effects.len()) > MAX_HOST_EVENTS
        || current_bytes.saturating_add(requested_bytes) > MAX_HOST_OUTPUT_BYTES
    {
        return Ok(());
    }
    events.extend(effects.into_iter().map(HostEvent::Overlay));
    Ok(())
}

fn overlay_effect_size(effect: &OverlayEffect) -> usize {
    match effect {
        OverlayEffect::LoadPluginFile(path) => path.as_os_str().to_string_lossy().len(),
        OverlayEffect::SetStyle(value) | OverlayEffect::SetContent(value) => value.len(),
        OverlayEffect::PostMessage { name, data }
        | OverlayEffect::DeliverMessage { name, data } => {
            name.len() + serde_json::to_vec(data).map_or(usize::MAX, |bytes| bytes.len())
        }
        OverlayEffect::ListenMessage(name) => name.len(),
        OverlayEffect::EnterSimpleMode
        | OverlayEffect::Show
        | OverlayEffect::Hide
        | OverlayEffect::SetOpacity(_)
        | OverlayEffect::SetClickable(_) => 8,
    }
}

pub(super) fn record_plugin_sidebar(
    events: &Mutex<Vec<HostEvent>>,
    state: &Mutex<PluginSidebarState>,
    intent: PluginSidebarIntent,
) -> Result<(), String> {
    let effect = state
        .lock()
        .map_err(|_| "Plugin sidebar state was poisoned".to_owned())?
        .apply(intent)?;
    let Some(effect) = effect else {
        return Ok(());
    };
    let size = sidebar_effect_size(&effect);
    if size > MAX_PLUGIN_MESSAGE_BYTES {
        return Err("Sidebar request exceeds the 1 MiB limit".into());
    }
    let mut events = riina_core::sync::lock(events);
    let current_bytes = events.iter().map(host_event_size).sum::<usize>();
    if events.len() >= MAX_HOST_EVENTS || current_bytes.saturating_add(size) > MAX_HOST_OUTPUT_BYTES
    {
        return Ok(());
    }
    events.push(HostEvent::Sidebar(effect));
    Ok(())
}

fn sidebar_effect_size(effect: &PluginSidebarEffect) -> usize {
    match effect {
        PluginSidebarEffect::LoadFile(path) => path.as_os_str().to_string_lossy().len(),
        PluginSidebarEffect::PostMessage { name, data }
        | PluginSidebarEffect::DeliverMessage { name, data } => name
            .len()
            .saturating_add(serde_json::to_vec(data).map_or(usize::MAX, |bytes| bytes.len())),
        PluginSidebarEffect::ListenMessage(name) => name.len(),
        PluginSidebarEffect::Show | PluginSidebarEffect::Hide => 1,
    }
}

pub(super) fn record_plugin_menu(
    events: &Mutex<Vec<HostEvent>>,
    items: Vec<PluginMenuItem>,
) -> Result<(), String> {
    PluginMenuItem::validate_all(&items)?;
    let bytes = serde_json::to_vec(&items)
        .map_err(|error| format!("Plugin menu is not valid JSON: {error}"))?
        .len();
    if bytes > MAX_PLUGIN_MESSAGE_BYTES {
        return Err("Plugin menu exceeds the 1 MiB limit".into());
    }
    let mut events = riina_core::sync::lock(events);
    events.retain(|event| !matches!(event, HostEvent::MenuUpdate(_)));
    let current_bytes = events.iter().map(host_event_size).sum::<usize>();
    if events.len() < MAX_HOST_EVENTS
        && current_bytes.saturating_add(bytes) <= MAX_HOST_OUTPUT_BYTES
    {
        events.push(HostEvent::MenuUpdate(items));
    }
    Ok(())
}

pub(super) fn menu_js_error(message: String) -> rquickjs::Error {
    rquickjs::Error::new_from_js_message("menu request", "valid plugin menu", message)
}

pub(super) fn sidebar_js_error(message: String) -> rquickjs::Error {
    rquickjs::Error::new_from_js_message("sidebar request", "valid sidebar effect", message)
}

fn host_event_size(event: &HostEvent) -> usize {
    match event {
        HostEvent::Ui(effect) => match effect {
            UiEffect::Log(message) | UiEffect::ShowOsd(message) | UiEffect::ShowAlert(message) => {
                message.len()
            }
            UiEffect::CoreCommand(command) => format!("{command:?}").len(),
            UiEffect::SetUiVisibility(_) => 1,
            UiEffect::WindowAction(action) => format!("{action:?}").len(),
        },
        HostEvent::Overlay(effect) => overlay_effect_size(effect),
        HostEvent::Sidebar(effect) => sidebar_effect_size(effect),
        HostEvent::MenuUpdate(items) => {
            serde_json::to_vec(items).map_or(usize::MAX, |bytes| bytes.len())
        }
        HostEvent::PluginMessage(name, data) => name
            .len()
            .saturating_add(serde_json::to_vec(data).map_or(usize::MAX, |bytes| bytes.len())),
    }
}

pub(super) fn overlay_js_error(message: String) -> rquickjs::Error {
    rquickjs::Error::new_from_js_message("overlay request", "valid overlay effect", message)
}

pub(super) fn record(events: &Mutex<Vec<HostEvent>>, mut effect: UiEffect) {
    let mut events = riina_core::sync::lock(events);
    if events.len() >= MAX_HOST_EVENTS {
        return;
    }
    match &mut effect {
        UiEffect::Log(message) | UiEffect::ShowOsd(message) | UiEffect::ShowAlert(message) => {
            if let Some((limit, _)) = message.char_indices().nth(MAX_MESSAGE_CHARS) {
                message.truncate(limit);
                message.shrink_to_fit();
            }
        }
        UiEffect::CoreCommand(_) | UiEffect::SetUiVisibility(_) | UiEffect::WindowAction(_) => {}
    }
    events.push(HostEvent::Ui(effect));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_messages_are_truncated_at_a_character_boundary_in_queue_order() {
        let events = Mutex::new(Vec::new());
        let maximum = "界".repeat(MAX_MESSAGE_CHARS);
        record(&events, UiEffect::Log(format!("{maximum}尾")));
        record(&events, UiEffect::SetUiVisibility(true));
        record(&events, UiEffect::ShowOsd("now playing".into()));

        assert_eq!(
            *riina_core::sync::lock(&events),
            [
                HostEvent::Ui(UiEffect::Log(maximum)),
                HostEvent::Ui(UiEffect::SetUiVisibility(true)),
                HostEvent::Ui(UiEffect::ShowOsd("now playing".into())),
            ]
        );
    }
}
