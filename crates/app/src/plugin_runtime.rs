//! Bounded JavaScript entry evaluation for permission-reviewed plugins.

mod core_api;
mod events;
mod execution;
#[cfg(test)]
mod execution_tests;
mod file_api;
mod menu;
mod messaging;
mod modules;
mod network;
mod output;
mod permissions;
#[cfg(test)]
mod tests;
mod timers;
mod webviews;
mod worker;

use crate::plugin_files::{PluginFiles, PluginPlayerFileContext};
use events::validate_player_event_name;
use execution::ExecutionControl;
use messaging::validate_plugin_message;
use modules::{PluginModuleContext, read_package_script};
use network::NetworkWorkerContext;
use permissions::{AvailablePermissions, reviewed_permissions};
use riina_core::{
    Command,
    plugin::{Manifest, OverlayEffect, WindowAction},
    plugin_menu::PluginMenuItem,
    plugin_sidebar::PluginSidebarEffect,
};
use std::{
    collections::BTreeSet,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::AtomicUsize,
        mpsc::{self, SyncSender},
    },
    thread::JoinHandle,
    time::Duration,
};
use worker::run_entry_worker;

const MAX_ENTRY_BYTES: usize = 1024 * 1024;
const MAX_RUNTIME_MEMORY: usize = 32 * 1024 * 1024;
const MAX_STACK_BYTES: usize = 256 * 1024;
const MAX_HOST_EVENTS: usize = 128;
const MAX_MESSAGE_CHARS: usize = 2048;
const MAX_OVERLAY_INTENT_BYTES: usize = 1024 * 1024;
const MAX_PLUGIN_MESSAGE_BYTES: usize = 1024 * 1024;
const MAX_HOST_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
#[derive(Debug, Clone, PartialEq)]
pub enum HostEvent {
    Log(String),
    ShowOsd(String),
    ShowAlert(String),
    Overlay(OverlayEffect),
    Sidebar(PluginSidebarEffect),
    MenuUpdate(Vec<PluginMenuItem>),
    PluginMessage(String, serde_json::Value),
    CoreCommand(Command),
    SetUiVisibility(bool),
    WindowAction(WindowAction),
}

enum RuntimeCommand {
    DeliverOverlayMessage(String, serde_json::Value),
    DeliverSidebarMessage(String, serde_json::Value),
    DeliverPluginMenuAction(String),
    DeliverPluginMessage(String, serde_json::Value),
    DeliverPlayerEvent(String, serde_json::Value),
    Barrier(mpsc::SyncSender<()>),
    DeliverNetworkResult(u32, Result<serde_json::Value, String>),
    DeliverNetworkDownload(u32, Result<(), String>),
    DeliverXmlRpcResult(u32, Result<serde_json::Value, String>),
    Shutdown,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GlobalApiMode {
    None,
    Controller,
    Child,
}

struct PluginWorkerHost {
    execution: Arc<ExecutionControl>,
    global_api_mode: GlobalApiMode,
    module_context: Option<PluginModuleContext>,
    file_system: PluginFiles,
    events: Arc<Mutex<Vec<HostEvent>>>,
    player_event_subscriptions: Arc<Mutex<BTreeSet<String>>>,
    observed_mpv_properties: Arc<Mutex<BTreeSet<String>>>,
    mpv_property_observation_requests: Arc<Mutex<BTreeSet<String>>>,
}

struct PluginStorageContext {
    file_system: PluginFiles,
    allowed_domains: Vec<String>,
}

/// Owns a persistent QuickJS realm on its worker so plugin callbacks never run
/// on the UI or playback thread.
pub struct PluginRuntimeSession {
    execution: Arc<ExecutionControl>,
    commands: SyncSender<RuntimeCommand>,
    events: Arc<Mutex<Vec<HostEvent>>>,
    file_system: PluginFiles,
    player_event_subscriptions: Arc<Mutex<BTreeSet<String>>>,
    mpv_property_observation_requests: Arc<Mutex<BTreeSet<String>>>,
    worker: Option<JoinHandle<()>>,
}

impl PluginRuntimeSession {
    pub fn start_package_entry(
        package_root: &Path,
        manifest: &Manifest,
        granted_permissions: &[String],
    ) -> Result<Self, String> {
        Self::start_package_entry_with_player_context(
            package_root,
            manifest,
            granted_permissions,
            PluginPlayerFileContext::default(),
        )
    }

    pub(crate) fn start_package_entry_with_player_context(
        package_root: &Path,
        manifest: &Manifest,
        granted_permissions: &[String],
        player_context: PluginPlayerFileContext,
    ) -> Result<Self, String> {
        let (source, module_context) = read_package_script(package_root, &manifest.entry)?;
        let file_system = PluginFiles::for_package(package_root, &manifest.identifier);
        file_system.set_player_context(player_context);
        Self::start_entry_with_context(
            &source,
            &manifest.identifier,
            &manifest.permissions,
            granted_permissions,
            if manifest.global_entry.is_some() {
                GlobalApiMode::Child
            } else {
                GlobalApiMode::None
            },
            Some(module_context),
            PluginStorageContext {
                file_system,
                allowed_domains: manifest.allowed_domains.clone(),
            },
        )
    }

    pub fn start_package_global_entry(
        package_root: &Path,
        manifest: &Manifest,
        granted_permissions: &[String],
    ) -> Result<Self, String> {
        let Some(entry) = manifest.global_entry.as_deref() else {
            return Err("Plugin does not declare a global entry".into());
        };
        let (source, module_context) = read_package_script(package_root, entry)?;
        Self::start_entry_with_context(
            &source,
            &format!("{}.global", manifest.identifier),
            &manifest.permissions,
            granted_permissions,
            GlobalApiMode::Controller,
            Some(module_context),
            PluginStorageContext {
                file_system: PluginFiles::for_package(package_root, &manifest.identifier),
                allowed_domains: manifest.allowed_domains.clone(),
            },
        )
    }

    pub fn start_entry(
        source: &str,
        plugin_identifier: &str,
        requested_permissions: &[String],
        granted_permissions: &[String],
    ) -> Result<Self, String> {
        Self::start_entry_with_context(
            source,
            plugin_identifier,
            requested_permissions,
            granted_permissions,
            GlobalApiMode::None,
            None,
            PluginStorageContext {
                file_system: PluginFiles::temporary(plugin_identifier),
                allowed_domains: Vec::new(),
            },
        )
    }

    fn start_entry_with_context(
        source: &str,
        plugin_identifier: &str,
        requested_permissions: &[String],
        granted_permissions: &[String],
        global_api_mode: GlobalApiMode,
        module_context: Option<PluginModuleContext>,
        storage: PluginStorageContext,
    ) -> Result<Self, String> {
        if source.len() > MAX_ENTRY_BYTES {
            return Err("Plugin entry exceeds the 1 MiB execution limit".into());
        }
        let permissions = reviewed_permissions(requested_permissions, granted_permissions)?;
        let execution = Arc::new(ExecutionControl::new());
        let worker_execution = execution.clone();
        let (commands, receiver) = mpsc::sync_channel(32);
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let events = Arc::new(Mutex::new(Vec::new()));
        let session_file_system = storage.file_system.clone();
        let worker_events = events.clone();
        let player_event_subscriptions = Arc::new(Mutex::new(BTreeSet::new()));
        let worker_player_event_subscriptions = player_event_subscriptions.clone();
        let observed_mpv_properties = Arc::new(Mutex::new(BTreeSet::new()));
        let worker_observed_mpv_properties = observed_mpv_properties.clone();
        let mpv_property_observation_requests = Arc::new(Mutex::new(BTreeSet::new()));
        let worker_mpv_property_observation_requests = mpv_property_observation_requests.clone();
        let network_active = Arc::new(AtomicUsize::new(0));
        let network_commands = commands.clone();
        let network_context = NetworkWorkerContext {
            commands: network_commands,
            allowed_domains: storage.allowed_domains,
            active: network_active,
        };
        let source = source.to_owned();
        let plugin_identifier = plugin_identifier.to_owned();
        let worker = std::thread::Builder::new()
            .name(format!(
                "plugin-{}",
                plugin_identifier.chars().take(32).collect::<String>()
            ))
            .spawn(move || {
                run_entry_worker(
                    source,
                    plugin_identifier,
                    permissions,
                    PluginWorkerHost {
                        execution: worker_execution,
                        global_api_mode,
                        module_context,
                        file_system: storage.file_system,
                        events: worker_events,
                        player_event_subscriptions: worker_player_event_subscriptions,
                        observed_mpv_properties: worker_observed_mpv_properties,
                        mpv_property_observation_requests: worker_mpv_property_observation_requests,
                    },
                    receiver,
                    ready_sender,
                    network_context,
                );
            })
            .map_err(|error| format!("Could not start plugin worker: {error}"))?;
        match ready_receiver.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(())) => Ok(Self {
                execution,
                commands,
                events,
                file_system: session_file_system,
                player_event_subscriptions,
                mpv_property_observation_requests,
                worker: Some(worker),
            }),
            Ok(Err(error)) => {
                let _ = worker.join();
                Err(error)
            }
            Err(error) => {
                execution.cancel();
                let _ = commands.try_send(RuntimeCommand::Shutdown);
                let _ = worker.join();
                Err(format!("Plugin worker did not initialize: {error}"))
            }
        }
    }

    pub(crate) fn update_player_file_context(&self, context: PluginPlayerFileContext) {
        self.file_system.set_player_context(context);
    }

    pub fn deliver_overlay_message(
        &self,
        name: String,
        data: serde_json::Value,
    ) -> Result<(), String> {
        // The session's worker owns the authoritative state. Host callers send
        // bounded data here; the worker validates subscription and lifecycle.
        if name.is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
            return Err("Overlay message name is invalid or exceeds 256 bytes".into());
        }
        let data_bytes = serde_json::to_vec(&data)
            .map_err(|error| format!("Overlay message is not valid JSON: {error}"))?;
        if data_bytes.len() > MAX_OVERLAY_INTENT_BYTES {
            return Err("Overlay message exceeds the 1 MiB limit".into());
        }
        self.commands
            .try_send(RuntimeCommand::DeliverOverlayMessage(name, data))
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => "Plugin message queue is busy".into(),
                mpsc::TrySendError::Disconnected(_) => "Plugin session has stopped".into(),
            })
    }

    pub fn deliver_sidebar_message(
        &self,
        name: String,
        data: serde_json::Value,
    ) -> Result<(), String> {
        validate_plugin_message(&name, &data)?;
        self.commands
            .try_send(RuntimeCommand::DeliverSidebarMessage(name, data))
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => "Plugin message queue is busy".into(),
                mpsc::TrySendError::Disconnected(_) => "Plugin session has stopped".into(),
            })
    }

    pub(crate) fn deliver_plugin_menu_action(&self, item_id: String) -> Result<(), String> {
        if item_id.is_empty() || item_id.len() > 64 {
            return Err("Plugin menu action identifier is invalid".into());
        }
        self.commands
            .try_send(RuntimeCommand::DeliverPluginMenuAction(item_id))
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => "Plugin event queue is busy".into(),
                mpsc::TrySendError::Disconnected(_) => "Plugin session has stopped".into(),
            })
    }

    pub fn deliver_plugin_message(
        &self,
        name: String,
        data: serde_json::Value,
    ) -> Result<(), String> {
        validate_plugin_message(&name, &data)?;
        self.commands
            .try_send(RuntimeCommand::DeliverPluginMessage(name, data))
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => "Plugin message queue is busy".into(),
                mpsc::TrySendError::Disconnected(_) => "Plugin session has stopped".into(),
            })
    }

    pub(crate) fn is_subscribed_to_player_event(&self, name: &str) -> bool {
        self.player_event_subscriptions
            .lock()
            .is_ok_and(|subscriptions| subscriptions.contains(name))
    }

    pub(crate) fn take_mpv_property_observation_requests(&self) -> Vec<String> {
        self.mpv_property_observation_requests
            .lock()
            .map(|mut requests| std::mem::take(&mut *requests).into_iter().collect())
            .unwrap_or_default()
    }

    pub(crate) fn deliver_player_event(
        &self,
        name: String,
        data: serde_json::Value,
    ) -> Result<(), String> {
        validate_player_event_name(&name)?;
        if !self.is_subscribed_to_player_event(&name) {
            return Ok(());
        }
        let bytes = serde_json::to_vec(&data)
            .map_err(|error| format!("Plugin event data is not valid JSON: {error}"))?;
        if bytes.len() > MAX_PLUGIN_MESSAGE_BYTES {
            return Err("Plugin event data exceeds the 1 MiB limit".into());
        }
        self.commands
            .try_send(RuntimeCommand::DeliverPlayerEvent(name, data))
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => "Plugin event queue is busy".into(),
                mpsc::TrySendError::Disconnected(_) => "Plugin session has stopped".into(),
            })
    }

    pub(crate) fn flush_events(&self, timeout: Duration) -> Result<(), String> {
        let (response, result) = mpsc::sync_channel(1);
        self.commands
            .try_send(RuntimeCommand::Barrier(response))
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => "Plugin event queue is busy".to_owned(),
                mpsc::TrySendError::Disconnected(_) => "Plugin session has stopped".to_owned(),
            })?;
        result
            .recv_timeout(timeout)
            .map_err(|error| format!("Plugin event callbacks did not finish: {error}"))
    }

    pub fn drain_events(&self) -> Result<Vec<HostEvent>, String> {
        let mut events = self
            .events
            .lock()
            .map_err(|_| "Plugin event queue was poisoned".to_owned())?;
        Ok(std::mem::take(&mut *events))
    }
}

impl Drop for PluginRuntimeSession {
    fn drop(&mut self) {
        self.execution.cancel();
        // Wake an idle receiver. A full queue already guarantees a wakeup; the
        // cancellation flag prevents queued callbacks from delaying teardown.
        let _ = self.commands.try_send(RuntimeCommand::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Resolve and execute the already-validated entry again at activation time.
/// Rechecking containment prevents a replaced package file from escaping its root.
pub fn execute_package_entry(
    package_root: &Path,
    manifest: &Manifest,
    granted_permissions: &[String],
) -> Result<Vec<HostEvent>, String> {
    let session =
        PluginRuntimeSession::start_package_entry(package_root, manifest, granted_permissions)?;
    session.drain_events()
}

/// Evaluate an entry with only its granted host calls and collect startup events.
/// Keep a `PluginRuntimeSession` when the plugin needs to receive later messages.
pub fn execute_entry(
    source: &str,
    plugin_identifier: &str,
    requested_permissions: &[String],
    granted_permissions: &[String],
) -> Result<Vec<HostEvent>, String> {
    let session = PluginRuntimeSession::start_entry(
        source,
        plugin_identifier,
        requested_permissions,
        granted_permissions,
    )?;
    session.drain_events()
}
