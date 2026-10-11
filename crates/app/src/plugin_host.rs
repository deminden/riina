//! Application-lifetime owner for enabled plugin entry sessions.

use crate::plugin_runtime::{HostEvent, PluginRuntimeSession};
use riina_core::plugin::Manifest;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    thread::JoinHandle,
};

const MAX_PENDING_MESSAGES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PluginHostMessage {
    Log(String),
    ShowOsd(String),
    ShowAlert(String, String),
    Status(String),
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

impl PluginHost {
    pub(crate) fn start(preferences_path: &Path) -> Result<Self, String> {
        let plugins_directory = preferences_path.with_file_name("Plugins");
        let permissions_path = preferences_path.with_file_name("plugin-permissions.json");
        let enabled_path = preferences_path.with_file_name("plugin-enabled.json");
        let (commands, receiver) = mpsc::sync_channel(64);
        let messages = Arc::new(Mutex::new(Vec::new()));
        let worker_messages = messages.clone();
        let worker = std::thread::Builder::new()
            .name("riina-plugin-host".into())
            .spawn(move || {
                run_host(
                    plugins_directory,
                    permissions_path,
                    enabled_path,
                    worker_messages,
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
    commands: mpsc::Receiver<PluginCommand>,
) {
    let mut sessions = BTreeMap::<PluginInstanceKey, PluginRuntimeSession>::new();
    let mut deferred = Vec::new();
    for plugin in enabled_packages(&plugins_directory, &enabled_path) {
        loop {
            match commands.try_recv() {
                Ok(PluginCommand::Shutdown) => return,
                Ok(command) => deferred.push(command),
                Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected) => break,
            }
        }
        if let Err(error) = activate(
            &plugin.manifest,
            &plugin.directory,
            &permissions_path,
            &messages,
            &mut sessions,
        ) {
            record(&messages, PluginHostMessage::Status(error));
        }
    }
    for command in deferred {
        if handle_command(command, &permissions_path, &messages, &mut sessions) {
            return;
        }
    }

    while let Ok(command) = commands.recv() {
        if handle_command(command, &permissions_path, &messages, &mut sessions) {
            break;
        }
    }
}

fn handle_command(
    command: PluginCommand,
    permissions_path: &Path,
    messages: &Arc<Mutex<Vec<PluginHostMessage>>>,
    sessions: &mut BTreeMap<PluginInstanceKey, PluginRuntimeSession>,
) -> bool {
    match command {
        PluginCommand::Enable {
            manifest,
            directory,
        } => {
            if let Err(error) =
                activate(&manifest, &directory, permissions_path, messages, sessions)
            {
                record(messages, PluginHostMessage::Status(error));
            }
        }
        PluginCommand::Disable(identifier) => {
            sessions.retain(|key, _| key.identifier != identifier);
        }
        PluginCommand::Shutdown => return true,
    }
    false
}

struct InstalledPlugin {
    manifest: Manifest,
    directory: PathBuf,
}

fn enabled_packages(directory: &Path, enabled_path: &Path) -> Vec<InstalledPlugin> {
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
                Manifest::load_from_directory(&path)
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
    plugins.sort_by(|left, right| left.manifest.identifier.cmp(&right.manifest.identifier));
    plugins
}

fn activate(
    manifest: &Manifest,
    directory: &Path,
    permissions_path: &Path,
    messages: &Arc<Mutex<Vec<PluginHostMessage>>>,
    sessions: &mut BTreeMap<PluginInstanceKey, PluginRuntimeSession>,
) -> Result<(), String> {
    let player_key = PluginInstanceKey::player(&manifest.identifier);
    if sessions.contains_key(&player_key) {
        return Ok(());
    }
    if !manifest
        .permissions
        .iter()
        .all(|permission| matches!(permission.as_str(), "show-osd" | "show-alert"))
        || manifest.unknown_permissions().next().is_some()
    {
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
    let session = PluginRuntimeSession::start_package_entry(directory, manifest, &grants)
        .map_err(|error| format!("Could not start {}: {error}", manifest.name))?;
    if let Some(global) = global.as_ref() {
        for event in global.drain_events().map_err(|error| {
            format!(
                "Could not read global {} startup events: {error}",
                manifest.name
            )
        })? {
            record(messages, host_message(&manifest.name, event));
        }
    }
    for event in session
        .drain_events()
        .map_err(|error| format!("Could not read {} startup events: {error}", manifest.name))?
    {
        record(messages, host_message(&manifest.name, event));
    }
    if let Some(global) = global {
        sessions.insert(PluginInstanceKey::global(&manifest.identifier), global);
    }
    sessions.insert(player_key, session);
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
        HostEvent::Overlay(_) => PluginHostMessage::Status(format!(
            "{plugin_name} requested an overlay; overlay rendering is unavailable."
        )),
    }
}

fn record(messages: &Mutex<Vec<PluginHostMessage>>, message: PluginHostMessage) {
    if let Ok(mut messages) = messages.lock()
        && messages.len() < MAX_PENDING_MESSAGES
    {
        messages.push(message);
    }
}
