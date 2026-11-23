//! Application-lifetime owner for enabled plugin entry sessions.

mod playlist_menu;
mod ui_activity;

use crate::{
    plugin_files::PluginPlayerFileContext,
    plugin_runtime::{HostEvent, PluginRuntimeSession, UiEffect, input, utilities::PluginDialogs},
    plugin_snapshot::{playback_history_values, recent_document_values, version_snapshot},
};
pub(crate) use playlist_menu::PlaylistMenuContribution;
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
use ui_activity::UiActivity;

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
    SubtitleProviders(String, Vec<riina_core::plugin::SubtitleProvider>),
}

#[derive(Clone)]
pub(crate) struct PluginHostHandle {
    pub(crate) dialogs: PluginDialogs,
    pub(crate) input: input::Router,
    commands: mpsc::SyncSender<PluginCommand>,
    messages: Arc<Mutex<Vec<PluginHostMessage>>>,
    ui_activity: Arc<UiActivity>,
}

impl PluginHostHandle {
    pub(crate) fn run_subtitle_provider(
        &self,
        key: String,
        job: crate::plugin_runtime::subtitles::Job,
    ) {
        let command = PluginCommand::RunSubtitleProvider(key, job);
        if let Err(mpsc::TrySendError::Full(command) | mpsc::TrySendError::Disconnected(command)) =
            self.commands.try_send(command)
            && let PluginCommand::RunSubtitleProvider(_, job) = command
        {
            job.fail("The subtitle plugin host is unavailable or busy".into());
        }
    }

    pub(crate) fn after_initial_plugins(&self, ready: impl FnOnce() + Send + 'static) {
        // The host defers queued work until all startup entry points have run.
        let command = PluginCommand::AfterInitialPlugins(Box::new(ready));
        if let Err(mpsc::TrySendError::Full(command) | mpsc::TrySendError::Disconnected(command)) =
            self.commands.try_send(command)
            && let PluginCommand::AfterInitialPlugins(ready) = command
        {
            ready();
        }
    }

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

    pub(crate) fn has_active_sessions(&self) -> bool {
        self.ui_activity.has_active_sessions()
    }

    pub(crate) fn set_ui_waker(&self, wake: impl Fn() + Send + Sync + 'static) {
        self.ui_activity.set_waker(wake);
    }
}

pub(crate) struct PluginHost {
    handle: PluginHostHandle,
    player_start: Option<mpsc::SyncSender<Option<riina_mpv::CommandSender>>>,
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
    #[cfg(test)]
    pub(crate) fn start(
        preferences_path: &Path,
        recent_media: &[riina_core::RecentMedia],
        playback_history: &[riina_core::playback_history::Entry],
    ) -> Result<Self, String> {
        let mut host = Self::prepare(preferences_path, recent_media, playback_history)?;
        host.start_player(None);
        Ok(host)
    }

    pub(crate) fn prepare(
        preferences_path: &Path,
        recent_media: &[riina_core::RecentMedia],
        playback_history: &[riina_core::playback_history::Entry],
    ) -> Result<Self, String> {
        let plugins_directory = preferences_path.with_file_name("Plugins");
        let permissions_path = preferences_path.with_file_name("plugin-permissions.json");
        let enabled_path = preferences_path.with_file_name("plugin-enabled.json");
        let (commands, receiver) = mpsc::sync_channel(64);
        let (player_start, player_ready) = mpsc::sync_channel(1);
        let messages = Arc::new(Mutex::new(Vec::new()));
        let worker_messages = messages.clone();
        let ui_activity = Arc::new(UiActivity::new());
        let worker_activity = ui_activity.clone();
        let input = input::Router::default();
        let worker_input = input.clone();
        let dialogs = PluginDialogs::default();
        let worker_dialogs = dialogs.clone();
        let initial_player_context = PluginPlayerFileContext::default()
            .with_core_version(version_snapshot(None))
            .with_recent_documents(recent_document_values(recent_media))
            .with_playback_history(playback_history_values(playback_history));
        let worker = crate::background::spawn_named("riina-plugin-host", move || {
            let Ok(player_mpv) = player_ready.recv() else {
                return;
            };
            crate::plugin_packages::sweep_staging(&plugins_directory);
            run_host(
                PluginHostPaths {
                    dialogs: worker_dialogs,
                    input: worker_input,
                    plugins_directory,
                    permissions_path,
                    enabled_path,
                    player_mpv,
                },
                worker_messages,
                initial_player_context,
                receiver,
                &worker_activity,
            );
        })
        .map_err(|error| format!("Could not start plugin host: {error}"))?;
        Ok(Self {
            handle: PluginHostHandle {
                dialogs,
                input,
                commands,
                messages,
                ui_activity,
            },
            player_start: Some(player_start),
            worker: Some(worker),
        })
    }

    pub(crate) fn handle(&self) -> PluginHostHandle {
        self.handle.clone()
    }

    pub(crate) fn attach_player(&mut self, commands: riina_mpv::CommandSender) {
        self.start_player(Some(commands));
    }

    fn start_player(&mut self, commands: Option<riina_mpv::CommandSender>) {
        if let Some(ready) = self.player_start.take() {
            let _ = ready.send(commands);
        }
    }
}

impl Drop for PluginHost {
    fn drop(&mut self) {
        // Bootstrap can be waiting on a UI dialog before reading the command queue.
        self.handle.dialogs.stop();
        // An engine startup failure closes the gate before joining the host.
        drop(self.player_start.take());
        // A disconnected receiver means the host has already stopped.
        let _ = self.handle.commands.send(PluginCommand::Shutdown);
        if let Some(worker) = self.worker.take() {
            // The named worker records panics before returning.
            let _ = worker.join();
        }
    }
}

enum PluginCommand {
    AfterInitialPlugins(Box<dyn FnOnce() + Send>),
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
    BuildPlaylistMenu(playlist_menu::BuildRequest),
    RunSubtitleProvider(String, crate::plugin_runtime::subtitles::Job),
    ActivatePlaylistMenuItem {
        request: u64,
        identifier: String,
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
    paths: PluginHostPaths,
    messages: Arc<Mutex<Vec<PluginHostMessage>>>,
    initial_player_context: PluginPlayerFileContext,
    commands: mpsc::Receiver<PluginCommand>,
    ui_activity: &UiActivity,
) {
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
                    player_context.replace_playback_snapshot(context);
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
            &paths,
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
        let active = !sessions.is_empty();
        ui_activity.publish(active, || !riina_core::sync::lock(&messages).is_empty());
        // Without entry sessions there are no plugin timers or callbacks to drain.
        // Enabling, reloading and shutting down all arrive through the command queue.
        let command = if active {
            commands.recv_timeout(HOST_EVENT_POLL_INTERVAL)
        } else {
            commands
                .recv()
                .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
        };
        match command {
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
    dialogs: PluginDialogs,
    input: input::Router,
    plugins_directory: PathBuf,
    permissions_path: PathBuf,
    enabled_path: PathBuf,
    player_mpv: Option<riina_mpv::CommandSender>,
}

struct PluginDirectory {
    path: PathBuf,
    name: String,
    subtitle_providers: Vec<riina_core::plugin::SubtitleProvider>,
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
        PluginCommand::AfterInitialPlugins(ready) => ready(),
        PluginCommand::Enable {
            manifest,
            directory,
        } => {
            if let Err(error) = activate(
                &manifest,
                &directory,
                paths,
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
            remove_plugin(identifier, messages, sessions, plugin_directories);
        }
        PluginCommand::SuspendForUpdate {
            identifier,
            response,
        } => {
            remove_plugin(identifier, messages, sessions, plugin_directories);
            // The updater may have timed out before the host completed the request.
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
                paths,
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
            // Activation is complete even if the updater has timed out.
            let _ = response.send(result);
        }
        PluginCommand::UpdatePlayerFileContext(context) => {
            let events = player_context.player_events_to(&context);
            player_context.replace_playback_snapshot(context);
            update_player_sessions(player_context, &events, messages, sessions);
        }
        PluginCommand::UpdateWindowProperties(properties) => {
            let events = player_context.window_events_to(&properties);
            player_context.set_core_window_properties(properties);
            update_player_sessions(player_context, &events, messages, sessions);
        }
        PluginCommand::WindowLoaded => {
            if !std::mem::replace(window_loaded, true) {
                for key in sessions.keys().filter(|key| !key.global) {
                    deliver_window_loaded(&key.identifier, messages, sessions);
                }
            }
        }
        PluginCommand::PluginOverlayLoaded => {
            broadcast_player_event("iina.plugin-overlay-loaded", messages, sessions);
        }
        PluginCommand::ThumbnailsReady => {
            broadcast_player_event("iina.thumbnails-ready", messages, sessions);
        }
        PluginCommand::MenuUpdate => {
            broadcast_player_event("iina.menu-update", messages, sessions);
        }
        PluginCommand::ReloadAll => reload_enabled_plugins(
            paths,
            messages,
            sessions,
            plugin_directories,
            player_context,
            *window_loaded,
        ),
        PluginCommand::WindowWillClose(response) => {
            flush_closing_sessions(messages, sessions);
            // The caller can finish closing before the acknowledgement arrives.
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
        } => activate_menu_item(&identifier, global, item_id, messages, sessions),
        PluginCommand::BuildPlaylistMenu(request) => {
            request.run(sessions, plugin_directories, &paths.enabled_path);
        }
        PluginCommand::RunSubtitleProvider(key, job) => {
            run_subtitle_provider(&key, job, sessions, plugin_directories);
        }
        PluginCommand::ActivatePlaylistMenuItem {
            request,
            identifier,
            item_id,
        } => {
            if let Some(session) = sessions.get(&PluginInstanceKey::player(&identifier)) {
                session.activate_playlist_menu_item(request, item_id);
            }
        }
        PluginCommand::Shutdown => return true,
    }
    pump_session_events(messages, sessions, plugin_directories);
    false
}

fn run_subtitle_provider(
    key: &str,
    mut job: crate::plugin_runtime::subtitles::Job,
    sessions: &BTreeMap<PluginInstanceKey, PluginRuntimeSession>,
    directories: &BTreeMap<String, PluginDirectory>,
) {
    if let Some((identifier, provider)) = key
        .strip_prefix("plugin:")
        .and_then(|key| key.split_once(':'))
        && directories.get(identifier).is_some_and(|directory| {
            directory
                .subtitle_providers
                .iter()
                .any(|entry| entry.id == provider)
        })
        && let Some(session) = sessions.get(&PluginInstanceKey::player(identifier))
    {
        if let crate::plugin_runtime::subtitles::Operation::Search {
            provider: requested,
            ..
        } = &mut job.operation
        {
            *requested = provider.to_owned();
        }
        session.run_subtitle_provider(job);
    } else {
        job.fail("The selected subtitle provider is no longer available".into());
    }
}

fn activate_menu_item(
    identifier: &str,
    global: bool,
    item_id: String,
    messages: &Mutex<Vec<PluginHostMessage>>,
    sessions: &BTreeMap<PluginInstanceKey, PluginRuntimeSession>,
) {
    let key = if global {
        PluginInstanceKey::global(identifier)
    } else {
        PluginInstanceKey::player(identifier)
    };
    if let Some(session) = sessions.get(&key)
        && let Err(error) = session.deliver_plugin_menu_action(item_id)
    {
        record(messages, PluginHostMessage::Status(error));
    }
}

fn remove_plugin(
    identifier: String,
    messages: &Mutex<Vec<PluginHostMessage>>,
    sessions: &mut BTreeMap<PluginInstanceKey, PluginRuntimeSession>,
    plugin_directories: &mut BTreeMap<String, PluginDirectory>,
) {
    sessions.retain(|key, _| key.identifier != identifier);
    plugin_directories.remove(&identifier);
    remove_plugin_ui(identifier, messages);
}

fn remove_plugin_ui(identifier: String, messages: &Mutex<Vec<PluginHostMessage>>) {
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

fn update_player_sessions(
    player_context: &PluginPlayerFileContext,
    events: &[(String, serde_json::Value)],
    messages: &Mutex<Vec<PluginHostMessage>>,
    sessions: &BTreeMap<PluginInstanceKey, PluginRuntimeSession>,
) {
    for (key, session) in sessions.iter() {
        if !key.global {
            session.update_player_file_context(player_context.clone());
            for (name, data) in events {
                if session.is_subscribed_to_player_event(name)
                    && let Err(error) = session.deliver_player_event(name.clone(), data.clone())
                {
                    record(messages, PluginHostMessage::Status(error));
                }
            }
        }
    }
}

fn broadcast_player_event(
    name: &str,
    messages: &Mutex<Vec<PluginHostMessage>>,
    sessions: &BTreeMap<PluginInstanceKey, PluginRuntimeSession>,
) {
    for (key, session) in sessions {
        if !key.global
            && session.is_subscribed_to_player_event(name)
            && let Err(error) = session.deliver_player_event(name.into(), serde_json::Value::Null)
        {
            record(messages, PluginHostMessage::Status(error));
        }
    }
}

fn reload_enabled_plugins(
    paths: &PluginHostPaths,
    messages: &Arc<Mutex<Vec<PluginHostMessage>>>,
    sessions: &mut BTreeMap<PluginInstanceKey, PluginRuntimeSession>,
    plugin_directories: &mut BTreeMap<String, PluginDirectory>,
    player_context: &PluginPlayerFileContext,
    window_loaded: bool,
) {
    let identifiers = sessions
        .keys()
        .map(|key| key.identifier.clone())
        .collect::<BTreeSet<_>>();
    sessions.clear();
    for identifier in identifiers {
        remove_plugin_ui(identifier, messages);
    }
    plugin_directories.clear();
    let mut succeeded = 0;
    for plugin in enabled_packages(&paths.plugins_directory, &paths.enabled_path) {
        match activate(
            &plugin.manifest,
            &plugin.directory,
            paths,
            messages,
            sessions,
            plugin_directories,
            player_context,
        ) {
            Ok(()) => {
                succeeded += 1;
                deliver_activation_events(
                    &plugin.manifest.identifier,
                    window_loaded,
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

fn flush_closing_sessions(
    messages: &Mutex<Vec<PluginHostMessage>>,
    sessions: &BTreeMap<PluginInstanceKey, PluginRuntimeSession>,
) {
    let mut notified_sessions = Vec::new();
    for (key, session) in sessions.iter() {
        if !key.global && session.is_subscribed_to_player_event("iina.window-will-close") {
            match session
                .deliver_player_event("iina.window-will-close".into(), serde_json::Value::Null)
            {
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
}

fn deliver_window_loaded(
    identifier: &str,
    messages: &Arc<Mutex<Vec<PluginHostMessage>>>,
    sessions: &BTreeMap<PluginInstanceKey, PluginRuntimeSession>,
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
    sessions: &BTreeMap<PluginInstanceKey, PluginRuntimeSession>,
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
                HostEvent::Ui(effect) => record(messages, host_message(&key.identifier, effect)),
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
    let order = read_plugin_order(enabled_path);
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

fn read_plugin_order(enabled_path: &Path) -> Vec<String> {
    fs::read_to_string(enabled_path.with_file_name("plugin-order.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn activate(
    manifest: &Manifest,
    directory: &Path,
    paths: &PluginHostPaths,
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
    let grants = load_grants(&paths.permissions_path, &manifest.identifier)?;
    let global = manifest
        .global_entry
        .as_ref()
        .map(|_| {
            PluginRuntimeSession::start_package_global_entry(
                directory,
                manifest,
                &grants,
                paths.dialogs.clone(),
            )
            .map_err(|error| format!("Could not start global {}: {error}", manifest.name))
        })
        .transpose()?;
    let session = PluginRuntimeSession::start_package_entry_with_player_context(
        directory,
        manifest,
        &grants,
        player_context.clone(),
        paths.player_mpv.clone(),
        paths.input.bindings.clone(),
        paths.dialogs.clone(),
    )
    .map_err(|error| format!("Could not start {}: {error}", manifest.name))?;
    if let Some(global) = global {
        sessions.insert(PluginInstanceKey::global(&manifest.identifier), global);
    }
    paths.input.add(&session.input);
    sessions.insert(player_key, session);
    plugin_directories.insert(
        manifest.identifier.clone(),
        PluginDirectory {
            path: directory.to_path_buf(),
            name: manifest.name.clone(),
            subtitle_providers: manifest.subtitle_providers.clone(),
        },
    );
    if !manifest.subtitle_providers.is_empty() {
        record(
            messages,
            PluginHostMessage::SubtitleProviders(
                manifest.identifier.clone(),
                manifest.subtitle_providers.clone(),
            ),
        );
    }
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

fn host_message(plugin_name: &str, effect: UiEffect) -> PluginHostMessage {
    match effect {
        UiEffect::Log(message) => PluginHostMessage::Log(format!("{plugin_name}: {message}")),
        UiEffect::ShowOsd(message) => PluginHostMessage::ShowOsd(message),
        UiEffect::ShowAlert(message) => {
            PluginHostMessage::ShowAlert(plugin_name.to_owned(), message)
        }
        UiEffect::CoreCommand(command) => PluginHostMessage::PlayerCommand(command),
        UiEffect::SetUiVisibility(visible) => PluginHostMessage::SetUiVisibility(visible),
        UiEffect::WindowAction(action) => PluginHostMessage::WindowAction(action),
    }
}

fn record(messages: &Mutex<Vec<PluginHostMessage>>, message: PluginHostMessage) {
    let mut messages = riina_core::sync::lock(messages);
    if messages.len() < MAX_PENDING_MESSAGES {
        messages.push(message);
    }
}

#[cfg(test)]
mod tests;
