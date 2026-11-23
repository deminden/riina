//! Player-lifetime host for playback plugins and their application entry routes.

mod application;
mod playlist_menu;
mod ui_activity;

use crate::{
    plugin_files::PluginPlayerFileContext,
    plugin_runtime::{HostEvent, PluginRuntimeSession, PluginUi, UiEffect},
    plugin_snapshot::{playback_history_values, recent_document_values, version_snapshot},
    plugins::plugin_players::Ticket,
};
pub(crate) use application::{GlobalHandle, GlobalHost};
use application::{PackageChange, PlayerPort};
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
    sync::{Arc, Mutex, atomic::Ordering, mpsc},
    thread::JoinHandle,
    time::Duration,
};
use ui_activity::UiActivity;

const MAX_PENDING_MESSAGES: usize = 256;
// Timers and cross-context callbacks produce events while no host command arrives.
const HOST_EVENT_POLL_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlayerPluginRoute {
    pub(crate) identifier: String,
    pub(crate) realm: u64,
}

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
        route: PlayerPluginRoute,
        plugin_directory: PathBuf,
        effect: OverlayEffect,
    },
    RemoveOverlay(String),
    Sidebar {
        route: PlayerPluginRoute,
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
    pub(crate) ui: PluginUi,
    pub(crate) global: GlobalHandle,
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
        self.global
            .change(PackageChange::Enable(Arc::new(manifest), directory))
    }

    pub(crate) fn disable(&self, identifier: String) -> Result<(), String> {
        self.global.change(PackageChange::Disable(identifier))
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

    #[cfg(test)]
    fn notify_plugin_overlay_loaded(&self) -> Result<(), String> {
        self.commands
            .try_send(PluginCommand::PluginOverlayLoaded(None))
            .map_err(command_error)
    }

    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    pub(crate) fn notify_plugin_page_loaded(&self, route: PlayerPluginRoute) -> Result<(), String> {
        self.commands
            .try_send(PluginCommand::PluginOverlayLoaded(Some(route)))
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
        self.global.change(PackageChange::Reload(BTreeSet::new()))
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
        self.global
            .change_and_wait(PackageChange::Suspend(identifier))
    }

    pub(crate) fn activate_after_update(
        &self,
        manifest: Manifest,
        directory: PathBuf,
    ) -> Result<(), String> {
        self.global
            .change_and_wait(PackageChange::Enable(Arc::new(manifest), directory))
    }

    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    pub(crate) fn deliver_overlay_message(
        &self,
        route: PlayerPluginRoute,
        name: String,
        data: serde_json::Value,
    ) -> Result<(), String> {
        self.commands
            .try_send(PluginCommand::DeliverOverlayMessage { route, name, data })
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => "Plugin host is busy".to_owned(),
                mpsc::TrySendError::Disconnected(_) => "Plugin host has stopped".to_owned(),
            })
    }

    pub(crate) fn deliver_sidebar_message(
        &self,
        route: PlayerPluginRoute,
        name: String,
        data: serde_json::Value,
    ) -> Result<(), String> {
        self.commands
            .try_send(PluginCommand::DeliverSidebarMessage { route, name, data })
            .map_err(command_error)
    }

    pub(crate) fn activate_plugin_menu_item(
        &self,
        identifier: String,
        global: bool,
        item_id: String,
    ) -> Result<(), String> {
        if global {
            return self.global.activate_menu(identifier, item_id);
        }
        self.commands
            .try_send(PluginCommand::ActivatePluginMenuItem {
                identifier,
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
    port: Arc<PlayerPort>,
    player_start: Option<mpsc::SyncSender<Option<riina_mpv::CommandSender>>>,
    worker: Option<JoinHandle<()>>,
    #[cfg(test)]
    isolated_global: Option<GlobalHost>,
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

    #[cfg(test)]
    pub(crate) fn prepare(
        preferences_path: &Path,
        recent_media: &[riina_core::RecentMedia],
        playback_history: &[riina_core::playback_history::Entry],
    ) -> Result<Self, String> {
        let global = GlobalHost::start(preferences_path)?;
        let mut player = Self::prepare_for_application(
            preferences_path,
            recent_media,
            playback_history,
            global.handle(),
            None,
        )?;
        player.isolated_global = Some(global);
        Ok(player)
    }

    pub(crate) fn prepare_for_application(
        preferences_path: &Path,
        recent_media: &[riina_core::RecentMedia],
        playback_history: &[riina_core::playback_history::Entry],
        global: GlobalHandle,
        managed: Option<Arc<Ticket>>,
    ) -> Result<Self, String> {
        let plugins_directory = preferences_path.with_file_name("Plugins");
        let permissions_path = preferences_path.with_file_name("plugin-permissions.json");
        let enabled_path = preferences_path.with_file_name("plugin-enabled.json");
        let (commands, receiver) = mpsc::sync_channel(64);
        let (player_start, player_ready) = mpsc::sync_channel(1);
        let messages = Arc::new(Mutex::new(Vec::new()));
        let ui_activity = Arc::new(UiActivity::new());
        let label = managed.as_ref().map_or_else(
            || global.next_player_label(),
            |ticket| ticket.internal_label(),
        );
        let ui = PluginUi {
            player_label: managed
                .as_ref()
                .and_then(|ticket| ticket.options.label.clone()),
            ..PluginUi::default()
        };
        let port = Arc::new(PlayerPort {
            commands: commands.clone(),
            messages: messages.clone(),
            activity: ui_activity.clone(),
            closed: Default::default(),
            managed,
            label,
        });
        let worker_port = port.clone();
        let worker_global = global.clone();
        let worker_ui = ui.clone();
        let initial_player_context = PluginPlayerFileContext::default()
            .with_core_version(version_snapshot(None))
            .with_recent_documents(recent_document_values(recent_media))
            .with_playback_history(playback_history_values(playback_history));
        let worker = crate::background::spawn_named("riina-plugin-host", move || {
            let Ok(player_mpv) = player_ready.recv() else {
                return;
            };
            run_host(
                PluginHostPaths {
                    ui: worker_ui,
                    plugins_directory,
                    permissions_path,
                    enabled_path,
                    player_mpv,
                    global: worker_global,
                    port: worker_port,
                },
                initial_player_context,
                receiver,
            );
        })
        .map_err(|error| format!("Could not start plugin host: {error}"))?;
        let host = Self {
            handle: PluginHostHandle {
                ui,
                global,
                commands,
                messages,
                ui_activity,
            },
            player_start: Some(player_start),
            worker: Some(worker),
            port,
            #[cfg(test)]
            isolated_global: None,
        };
        // Register before returning the handle so even changes requested during
        // player startup reach this window in the normal command order.
        host.handle.global.join(&host.port)?;
        Ok(host)
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
        self.handle.ui.dialogs.stop();
        self.port.closed.store(true, Ordering::Release);
        if let Some(ticket) = &self.port.managed {
            ticket.close();
        }
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
    Change(
        Arc<PackageChange>,
        Option<mpsc::SyncSender<Result<(), String>>>,
    ),
    FromGlobal(String, String, serde_json::Value),
    GlobalMenu(PluginHostMessage),
    UpdatePlayerFileContext(PluginPlayerFileContext),
    UpdateWindowProperties(BTreeMap<String, serde_json::Value>),
    WindowLoaded,
    PluginOverlayLoaded(Option<PlayerPluginRoute>),
    ThumbnailsReady,
    MenuUpdate,
    WindowWillClose(mpsc::SyncSender<()>),
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    DeliverOverlayMessage {
        route: PlayerPluginRoute,
        name: String,
        data: serde_json::Value,
    },
    DeliverSidebarMessage {
        route: PlayerPluginRoute,
        name: String,
        data: serde_json::Value,
    },
    ActivatePluginMenuItem {
        identifier: String,
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
}

impl PluginInstanceKey {
    fn player(identifier: &str) -> Self {
        Self {
            identifier: identifier.to_owned(),
        }
    }
}

fn run_host(
    paths: PluginHostPaths,
    initial_player_context: PluginPlayerFileContext,
    commands: mpsc::Receiver<PluginCommand>,
) {
    let port = &paths.port;
    let messages = port.messages.clone();
    let mut sessions = BTreeMap::<PluginInstanceKey, PluginRuntimeSession>::new();
    let mut plugin_directories = BTreeMap::<String, PluginDirectory>::new();
    let mut player_context = initial_player_context;
    let mut window_loaded = false;
    let mut deferred = Vec::new();
    for plugin in enabled_packages(&paths.plugins_directory, &paths.enabled_path) {
        if !port.allows(&plugin.manifest.identifier) {
            continue;
        }
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
        let activated = paths
            .global
            .ensure(&plugin.manifest, &plugin.directory, port)
            .and_then(|()| {
                activate(
                    &plugin.manifest,
                    &plugin.directory,
                    &paths,
                    &messages,
                    &mut sessions,
                    &mut plugin_directories,
                    &player_context,
                )
            });
        if let Err(error) = activated {
            record(&messages, PluginHostMessage::Status(error));
        }
        pump_session_events(&paths, &messages, &sessions, &plugin_directories);
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
        port.activity
            .publish(active, || !riina_core::sync::lock(&messages).is_empty());
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
        pump_session_events(&paths, &messages, &sessions, &plugin_directories);
    }
}

struct PluginHostPaths {
    ui: PluginUi,
    plugins_directory: PathBuf,
    permissions_path: PathBuf,
    enabled_path: PathBuf,
    player_mpv: Option<riina_mpv::CommandSender>,
    global: GlobalHandle,
    port: Arc<PlayerPort>,
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
        PluginCommand::Change(change, response) => {
            let result = match change.as_ref() {
                PackageChange::Enable(manifest, directory) => activate(
                    manifest,
                    directory,
                    paths,
                    messages,
                    sessions,
                    plugin_directories,
                    player_context,
                )
                .map(|()| {
                    deliver_activation_events(
                        &manifest.identifier,
                        *window_loaded,
                        player_context,
                        messages,
                        sessions,
                    );
                }),
                PackageChange::Disable(identifier) | PackageChange::Suspend(identifier) => {
                    remove_plugin(identifier.clone(), messages, sessions, plugin_directories);
                    Ok(())
                }
                PackageChange::Reload(suspended) => {
                    reload_enabled_plugins(
                        paths,
                        messages,
                        sessions,
                        plugin_directories,
                        player_context,
                        *window_loaded,
                        suspended,
                    );
                    Ok(())
                }
            };
            if let Err(error) = &result {
                record(messages, PluginHostMessage::Status(error.clone()));
            }
            if let Some(response) = response {
                let _ = response.try_send(result);
            }
        }
        PluginCommand::FromGlobal(identifier, name, data) => {
            if let Some(session) = sessions.get(&PluginInstanceKey::player(&identifier))
                && let Err(error) = session.deliver_plugin_message(name, data, None)
            {
                record(messages, PluginHostMessage::Status(error));
            }
        }
        PluginCommand::GlobalMenu(message) => record(messages, message),
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
                for key in sessions.keys() {
                    deliver_window_loaded(&key.identifier, messages, sessions);
                }
            }
        }
        PluginCommand::PluginOverlayLoaded(route) => {
            if route
                .as_ref()
                .is_none_or(|route| player_plugin_session(sessions, route).is_some())
            {
                broadcast_player_event("iina.plugin-overlay-loaded", messages, sessions);
            }
        }
        PluginCommand::ThumbnailsReady => {
            broadcast_player_event("iina.thumbnails-ready", messages, sessions);
        }
        PluginCommand::MenuUpdate => {
            broadcast_player_event("iina.menu-update", messages, sessions);
        }
        PluginCommand::WindowWillClose(response) => {
            flush_closing_sessions(messages, sessions);
            // The caller can finish closing before the acknowledgement arrives.
            let _ = response.send(());
        }
        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
        PluginCommand::DeliverOverlayMessage { route, name, data } => {
            if let Some(session) = player_plugin_session(sessions, &route)
                && let Err(error) = session.deliver_overlay_message(name, data)
            {
                record(messages, PluginHostMessage::Status(error));
            }
        }
        PluginCommand::DeliverSidebarMessage { route, name, data } => {
            if let Some(session) = player_plugin_session(sessions, &route)
                && let Err(error) = session.deliver_sidebar_message(name, data)
            {
                record(messages, PluginHostMessage::Status(error));
            }
        }
        PluginCommand::ActivatePluginMenuItem {
            identifier,
            item_id,
        } => activate_menu_item(&identifier, item_id, messages, sessions),
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
    item_id: String,
    messages: &Mutex<Vec<PluginHostMessage>>,
    sessions: &BTreeMap<PluginInstanceKey, PluginRuntimeSession>,
) {
    let key = PluginInstanceKey::player(identifier);
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
    for session in sessions.values() {
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

fn broadcast_player_event(
    name: &str,
    messages: &Mutex<Vec<PluginHostMessage>>,
    sessions: &BTreeMap<PluginInstanceKey, PluginRuntimeSession>,
) {
    for session in sessions.values() {
        if session.is_subscribed_to_player_event(name)
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
    suspended: &BTreeSet<String>,
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
        if suspended.contains(&plugin.manifest.identifier) {
            continue;
        }
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
    for session in sessions.values() {
        if session.is_subscribed_to_player_event("iina.window-will-close") {
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
    paths: &PluginHostPaths,
    messages: &Arc<Mutex<Vec<PluginHostMessage>>>,
    sessions: &BTreeMap<PluginInstanceKey, PluginRuntimeSession>,
    plugin_directories: &BTreeMap<String, PluginDirectory>,
) {
    for (key, session) in sessions {
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
                HostEvent::PluginMessage(message) => {
                    if let Err(error) =
                        paths
                            .global
                            .deliver(&paths.port, key.identifier.clone(), message)
                    {
                        record(messages, PluginHostMessage::Status(error));
                    }
                }
                HostEvent::CreatePluginPlayer(_, _) => {}
                HostEvent::Overlay(effect) => {
                    if let Some(plugin_directory) = plugin_directories.get(&key.identifier) {
                        record(
                            messages,
                            PluginHostMessage::Overlay {
                                route: PlayerPluginRoute {
                                    identifier: key.identifier.clone(),
                                    realm: session.realm,
                                },
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
                                route: PlayerPluginRoute {
                                    identifier: key.identifier.clone(),
                                    realm: session.realm,
                                },
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
                            global: false,
                            items,
                        },
                    );
                }
                HostEvent::Ui(effect) => record(messages, host_message(&key.identifier, effect)),
                HostEvent::Console(message) => {
                    let component = format!("plugin/{}", key.identifier);
                    crate::diagnostics::log(message.level, &component, &message.text);
                }
            }
        }
    }
}

struct InstalledPlugin {
    manifest: Arc<Manifest>,
    directory: PathBuf,
}

fn enabled_packages(directory: &Path, enabled_path: &Path) -> Vec<InstalledPlugin> {
    let locale = crate::platform::language::current_locale_identifier();
    let enabled = fs::read_to_string(enabled_path)
        .ok()
        .and_then(|text| serde_json::from_str::<BTreeSet<String>>(&text).ok())
        .unwrap_or_default();
    let mut plugins = crate::plugin_packages::discover(directory, locale.as_deref())
        .plugins
        .into_iter()
        .filter(|plugin| enabled.contains(&plugin.manifest.identifier))
        .map(|plugin| InstalledPlugin {
            manifest: Arc::new(plugin.manifest),
            directory: plugin.directory,
        })
        .collect::<Vec<_>>();
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
    if !paths.port.allows(&manifest.identifier) || sessions.contains_key(&player_key) {
        return Ok(());
    }
    if !crate::plugin_preferences::current_runtime_supports(manifest) {
        return Err(format!(
            "{} is enabled, but requests plugin capabilities that are unavailable",
            manifest.name
        ));
    }
    let grants = load_grants(&paths.permissions_path, &manifest.identifier)?;
    let session = PluginRuntimeSession::start_package_entry_with_player_context(
        directory,
        manifest,
        &grants,
        player_context.clone(),
        paths.player_mpv.clone(),
        paths.ui.clone(),
    )
    .map_err(|error| format!("Could not start {}: {error}", manifest.name))?;
    paths.ui.input.add(&session.input);
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

fn player_plugin_session<'a>(
    sessions: &'a BTreeMap<PluginInstanceKey, PluginRuntimeSession>,
    route: &PlayerPluginRoute,
) -> Option<&'a PluginRuntimeSession> {
    sessions
        .get(&PluginInstanceKey::player(&route.identifier))
        .filter(|session| session.realm == route.realm)
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
