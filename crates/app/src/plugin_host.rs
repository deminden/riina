//! Application-lifetime owner for enabled plugin entry sessions.

use crate::{
    plugin_files::PluginPlayerFileContext,
    plugin_runtime::{HostEvent, PluginRuntimeSession},
    plugin_snapshot::{playback_history_values, recent_document_values, version_snapshot},
};
use riina_core::{
    Command,
    plugin::{Manifest, OverlayEffect, WindowAction},
    plugin_menu::PluginMenuItem,
    plugin_sidebar::PluginSidebarEffect,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    thread::JoinHandle,
    time::Duration,
};

const MAX_PENDING_MESSAGES: usize = 256;
// Timers and cross-context callbacks produce events while no host command arrives.
const HOST_EVENT_POLL_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PluginHostMessage {
    Log(String),
    ShowOsd(String),
    ShowAlert(String, String),
    Status(String),
    PlayerCommand(Command),
    SetUiVisibility(bool),
    WindowAction(WindowAction),
    ObserveMpvProperty(String),
    Overlay {
        plugin_identifier: String,
        plugin_directory: PathBuf,
        effect: OverlayEffect,
    },
    RemoveOverlay(String),
    Sidebar {
        plugin_identifier: String,
        plugin_directory: PathBuf,
        effect: PluginSidebarEffect,
    },
    RemoveSidebar(String),
    SidebarTabAdded {
        plugin_identifier: String,
        label: String,
    },
    PluginMenuUpdated {
        plugin_identifier: String,
        plugin_name: String,
        global: bool,
        items: Vec<PluginMenuItem>,
    },
    RemovePluginMenu(String),
}

#[derive(Clone)]
pub(crate) struct PluginHostHandle {
    commands: mpsc::SyncSender<PluginCommand>,
    messages: Arc<Mutex<Vec<PluginHostMessage>>>,
}

impl PluginHostHandle {
    pub(crate) fn enable(&self, manifest: Manifest, directory: PathBuf) -> Result<(), String> {
        self.commands
            .try_send(PluginCommand::Enable {
                manifest: Box::new(manifest),
                directory,
            })
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => "Plugin host is busy".to_owned(),
                mpsc::TrySendError::Disconnected(_) => "Plugin host has stopped".to_owned(),
            })
    }

    pub(crate) fn disable(&self, identifier: String) -> Result<(), String> {
        self.commands
            .try_send(PluginCommand::Disable(identifier))
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => "Plugin host is busy".to_owned(),
                mpsc::TrySendError::Disconnected(_) => "Plugin host has stopped".to_owned(),
            })
    }

    pub(crate) fn update_player_file_context(
        &self,
        context: PluginPlayerFileContext,
    ) -> Result<(), String> {
        self.commands
            .try_send(PluginCommand::UpdatePlayerFileContext(context))
            .map_err(command_error)
    }

    pub(crate) fn update_window_properties(
        &self,
        properties: BTreeMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        self.commands
            .try_send(PluginCommand::UpdateWindowProperties(properties))
            .map_err(command_error)
    }

    pub(crate) fn notify_window_loaded(&self) -> Result<(), String> {
        self.commands
            .try_send(PluginCommand::WindowLoaded)
            .map_err(command_error)
    }

    pub(crate) fn notify_plugin_overlay_loaded(&self) -> Result<(), String> {
        self.commands
            .try_send(PluginCommand::PluginOverlayLoaded)
            .map_err(command_error)
    }

    pub(crate) fn notify_thumbnails_ready(&self) -> Result<(), String> {
        self.commands
            .try_send(PluginCommand::ThumbnailsReady)
            .map_err(command_error)
    }

    pub(crate) fn notify_menu_update(&self) -> Result<(), String> {
        self.commands
            .try_send(PluginCommand::MenuUpdate)
            .map_err(command_error)
    }

    pub(crate) fn reload_all(&self) -> Result<(), String> {
        self.commands
            .try_send(PluginCommand::ReloadAll)
            .map_err(command_error)
    }

    pub(crate) fn notify_window_will_close(&self) -> Result<(), String> {
        let (response, result) = mpsc::sync_channel(1);
        self.commands
            .try_send(PluginCommand::WindowWillClose(response))
            .map_err(command_error)?;
        result
            .recv_timeout(Duration::from_secs(2))
            .map_err(|error| format!("Plugin host did not prepare window shutdown: {error}"))
    }

    pub(crate) fn suspend_for_update(&self, identifier: String) -> Result<(), String> {
        let (response, result) = mpsc::sync_channel(1);
        self.commands
            .try_send(PluginCommand::SuspendForUpdate {
                identifier,
                response,
            })
            .map_err(command_error)?;
        result
            .recv_timeout(Duration::from_secs(30))
            .map_err(|error| format!("Plugin host did not stop the plugin for update: {error}"))?
    }

    pub(crate) fn activate_after_update(
        &self,
        manifest: Manifest,
        directory: PathBuf,
    ) -> Result<(), String> {
        let (response, result) = mpsc::sync_channel(1);
        self.commands
            .try_send(PluginCommand::ActivateAfterUpdate {
                manifest: Box::new(manifest),
                directory,
                response,
            })
            .map_err(command_error)?;
        result
            .recv_timeout(Duration::from_secs(30))
            .map_err(|error| format!("Plugin host did not restart the updated plugin: {error}"))?
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn deliver_overlay_message(
        &self,
        identifier: String,
        name: String,
        data: serde_json::Value,
    ) -> Result<(), String> {
        self.commands
            .try_send(PluginCommand::DeliverOverlayMessage {
                identifier,
                name,
                data,
            })
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => "Plugin host is busy".to_owned(),
                mpsc::TrySendError::Disconnected(_) => "Plugin host has stopped".to_owned(),
            })
    }

    pub(crate) fn deliver_sidebar_message(
        &self,
        identifier: String,
        name: String,
        data: serde_json::Value,
    ) -> Result<(), String> {
        self.commands
            .try_send(PluginCommand::DeliverSidebarMessage {
                identifier,
                name,
                data,
            })
            .map_err(command_error)
    }

    pub(crate) fn activate_plugin_menu_item(
        &self,
        identifier: String,
        global: bool,
        item_id: String,
    ) -> Result<(), String> {
        self.commands
            .try_send(PluginCommand::ActivatePluginMenuItem {
                identifier,
                global,
                item_id,
            })
            .map_err(command_error)
    }

    pub(crate) fn drain_messages(&self) -> Result<Vec<PluginHostMessage>, String> {
        let mut messages = self
            .messages
            .lock()
            .map_err(|_| "Plugin host message queue was poisoned".to_owned())?;
        Ok(std::mem::take(&mut *messages))
    }
}

pub(crate) struct PluginHost {
    handle: PluginHostHandle,
    worker: Option<JoinHandle<()>>,
}

pub(crate) fn is_enabled_plugin(preferences_path: &Path, identifier: &str) -> bool {
    enabled_packages(
        &preferences_path.with_file_name("Plugins"),
        &preferences_path.with_file_name("plugin-enabled.json"),
    )
    .iter()
    .any(|plugin| plugin.manifest.identifier == identifier)
}

impl PluginHost {
    pub(crate) fn start(
        preferences_path: &Path,
        recent_media: &[riina_core::RecentMedia],
        playback_history: &[riina_core::playback_history::Entry],
    ) -> Result<Self, String> {
        let plugins_directory = preferences_path.with_file_name("Plugins");
        let permissions_path = preferences_path.with_file_name("plugin-permissions.json");
        let enabled_path = preferences_path.with_file_name("plugin-enabled.json");
        let (commands, receiver) = mpsc::sync_channel(64);
        let messages = Arc::new(Mutex::new(Vec::new()));
        let worker_messages = messages.clone();
        let initial_player_context = PluginPlayerFileContext::default()
            .with_core_version(version_snapshot(None))
            .with_recent_documents(recent_document_values(recent_media))
            .with_playback_history(playback_history_values(playback_history));
        let worker = std::thread::Builder::new()
            .name("riina-plugin-host".into())
            .spawn(move || {
                run_host(
                    plugins_directory,
                    permissions_path,
                    enabled_path,
                    worker_messages,
                    initial_player_context,
                    receiver,
                );
            })
            .map_err(|error| format!("Could not start plugin host: {error}"))?;
        Ok(Self {
            handle: PluginHostHandle { commands, messages },
            worker: Some(worker),
        })
    }

    pub(crate) fn handle(&self) -> PluginHostHandle {
        self.handle.clone()
    }
}

impl Drop for PluginHost {
    fn drop(&mut self) {
        let _ = self.handle.commands.send(PluginCommand::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

enum PluginCommand {
    Enable {
        manifest: Box<Manifest>,
        directory: PathBuf,
    },
    Disable(String),
    SuspendForUpdate {
        identifier: String,
        response: mpsc::SyncSender<Result<(), String>>,
    },
    ActivateAfterUpdate {
        manifest: Box<Manifest>,
        directory: PathBuf,
        response: mpsc::SyncSender<Result<(), String>>,
    },
    UpdatePlayerFileContext(PluginPlayerFileContext),
    UpdateWindowProperties(BTreeMap<String, serde_json::Value>),
    WindowLoaded,
    PluginOverlayLoaded,
    ThumbnailsReady,
    MenuUpdate,
    ReloadAll,
    WindowWillClose(mpsc::SyncSender<()>),
    #[cfg(target_os = "windows")]
    DeliverOverlayMessage {
        identifier: String,
        name: String,
        data: serde_json::Value,
    },
    DeliverSidebarMessage {
        identifier: String,
        name: String,
        data: serde_json::Value,
    },
    ActivatePluginMenuItem {
        identifier: String,
        global: bool,
        item_id: String,
    },
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PluginInstanceKey {
    identifier: String,
    global: bool,
}

impl PluginInstanceKey {
    fn player(identifier: &str) -> Self {
        Self {
            identifier: identifier.to_owned(),
            global: false,
        }
    }

    fn global(identifier: &str) -> Self {
        Self {
            identifier: identifier.to_owned(),
            global: true,
        }
    }
}

fn run_host(
    plugins_directory: PathBuf,
    permissions_path: PathBuf,
    enabled_path: PathBuf,
    messages: Arc<Mutex<Vec<PluginHostMessage>>>,
    initial_player_context: PluginPlayerFileContext,
    commands: mpsc::Receiver<PluginCommand>,
) {
    let paths = PluginHostPaths {
        plugins_directory,
        permissions_path,
        enabled_path,
    };
    let mut sessions = BTreeMap::<PluginInstanceKey, PluginRuntimeSession>::new();
    let mut plugin_directories = BTreeMap::<String, PluginDirectory>::new();
    let mut player_context = initial_player_context;
    let mut window_loaded = false;
    let mut deferred = Vec::new();
    for plugin in enabled_packages(&paths.plugins_directory, &paths.enabled_path) {
        loop {
            match commands.try_recv() {
                Ok(PluginCommand::Shutdown) => return,
                Ok(PluginCommand::UpdatePlayerFileContext(context)) => {
                    player_context = context;
                }
                Ok(PluginCommand::UpdateWindowProperties(properties)) => {
                    player_context.set_core_window_properties(properties);
                }
                Ok(command) => deferred.push(command),
                Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected) => break,
            }
        }
        if let Err(error) = activate(
            &plugin.manifest,
            &plugin.directory,
            &paths.permissions_path,
            &messages,
            &mut sessions,
            &mut plugin_directories,
            &player_context,
        ) {
            record(&messages, PluginHostMessage::Status(error));
        }
        pump_session_events(&messages, &mut sessions, &plugin_directories);
    }
    for command in deferred {
        if handle_command(
            command,
            &paths,
            &messages,
            &mut sessions,
            &mut plugin_directories,
            &mut player_context,
            &mut window_loaded,
        ) {
            return;
        }
    }

    loop {
        match commands.recv_timeout(HOST_EVENT_POLL_INTERVAL) {
            Ok(command) => {
                if handle_command(
                    command,
                    &paths,
                    &messages,
                    &mut sessions,
                    &mut plugin_directories,
                    &mut player_context,
                    &mut window_loaded,
                ) {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        pump_session_events(&messages, &mut sessions, &plugin_directories);
    }
}

struct PluginHostPaths {
    plugins_directory: PathBuf,
    permissions_path: PathBuf,
    enabled_path: PathBuf,
}

struct PluginDirectory {
    path: PathBuf,
    name: String,
}

fn handle_command(
    command: PluginCommand,
    paths: &PluginHostPaths,
    messages: &Arc<Mutex<Vec<PluginHostMessage>>>,
    sessions: &mut BTreeMap<PluginInstanceKey, PluginRuntimeSession>,
    plugin_directories: &mut BTreeMap<String, PluginDirectory>,
    player_context: &mut PluginPlayerFileContext,
    window_loaded: &mut bool,
) -> bool {
    match command {
        PluginCommand::Enable {
            manifest,
            directory,
        } => {
            if let Err(error) = activate(
                &manifest,
                &directory,
                &paths.permissions_path,
                messages,
                sessions,
                plugin_directories,
                player_context,
            ) {
                record(messages, PluginHostMessage::Status(error));
            } else {
                deliver_activation_events(
                    &manifest.identifier,
                    *window_loaded,
                    player_context,
                    messages,
                    sessions,
                );
            }
        }
        PluginCommand::Disable(identifier) => {
            sessions.retain(|key, _| key.identifier != identifier);
            plugin_directories.remove(&identifier);
            record(
                messages,
                PluginHostMessage::RemoveOverlay(identifier.clone()),
            );
            record(
                messages,
                PluginHostMessage::RemoveSidebar(identifier.clone()),
            );
            record(messages, PluginHostMessage::RemovePluginMenu(identifier));
        }
        PluginCommand::SuspendForUpdate {
            identifier,
            response,
        } => {
            sessions.retain(|key, _| key.identifier != identifier);
            plugin_directories.remove(&identifier);
            record(
                messages,
                PluginHostMessage::RemoveOverlay(identifier.clone()),
            );
            record(
                messages,
                PluginHostMessage::RemoveSidebar(identifier.clone()),
            );
            record(messages, PluginHostMessage::RemovePluginMenu(identifier));
            let _ = response.send(Ok(()));
        }
        PluginCommand::ActivateAfterUpdate {
            manifest,
            directory,
            response,
        } => {
            let result = activate(
                &manifest,
                &directory,
                &paths.permissions_path,
                messages,
                sessions,
                plugin_directories,
                player_context,
            );
            if let Err(error) = &result {
                record(messages, PluginHostMessage::Status(error.clone()));
            } else {
                deliver_activation_events(
                    &manifest.identifier,
                    *window_loaded,
                    player_context,
                    messages,
                    sessions,
                );
            }
            let _ = response.send(result);
        }
        PluginCommand::UpdatePlayerFileContext(context) => {
            let events = player_context.player_events_to(&context);
            *player_context = context;
            for (key, session) in sessions.iter() {
                if !key.global {
                    session.update_player_file_context(player_context.clone());
                    for (name, data) in &events {
                        if session.is_subscribed_to_player_event(name)
                            && let Err(error) =
                                session.deliver_player_event(name.clone(), data.clone())
                        {
                            record(messages, PluginHostMessage::Status(error));
                        }
                    }
                }
            }
        }
        PluginCommand::UpdateWindowProperties(properties) => {
            let events = player_context.window_events_to(&properties);
            player_context.set_core_window_properties(properties);
            for (key, session) in sessions.iter() {
                if !key.global {
                    session.update_player_file_context(player_context.clone());
                    for (name, data) in &events {
                        if session.is_subscribed_to_player_event(name)
                            && let Err(error) =
                                session.deliver_player_event(name.clone(), data.clone())
                        {
                            record(messages, PluginHostMessage::Status(error));
                        }
                    }
                }
            }
        }
        PluginCommand::WindowLoaded => {
            if !*window_loaded {
                *window_loaded = true;
                let identifiers = sessions
                    .keys()
                    .filter(|key| !key.global)
                    .map(|key| key.identifier.clone())
                    .collect::<Vec<_>>();
                for identifier in identifiers {
                    deliver_window_loaded(&identifier, messages, sessions);
                }
            }
        }
        PluginCommand::PluginOverlayLoaded => {
            for (key, session) in sessions.iter() {
                if !key.global
                    && session.is_subscribed_to_player_event("iina.plugin-overlay-loaded")
                    && let Err(error) = session.deliver_player_event(
                        "iina.plugin-overlay-loaded".into(),
                        serde_json::Value::Null,
                    )
                {
                    record(messages, PluginHostMessage::Status(error));
                }
            }
        }
        PluginCommand::ThumbnailsReady => {
            for (key, session) in sessions.iter() {
                if !key.global
                    && session.is_subscribed_to_player_event("iina.thumbnails-ready")
                    && let Err(error) = session.deliver_player_event(
                        "iina.thumbnails-ready".into(),
                        serde_json::Value::Null,
                    )
                {
                    record(messages, PluginHostMessage::Status(error));
                }
            }
        }
        PluginCommand::MenuUpdate => {
            for (key, session) in sessions.iter() {
                if !key.global
                    && session.is_subscribed_to_player_event("iina.menu-update")
                    && let Err(error) = session
                        .deliver_player_event("iina.menu-update".into(), serde_json::Value::Null)
                {
                    record(messages, PluginHostMessage::Status(error));
                }
            }
        }
        PluginCommand::ReloadAll => {
            let identifiers = sessions
                .keys()
                .map(|key| key.identifier.clone())
                .collect::<BTreeSet<_>>();
            sessions.clear();
            for identifier in &identifiers {
                record(
                    messages,
                    PluginHostMessage::RemoveOverlay(identifier.clone()),
                );
                record(
                    messages,
                    PluginHostMessage::RemoveSidebar(identifier.clone()),
                );
                record(
                    messages,
                    PluginHostMessage::RemovePluginMenu(identifier.clone()),
                );
            }
            plugin_directories.clear();
            let mut succeeded = 0;
            for plugin in enabled_packages(&paths.plugins_directory, &paths.enabled_path) {
                match activate(
                    &plugin.manifest,
                    &plugin.directory,
                    &paths.permissions_path,
                    messages,
                    sessions,
                    plugin_directories,
                    player_context,
                ) {
                    Ok(()) => {
                        succeeded += 1;
                        deliver_activation_events(
                            &plugin.manifest.identifier,
                            *window_loaded,
                            player_context,
                            messages,
                            sessions,
                        );
                    }
                    Err(error) => record(messages, PluginHostMessage::Status(error)),
                }
                pump_session_events(messages, sessions, plugin_directories);
            }
            record(
                messages,
                PluginHostMessage::Status(format!("Reloaded {succeeded} enabled plugin(s).")),
            );
        }
        PluginCommand::WindowWillClose(response) => {
            let mut notified_sessions = Vec::new();
            for (key, session) in sessions.iter() {
                if !key.global && session.is_subscribed_to_player_event("iina.window-will-close") {
                    match session.deliver_player_event(
                        "iina.window-will-close".into(),
                        serde_json::Value::Null,
                    ) {
                        Ok(()) => notified_sessions.push(session),
                        Err(error) => record(messages, PluginHostMessage::Status(error)),
                    }
                }
            }
            for session in notified_sessions {
                if let Err(error) = session.flush_events(Duration::from_secs(2)) {
                    record(messages, PluginHostMessage::Status(error));
                }
            }
            let _ = response.send(());
        }
        #[cfg(target_os = "windows")]
        PluginCommand::DeliverOverlayMessage {
            identifier,
            name,
            data,
        } => {
            if let Some(session) = sessions.get(&PluginInstanceKey::player(&identifier))
                && let Err(error) = session.deliver_overlay_message(name, data)
            {
                record(messages, PluginHostMessage::Status(error));
            }
        }
        PluginCommand::DeliverSidebarMessage {
            identifier,
            name,
            data,
        } => {
            if let Some(session) = sessions.get(&PluginInstanceKey::player(&identifier))
                && let Err(error) = session.deliver_sidebar_message(name, data)
            {
                record(messages, PluginHostMessage::Status(error));
            }
        }
        PluginCommand::ActivatePluginMenuItem {
            identifier,
            global,
            item_id,
        } => {
            let key = if global {
                PluginInstanceKey::global(&identifier)
            } else {
                PluginInstanceKey::player(&identifier)
            };
            if let Some(session) = sessions.get(&key)
                && let Err(error) = session.deliver_plugin_menu_action(item_id)
            {
                record(messages, PluginHostMessage::Status(error));
            }
        }
        PluginCommand::Shutdown => return true,
    }
    pump_session_events(messages, sessions, plugin_directories);
    false
}

fn deliver_window_loaded(
    identifier: &str,
    messages: &Arc<Mutex<Vec<PluginHostMessage>>>,
    sessions: &mut BTreeMap<PluginInstanceKey, PluginRuntimeSession>,
) {
    let key = PluginInstanceKey::player(identifier);
    if let Some(session) = sessions.get(&key)
        && session.is_subscribed_to_player_event("iina.window-loaded")
        && let Err(error) =
            session.deliver_player_event("iina.window-loaded".into(), serde_json::Value::Null)
    {
        record(messages, PluginHostMessage::Status(error));
    }
}

fn deliver_activation_events(
    identifier: &str,
    window_loaded: bool,
    player_context: &PluginPlayerFileContext,
    messages: &Arc<Mutex<Vec<PluginHostMessage>>>,
    sessions: &mut BTreeMap<PluginInstanceKey, PluginRuntimeSession>,
) {
    if window_loaded {
        deliver_window_loaded(identifier, messages, sessions);
    }
    if player_context.mpv_initialized()
        && let Some(session) = sessions.get(&PluginInstanceKey::player(identifier))
        && session.is_subscribed_to_player_event("iina.mpv-initialized")
        && let Err(error) =
            session.deliver_player_event("iina.mpv-initialized".into(), serde_json::Value::Null)
    {
        record(messages, PluginHostMessage::Status(error));
    }
}

fn command_error(error: mpsc::TrySendError<PluginCommand>) -> String {
    match error {
        mpsc::TrySendError::Full(_) => "Plugin host is busy".to_owned(),
        mpsc::TrySendError::Disconnected(_) => "Plugin host has stopped".to_owned(),
    }
}

fn pump_session_events(
    messages: &Arc<Mutex<Vec<PluginHostMessage>>>,
    sessions: &mut BTreeMap<PluginInstanceKey, PluginRuntimeSession>,
    plugin_directories: &BTreeMap<String, PluginDirectory>,
) {
    let keys = sessions.keys().cloned().collect::<Vec<_>>();
    for key in keys {
        let Some(session) = sessions.get(&key) else {
            continue;
        };
        for property in session.take_mpv_property_observation_requests() {
            record(messages, PluginHostMessage::ObserveMpvProperty(property));
        }
        let events = match session.drain_events() {
            Ok(events) => events,
            Err(error) => {
                record(messages, PluginHostMessage::Status(error));
                continue;
            }
        };
        for event in events {
            match event {
                HostEvent::PluginMessage(name, data) => {
                    let peer_key = if key.global {
                        PluginInstanceKey::player(&key.identifier)
                    } else {
                        PluginInstanceKey::global(&key.identifier)
                    };
                    if let Some(peer) = sessions.get(&peer_key)
                        && let Err(error) = peer.deliver_plugin_message(name, data)
                    {
                        record(messages, PluginHostMessage::Status(error));
                    }
                }
                HostEvent::Overlay(effect) => {
                    if let Some(plugin_directory) = plugin_directories.get(&key.identifier) {
                        record(
                            messages,
                            PluginHostMessage::Overlay {
                                plugin_identifier: key.identifier.clone(),
                                plugin_directory: plugin_directory.path.clone(),
                                effect,
                            },
                        );
                    }
                }
                HostEvent::Sidebar(effect) => {
                    if let Some(plugin_directory) = plugin_directories.get(&key.identifier) {
                        record(
                            messages,
                            PluginHostMessage::Sidebar {
                                plugin_identifier: key.identifier.clone(),
                                plugin_directory: plugin_directory.path.clone(),
                                effect,
                            },
                        );
                    }
                }
                HostEvent::MenuUpdate(items) => {
                    record(
                        messages,
                        PluginHostMessage::PluginMenuUpdated {
                            plugin_identifier: key.identifier.clone(),
                            plugin_name: plugin_directories.get(&key.identifier).map_or_else(
                                || key.identifier.clone(),
                                |plugin| plugin.name.clone(),
                            ),
                            global: key.global,
                            items,
                        },
                    );
                }
                event => record(messages, host_message(&key.identifier, event)),
            }
        }
    }
}

struct InstalledPlugin {
    manifest: Manifest,
    directory: PathBuf,
}

fn enabled_packages(directory: &Path, enabled_path: &Path) -> Vec<InstalledPlugin> {
    let locale = crate::platform::language::current_locale_identifier();
    let enabled = fs::read_to_string(enabled_path)
        .ok()
        .and_then(|text| serde_json::from_str::<BTreeSet<String>>(&text).ok())
        .unwrap_or_default();
    let mut plugins = fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            (path
                .extension()
                .is_some_and(|extension| extension == "iinaplugin"))
            .then(|| {
                Manifest::load_from_directory_with_locale(&path, locale.as_deref())
                    .ok()
                    .map(|manifest| (path, manifest))
            })
            .flatten()
        })
        .filter(|(_, manifest)| enabled.contains(&manifest.identifier))
        .map(|(directory, manifest)| InstalledPlugin {
            manifest,
            directory,
        })
        .collect::<Vec<_>>();
    plugins.sort_by(|left, right| left.manifest.name.cmp(&right.manifest.name));
    let installed = plugins
        .iter()
        .map(|plugin| plugin.manifest.identifier.clone())
        .collect::<Vec<_>>();
    let order = fs::read_to_string(enabled_path.with_file_name("plugin-order.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<Vec<String>>(&text).ok())
        .unwrap_or_default();
    let order = riina_core::plugin_order::normalize(&order, &installed);
    let positions = order
        .iter()
        .enumerate()
        .map(|(position, identifier)| (identifier.as_str(), position))
        .collect::<BTreeMap<_, _>>();
    plugins.sort_by_key(|plugin| {
        positions
            .get(plugin.manifest.identifier.as_str())
            .copied()
            .unwrap_or(usize::MAX)
    });
    plugins
}

fn activate(
    manifest: &Manifest,
    directory: &Path,
    permissions_path: &Path,
    messages: &Arc<Mutex<Vec<PluginHostMessage>>>,
    sessions: &mut BTreeMap<PluginInstanceKey, PluginRuntimeSession>,
    plugin_directories: &mut BTreeMap<String, PluginDirectory>,
    player_context: &PluginPlayerFileContext,
) -> Result<(), String> {
    let player_key = PluginInstanceKey::player(&manifest.identifier);
    if sessions.contains_key(&player_key) {
        return Ok(());
    }
    if !crate::plugin_preferences::current_runtime_supports(manifest) {
        return Err(format!(
            "{} is enabled, but requests plugin capabilities that are unavailable",
            manifest.name
        ));
    }
    let grants = load_grants(permissions_path, &manifest.identifier)?;
    let global = manifest
        .global_entry
        .as_ref()
        .map(|_| {
            PluginRuntimeSession::start_package_global_entry(directory, manifest, &grants)
                .map_err(|error| format!("Could not start global {}: {error}", manifest.name))
        })
        .transpose()?;
    let session = PluginRuntimeSession::start_package_entry_with_player_context(
        directory,
        manifest,
        &grants,
        player_context.clone(),
    )
    .map_err(|error| format!("Could not start {}: {error}", manifest.name))?;
    if let Some(global) = global {
        sessions.insert(PluginInstanceKey::global(&manifest.identifier), global);
    }
    sessions.insert(player_key, session);
    plugin_directories.insert(
        manifest.identifier.clone(),
        PluginDirectory {
            path: directory.to_path_buf(),
            name: manifest.name.clone(),
        },
    );
    if let Some(label) = manifest.sidebar_tab_name.as_ref() {
        record(
            messages,
            PluginHostMessage::SidebarTabAdded {
                plugin_identifier: manifest.identifier.clone(),
                label: label.clone(),
            },
        );
    }
    record(
        messages,
        PluginHostMessage::Status(format!("{} is active.", manifest.name)),
    );
    Ok(())
}

fn load_grants(path: &Path, identifier: &str) -> Result<Vec<String>, String> {
    let grants: BTreeMap<String, Vec<String>> = match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map_err(|error| error.to_string())?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
        Err(error) => return Err(error.to_string()),
    };
    Ok(grants.get(identifier).cloned().unwrap_or_default())
}

fn host_message(plugin_name: &str, event: HostEvent) -> PluginHostMessage {
    match event {
        HostEvent::Log(message) => PluginHostMessage::Log(format!("{plugin_name}: {message}")),
        HostEvent::ShowOsd(message) => PluginHostMessage::ShowOsd(message),
        HostEvent::ShowAlert(message) => {
            PluginHostMessage::ShowAlert(plugin_name.to_owned(), message)
        }
        HostEvent::CoreCommand(command) => PluginHostMessage::PlayerCommand(command),
        HostEvent::SetUiVisibility(visible) => PluginHostMessage::SetUiVisibility(visible),
        HostEvent::WindowAction(action) => PluginHostMessage::WindowAction(action),
        HostEvent::Overlay(_) => unreachable!("overlay effects carry plugin package identity"),
        HostEvent::Sidebar(_) => unreachable!("sidebar effects carry plugin package identity"),
        HostEvent::MenuUpdate(_) => unreachable!("plugin menus carry their plugin identity"),
        HostEvent::PluginMessage(_, _) => {
            unreachable!("plugin messages are routed to their paired runtime")
        }
    }
}

fn record(messages: &Mutex<Vec<PluginHostMessage>>, message: PluginHostMessage) {
    if let Ok(mut messages) = messages.lock()
        && messages.len() < MAX_PENDING_MESSAGES
    {
        messages.push(message);
    }
}

#[cfg(test)]
mod tests;
