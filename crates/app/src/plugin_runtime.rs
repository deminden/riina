//! Bounded JavaScript entry evaluation for permission-reviewed plugins.

use crate::plugin_files::{PluginFileHandle, PluginFiles, PluginPlayerFileContext};
use riina_core::plugin::{
    Manifest, OverlayEffect, OverlayIntent, OverlayState, WindowAction, network_url_allowed,
};
use riina_core::{Command, control_preferences::SeekType};
use rquickjs::{Context, Ctx, Function, Object, Runtime, Value, function::Func};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

const MAX_ENTRY_BYTES: usize = 1024 * 1024;
const MAX_RUNTIME_MEMORY: usize = 32 * 1024 * 1024;
const MAX_EXECUTION_TIME: Duration = Duration::from_millis(500);
const MAX_STACK_BYTES: usize = 256 * 1024;
const MAX_HOST_EVENTS: usize = 128;
const MAX_MESSAGE_CHARS: usize = 2048;
const MAX_OVERLAY_INTENT_BYTES: usize = 1024 * 1024;
const MAX_PLUGIN_MESSAGE_BYTES: usize = 1024 * 1024;
const MAX_HOST_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
const MAX_ACTIVE_TIMERS: usize = 256;
const MAX_TIMER_DELAY: Duration = Duration::from_secs(24 * 60 * 60);
const MAX_REQUIRED_MODULES: usize = 256;
const MAX_REQUIRED_MODULE_BYTES: usize = 16 * 1024 * 1024;
const MAX_REQUIRE_DEPTH: usize = 64;
const MAX_ACTIVE_NETWORK_REQUESTS: usize = 8;
const MAX_NETWORK_REQUEST_BYTES: usize = 1024 * 1024;
const MAX_PLUGIN_DOWNLOAD_BYTES: usize = 64 * 1024 * 1024;
const MAX_OPEN_FILE_HANDLES: usize = 64;
const NETWORK_TIMEOUT: Duration = Duration::from_secs(30);
const PLUGIN_EVENT_API: &str = r#"(() => {
    const listeners = new Map();
    let nextId = 0;
    let listenerCount = 0;
    const validName = (name) => {
        if (typeof name !== "string") return false;
        const parts = name.split(".");
        return (parts.length === 2 || (parts.length === 3 && parts[2] === "changed"))
            && (parts[0] === "iina" || parts[0] === "mpv")
            && parts[1].length > 0
            && parts.every((part) => part.length > 0);
    };
    iina.event = {
        on(name, callback) {
            if (!validName(name)) throw new Error(`Incorrect event name syntax: ${String(name)}`);
            if (typeof callback !== "function") throw new TypeError("Event listener must be a function");
            if (listenerCount >= 1024) throw new Error("Plugin has reached the 1024 event listener limit");
            let entries = listeners.get(name);
            if (!entries) {
                iina.__subscribePlayerEvent(name);
                entries = new Map();
                listeners.set(name, entries);
            }
            const id = String(++nextId);
            entries.set(id, callback);
            listenerCount += 1;
            return id;
        },
        off(name, id) {
            const entries = listeners.get(name);
            if (!entries || !entries.delete(id)) {
                iina.log(`Event listener not found for id ${String(id)}`);
                return;
            }
            listenerCount -= 1;
            if (entries.size === 0) {
                listeners.delete(name);
                iina.__unsubscribePlayerEvent(name);
            }
        },
        __dispatch(name, data) {
            for (const callback of [...(listeners.get(name)?.values() || [])]) {
                try { callback(data); }
                catch (error) { iina.log(`Event callback failed for ${name}: ${String(error)}`); }
            }
        }
    };
})();"#;

#[derive(Debug, Clone, PartialEq)]
pub enum HostEvent {
    Log(String),
    ShowOsd(String),
    ShowAlert(String),
    Overlay(OverlayEffect),
    PluginMessage(String, serde_json::Value),
    CoreCommand(Command),
    SetUiVisibility(bool),
    WindowAction(WindowAction),
}

enum RuntimeCommand {
    DeliverOverlayMessage(String, serde_json::Value),
    DeliverPluginMessage(String, serde_json::Value),
    DeliverPlayerEvent(String, serde_json::Value),
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

#[derive(Default)]
struct PluginTimers {
    next_id: AtomicU32,
    deadlines: Mutex<BTreeMap<u32, Instant>>,
}

#[derive(Clone)]
struct PluginModuleContext {
    root: PathBuf,
    entry: PathBuf,
    current_files: Arc<Mutex<Vec<PathBuf>>>,
    budget: Arc<Mutex<ModuleLoadBudget>>,
}

#[derive(Default)]
struct ModuleLoadBudget {
    modules: usize,
    bytes: usize,
}

struct PluginWorkerHost {
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

struct NetworkWorkerContext {
    commands: SyncSender<RuntimeCommand>,
    allowed_domains: Vec<String>,
    active: Arc<AtomicUsize>,
}

impl PluginTimers {
    fn schedule(&self, delay_ms: f64) -> Result<u32, String> {
        if !delay_ms.is_finite() || !(0.0..=MAX_TIMER_DELAY.as_millis() as f64).contains(&delay_ms)
        {
            return Err("Plugin timer delay must be between 0 and 86400000 milliseconds".into());
        }
        let delay = Duration::from_secs_f64(delay_ms / 1000.0);
        let mut deadlines = self
            .deadlines
            .lock()
            .map_err(|_| "Plugin timer queue was poisoned".to_owned())?;
        if deadlines.len() >= MAX_ACTIVE_TIMERS {
            return Err("Plugin has reached the 256 active timer limit".into());
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        deadlines.insert(id, Instant::now() + delay);
        Ok(id)
    }

    fn cancel(&self, id: u32) {
        if let Ok(mut deadlines) = self.deadlines.lock() {
            deadlines.remove(&id);
        }
    }

    fn next_wait(&self) -> Option<Duration> {
        self.deadlines
            .lock()
            .ok()?
            .values()
            .copied()
            .min()
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
    }

    fn take_due(&self) -> Option<u32> {
        let mut deadlines = self.deadlines.lock().ok()?;
        let now = Instant::now();
        let id = deadlines
            .iter()
            .filter(|(_, deadline)| **deadline <= now)
            .min_by_key(|(_, deadline)| **deadline)
            .map(|(id, _)| *id)?;
        deadlines.remove(&id);
        Some(id)
    }
}

/// Owns a persistent QuickJS realm on its worker so plugin callbacks never run
/// on the UI or playback thread.
pub struct PluginRuntimeSession {
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
                let _ = commands.send(RuntimeCommand::Shutdown);
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
        let _ = self.commands.send(RuntimeCommand::Shutdown);
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

fn read_package_script(
    package_root: &Path,
    entry_path: &Path,
) -> Result<(String, PluginModuleContext), String> {
    let root = package_root
        .canonicalize()
        .map_err(|error| format!("Could not open plugin package: {error}"))?;
    if !root.is_dir() {
        return Err("Plugin package is not a directory".into());
    }
    let entry = resolve_package_file(&root, entry_path).map_err(|error| {
        format!(
            "Could not open plugin entry {}: {error}",
            entry_path.display()
        )
    })?;
    let source = read_plugin_script(&entry)?;
    Ok((
        source,
        PluginModuleContext {
            root,
            entry,
            current_files: Arc::new(Mutex::new(Vec::new())),
            budget: Arc::new(Mutex::new(ModuleLoadBudget::default())),
        },
    ))
}

/// Revalidate containment after installation and before each module load.
fn resolve_package_file(root: &Path, entry_path: &Path) -> Result<PathBuf, String> {
    let entry = root
        .join(entry_path)
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if !entry.starts_with(root) || !entry.is_file() {
        return Err(format!(
            "Plugin entry {} must be a file inside its package",
            entry_path.display()
        ));
    }
    Ok(entry)
}

fn read_plugin_script(entry: &Path) -> Result<String, String> {
    let length = fs::metadata(entry)
        .map_err(|error| format!("Could not inspect plugin entry: {error}"))?
        .len();
    if length > MAX_ENTRY_BYTES as u64 {
        return Err("Plugin entry exceeds the 1 MiB execution limit".into());
    }
    let bytes = read_bounded_plugin_file(entry, "Plugin entry")?;
    let source =
        std::str::from_utf8(&bytes).map_err(|_| "Plugin entry is not valid UTF-8".to_owned())?;
    Ok(source.to_owned())
}

fn read_bounded_plugin_file(path: &Path, label: &str) -> Result<Vec<u8>, String> {
    let file = fs::File::open(path).map_err(|error| format!("Could not read {label}: {error}"))?;
    let mut bytes = Vec::new();
    file.take((MAX_ENTRY_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Could not read {label}: {error}"))?;
    if bytes.len() > MAX_ENTRY_BYTES {
        return Err(format!("{label} exceeds the 1 MiB file limit"));
    }
    Ok(bytes)
}

fn load_plugin_module<'js>(
    ctx: Ctx<'js>,
    package: &PluginModuleContext,
    requested_path: &str,
) -> Result<Option<(String, Value<'js>)>, String> {
    if requested_path.is_empty() || requested_path.len() > 4096 {
        return Ok(None);
    }
    let (base_file, depth) = {
        let current_files = package
            .current_files
            .lock()
            .map_err(|_| "Plugin module path stack was poisoned".to_owned())?;
        let Some(base_file) = current_files.last().cloned() else {
            return Ok(None);
        };
        (base_file, current_files.len())
    };
    if depth >= MAX_REQUIRE_DEPTH {
        return Err("Plugin module nesting exceeds the 64-file limit".into());
    }
    let Some(parent) = base_file.parent() else {
        return Ok(None);
    };
    let requested = Path::new(requested_path);
    let Ok(module_path) = parent.join(requested).canonicalize() else {
        return Ok(None);
    };
    if !module_path.starts_with(&package.root) || !module_path.is_file() {
        return Ok(None);
    }
    let metadata = fs::metadata(&module_path).map_err(|error| error.to_string())?;
    if metadata.len() > MAX_ENTRY_BYTES as u64 {
        return Err("Plugin module exceeds the 1 MiB file limit".into());
    }
    let bytes = read_bounded_plugin_file(&module_path, "Plugin module")?;
    let source =
        std::str::from_utf8(&bytes).map_err(|_| "Plugin module is not valid UTF-8".to_owned())?;
    {
        let mut budget = package
            .budget
            .lock()
            .map_err(|_| "Plugin module load budget was poisoned".to_owned())?;
        if budget.modules >= MAX_REQUIRED_MODULES
            || budget.bytes.saturating_add(bytes.len()) > MAX_REQUIRED_MODULE_BYTES
        {
            return Err("Plugin module load budget exceeded".into());
        }
        budget.modules += 1;
        budget.bytes += bytes.len();
    }
    let canonical_path = fs::canonicalize(&module_path).map_err(|error| error.to_string())?;
    if !canonical_path.starts_with(&package.root) {
        return Ok(None);
    }

    let mut current_files = package
        .current_files
        .lock()
        .map_err(|_| "Plugin module path stack was poisoned".to_owned())?;
    current_files.push(canonical_path.clone());
    drop(current_files);

    let wrapped_source = format!("(function(module) {{\n{source}\nreturn module.exports;\n}})");
    let evaluated = (|| -> rquickjs::Result<Value<'js>> {
        let function: Function<'js> = ctx.eval(wrapped_source.as_str())?;
        let module = Object::new(ctx.clone())?;
        function.call((module,))
    })();

    if let Ok(mut current_files) = package.current_files.lock() {
        current_files.pop();
    }
    let exports = evaluated.map_err(|error| error.to_string())?;
    Ok(Some((
        canonical_path.to_string_lossy().into_owned(),
        exports,
    )))
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

fn run_entry_worker(
    source: String,
    plugin_identifier: String,
    permissions: AvailablePermissions,
    host: PluginWorkerHost,
    commands: Receiver<RuntimeCommand>,
    ready: SyncSender<Result<(), String>>,
    network: NetworkWorkerContext,
) {
    let NetworkWorkerContext {
        commands: network_commands,
        allowed_domains,
        active: network_active,
    } = network;
    let PluginWorkerHost {
        global_api_mode,
        module_context,
        file_system,
        events,
        player_event_subscriptions,
        observed_mpv_properties,
        mpv_property_observation_requests,
    } = host;
    let deadline = Arc::new(Mutex::new(Instant::now() + MAX_EXECUTION_TIME));
    let interrupt_deadline = deadline.clone();
    let runtime = match Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = ready.send(Err(format!("Could not create JavaScript runtime: {error}")));
            return;
        }
    };
    runtime.set_memory_limit(MAX_RUNTIME_MEMORY);
    runtime.set_max_stack_size(MAX_STACK_BYTES);
    // Each evaluation gets its own bounded deadline; plugin sessions may remain
    // idle indefinitely without their interrupt handler expiring in the future.
    runtime.set_interrupt_handler(Some(Box::new(move || {
        interrupt_deadline
            .lock()
            .map_or(true, |deadline| Instant::now() >= *deadline)
    })));
    if let Err(error) = runtime.set_info(format!("riina plugin {plugin_identifier}")) {
        let _ = ready.send(Err(format!("Could not label JavaScript runtime: {error}")));
        return;
    }

    let context = match Context::full(&runtime) {
        Ok(context) => context,
        Err(error) => {
            let _ = ready.send(Err(format!("Could not create plugin context: {error}")));
            return;
        }
    };
    let overlay_state = Arc::new(Mutex::new(OverlayState::default()));
    let worker_overlay_state = overlay_state.clone();
    let timers = Arc::new(PluginTimers::default());
    let worker_timers = timers.clone();
    let network_request_allowed = permissions.network_request;
    let network_sender_for_api = network_commands.clone();
    let network_active_for_api = network_active.clone();
    let allowed_domains_for_api = allowed_domains.clone();
    let module_context_for_require = module_context.clone();
    let initialized = context.with(|ctx| {
            let globals = ctx.globals();
            let api = Object::new(ctx.clone())?;

            let log_events = events.clone();
            api.set(
                "log",
                Func::from(move |message: String| {
                    record(&log_events, HostEvent::Log(message));
                }),
            )?;
            if permissions.show_osd {
                let osd_events = events.clone();
                api.set(
                    "showOSD",
                    Func::from(move |message: String| {
                        record(&osd_events, HostEvent::ShowOsd(message));
                    }),
                )?;
            }
            if permissions.show_alert {
                let alert_events = events.clone();
                api.set(
                    "showAlert",
                    Func::from(move |message: String| {
                        record(&alert_events, HostEvent::ShowAlert(message));
                    }),
                )?;
            }
            globals.set("iina", api.clone())?;
            if global_api_mode != GlobalApiMode::Controller {
                let subscriptions = player_event_subscriptions.clone();
                let observed_properties = observed_mpv_properties.clone();
                let observation_requests = mpv_property_observation_requests.clone();
                api.set(
                    "__subscribePlayerEvent",
                    Func::from(move |name: String| -> rquickjs::Result<()> {
                        validate_player_event_name(&name).map_err(|message| {
                            rquickjs::Error::new_from_js_message(
                                "event name",
                                "iina.* or mpv.* event",
                                message,
                            )
                        })?;
                        let mut subscriptions = subscriptions.lock().map_err(|_| {
                            rquickjs::Error::new_from_js_message(
                                "event listener",
                                "available subscription registry",
                                "Plugin event subscriptions were poisoned",
                            )
                        })?;
                        if let Some(property) = mpv_property_for_changed_event(&name)
                            && !player_snapshot_mpv_property(property)
                        {
                            let mut observed = observed_properties.lock().map_err(|_| {
                                rquickjs::Error::new_from_js_message(
                                    "mpv property",
                                    "available observation registry",
                                    "Plugin property observations were poisoned",
                                )
                            })?;
                            if !observed.contains(property)
                                && observed.len()
                                    >= riina_core::MAX_PLUGIN_OBSERVED_MPV_PROPERTIES
                            {
                                return Err(rquickjs::Error::new_from_js_message(
                                    "mpv property",
                                    "at most 128 observed properties per plugin",
                                    "Plugin has reached the 128 observed mpv property limit",
                                ));
                            }
                            if observed.insert(property.to_owned()) {
                                observation_requests
                                    .lock()
                                    .map_err(|_| {
                                        rquickjs::Error::new_from_js_message(
                                            "mpv property",
                                            "available observation request queue",
                                            "Plugin observation request queue was poisoned",
                                        )
                                    })?
                                    .insert(property.to_owned());
                            }
                        }
                        subscriptions.insert(name);
                        Ok(())
                    }),
                )?;
                let subscriptions = player_event_subscriptions.clone();
                api.set(
                    "__unsubscribePlayerEvent",
                    Func::from(move |name: String| {
                        if let Ok(mut subscriptions) = subscriptions.lock() {
                            subscriptions.remove(&name);
                        }
                    }),
                )?;
                ctx.eval::<Value, _>(PLUGIN_EVENT_API)?;
            }
            install_plugin_file_api(
                ctx.clone(),
                globals.clone(),
                file_system.clone(),
                permissions.file_system,
            )?;
            install_plugin_core_api(
                ctx.clone(),
                api.clone(),
                events.clone(),
                file_system.clone(),
                permissions,
                allowed_domains.clone(),
            )?;
            if network_request_allowed {
                let http = Object::new(ctx.clone())?;
                let network_sender = network_sender_for_api.clone();
                let active_requests = network_active_for_api.clone();
                let domains = allowed_domains_for_api.clone();
                let download_files = file_system.clone();
                let download_filesystem_granted = permissions.file_system;
                let xmlrpc_check_domains = allowed_domains_for_api.clone();
                http.set(
                    "__xmlrpcAllowed",
                    Func::from(move |location: String| {
                        network_url_allowed(&location, &xmlrpc_check_domains)
                    }),
                )?;
                http.set(
                    "__request",
                    Func::from(
                        move |id: u32, method: String, url: String, options: String| -> rquickjs::Result<()> {
                            let options: serde_json::Value = serde_json::from_str(&options)
                                .map_err(|error| http_js_error(error.to_string()))?;
                            if !network_url_allowed(&url, &domains) {
                                return Err(http_js_error(
                                    "Plugin is not allowed to access this URL".into(),
                                ));
                            }
                            if options.to_string().len() > MAX_NETWORK_REQUEST_BYTES {
                                return Err(http_js_error(
                                    "HTTP request options exceed the 1 MiB limit".into(),
                                ));
                            }
                            if !acquire_network_request_slot(&active_requests) {
                                return Err(http_js_error(
                                    "Plugin has reached the 8 active HTTP request limit".into(),
                                ));
                            }
                            let sender = network_sender.clone();
                            let active_for_thread = active_requests.clone();
                            let domains = domains.clone();
                            let worker = std::thread::Builder::new()
                                .name("plugin-http".into())
                                .spawn(move || {
                                    let result = perform_http_request(
                                        &method,
                                        &url,
                                        &options,
                                        &domains,
                                    );
                                    active_for_thread.fetch_sub(1, Ordering::AcqRel);
                                    let _ = sender.send(RuntimeCommand::DeliverNetworkResult(id, result));
                                });
                            if let Err(error) = worker {
                                active_requests.fetch_sub(1, Ordering::AcqRel);
                                return Err(http_js_error(format!(
                                    "Could not start HTTP request: {error}"
                                )));
                            }
                            Ok(())
                        },
                    ),
                )?;
                let download_sender = network_sender_for_api.clone();
                let download_active = network_active_for_api.clone();
                let download_domains = allowed_domains_for_api.clone();
                let download_files = download_files.clone();
                http.set(
                    "__download",
                    Func::from(
                        move |id: u32, url: String, destination: String, options: String| -> rquickjs::Result<()> {
                            let options: serde_json::Value = serde_json::from_str(&options)
                                .map_err(|error| http_js_error(error.to_string()))?;
                            if !network_url_allowed(&url, &download_domains) {
                                return Err(http_js_error(
                                    "Plugin is not allowed to access this URL".into(),
                                ));
                            }
                            if options.to_string().len() > MAX_NETWORK_REQUEST_BYTES {
                                return Err(http_js_error(
                                    "HTTP request options exceed the 1 MiB limit".into(),
                                ));
                            }
                            if destination.len() > 16 * 1024 || destination.contains('\0') {
                                return Err(http_js_error("Download destination is invalid".into()));
                            }
                            if !acquire_network_request_slot(&download_active) {
                                return Err(http_js_error(
                                    "Plugin has reached the 8 active HTTP request limit".into(),
                                ));
                            }
                            let sender = download_sender.clone();
                            let active_for_thread = download_active.clone();
                            let domains = download_domains.clone();
                            let files = download_files.clone();
                            let filesystem_granted = download_filesystem_granted;
                            let worker = std::thread::Builder::new()
                                .name("plugin-http-download".into())
                                .spawn(move || {
                                    let result = perform_http_download(&url, &options, &domains)
                                        .and_then(|content| {
                                            files.write_download(
                                                &destination,
                                                &content,
                                                filesystem_granted,
                                            )
                                        });
                                    active_for_thread.fetch_sub(1, Ordering::AcqRel);
                                    let _ = sender.send(RuntimeCommand::DeliverNetworkDownload(id, result));
                                });
                            if let Err(error) = worker {
                                download_active.fetch_sub(1, Ordering::AcqRel);
                                return Err(http_js_error(format!(
                                    "Could not start HTTP download: {error}"
                                )));
                            }
                            Ok(())
                        },
                    ),
                )?;
                let xmlrpc_sender = network_sender_for_api.clone();
                let xmlrpc_active = network_active_for_api.clone();
                let xmlrpc_domains = allowed_domains_for_api.clone();
                http.set(
                    "__xmlrpc",
                    Func::from(
                        move |id: u32, location: String, method: String, args: String| -> rquickjs::Result<()> {
                            if !network_url_allowed(&location, &xmlrpc_domains) {
                                return Err(http_js_error(
                                    "Plugin is not allowed to access this URL".into(),
                                ));
                            }
                            if method.is_empty()
                                || method.len() > 1024
                                || method.chars().any(char::is_control)
                            {
                                return Err(http_js_error("XML-RPC method name is invalid".into()));
                            }
                            let args: serde_json::Value = serde_json::from_str(&args)
                                .map_err(|error| http_js_error(error.to_string()))?;
                            if !args.is_array() || args.to_string().len() > MAX_NETWORK_REQUEST_BYTES {
                                return Err(http_js_error(
                                    "XML-RPC arguments must be an array no larger than 1 MiB".into(),
                                ));
                            }
                            if !acquire_network_request_slot(&xmlrpc_active) {
                                return Err(http_js_error(
                                    "Plugin has reached the 8 active HTTP request limit".into(),
                                ));
                            }
                            let sender = xmlrpc_sender.clone();
                            let active_for_thread = xmlrpc_active.clone();
                            let domains = xmlrpc_domains.clone();
                            let worker = std::thread::Builder::new()
                                .name("plugin-xmlrpc".into())
                                .spawn(move || {
                                    let result = perform_xmlrpc_call(
                                        &location,
                                        &method,
                                        &args,
                                        &domains,
                                    );
                                    active_for_thread.fetch_sub(1, Ordering::AcqRel);
                                    let _ = sender.send(RuntimeCommand::DeliverXmlRpcResult(id, result));
                                });
                            if let Err(error) = worker {
                                xmlrpc_active.fetch_sub(1, Ordering::AcqRel);
                                return Err(http_js_error(format!(
                                    "Could not start XML-RPC call: {error}"
                                )));
                            }
                            Ok(())
                        },
                    ),
                )?;
                globals.set("__riinaHttp", http)?;
                ctx.eval::<(), _>(
                    r#"(() => {
                        const nativeRequest = __riinaHttp.__request;
                        const nativeDownload = __riinaHttp.__download;
                        const nativeXmlrpc = __riinaHttp.__xmlrpc;
                        const xmlrpcAllowed = __riinaHttp.__xmlrpcAllowed;
                        const pending = new Map();
                        let nextId = 0;
                        const request = (method, url, options = {}) => new Promise((resolve, reject) => {
                            nextId = (nextId + 1) >>> 0;
                            const id = nextId;
                            pending.set(id, { resolve, reject });
                            try {
                                nativeRequest(id, method, String(url), JSON.stringify(options));
                            } catch (error) {
                                pending.delete(id);
                                reject(error);
                            }
                        });
                        iina.http = {
                            get: (url, options) => request('GET', url, options),
                            post: (url, options) => request('POST', url, options),
                            put: (url, options) => request('PUT', url, options),
                            patch: (url, options) => request('PATCH', url, options),
                            delete: (url, options) => request('DELETE', url, options),
                            download: (url, destination, options = {}) => new Promise((resolve, reject) => {
                                nextId = (nextId + 1) >>> 0;
                                const id = nextId;
                                pending.set(id, { resolve, reject });
                                try {
                                    nativeDownload(id, String(url), String(destination), JSON.stringify(options));
                                } catch (error) {
                                    pending.delete(id);
                                    reject(error);
                                }
                            }),
                            xmlrpc: location => {
                                const endpoint = String(location);
                                if (!xmlrpcAllowed(endpoint)) throw new Error(`URL ${endpoint} is not allowed.`);
                                return {
                                    call: (method, args = []) => new Promise((resolve, reject) => {
                                        nextId = (nextId + 1) >>> 0;
                                        const id = nextId;
                                        pending.set(id, { resolve, reject });
                                        try {
                                            const json = JSON.stringify(args, function (key, value) {
                                                const original = this && this[key];
                                                if (original instanceof Date) return { __riinaXmlrpcDate: original.toISOString() };
                                                if (ArrayBuffer.isView(original)) return { __riinaXmlrpcBinary: Array.from(original) };
                                                return value;
                                            });
                                            nativeXmlrpc(id, endpoint, String(method), json);
                                        } catch (error) {
                                            pending.delete(id);
                                            reject(error);
                                        }
                                    })
                                };
                            }
                        };
                        globalThis.__riinaDeliverHttp = (id, response, error) => {
                            const callbacks = pending.get(id);
                            if (!callbacks) return;
                            pending.delete(id);
                            if (error !== null) callbacks.reject(new Error(error));
                            else if (response.statusCode < 400 || response.statusCode >= 600) callbacks.resolve(response);
                            else callbacks.reject(response);
                        };
                        globalThis.__riinaDeliverDownload = (id, error) => {
                            const callbacks = pending.get(id);
                            if (!callbacks) return;
                            pending.delete(id);
                            if (error !== null) callbacks.reject(new Error(error));
                            else callbacks.resolve();
                        };
                        globalThis.__riinaDeliverXmlrpc = (id, response, error) => {
                            const callbacks = pending.get(id);
                            if (!callbacks) return;
                            pending.delete(id);
                            if (error !== null) callbacks.reject(new Error(error));
                            else {
                                const hydrate = value => {
                                    if (Array.isArray(value)) return value.map(hydrate);
                                    if (value && typeof value === 'object') {
                                        if (Array.isArray(value.__riinaXmlrpcBinary)) return new Uint8Array(value.__riinaXmlrpcBinary);
                                        if (typeof value.__riinaXmlrpcDate === 'string') return new Date(value.__riinaXmlrpcDate);
                                        for (const key of Object.keys(value)) value[key] = hydrate(value[key]);
                                    }
                                    return value;
                                };
                                callbacks.resolve(hydrate(response));
                            }
                        };
                        globalThis.__riinaHttp = undefined;
                    })();"#,
                )?;
            }
            if let Some(module_context) = module_context_for_require.clone() {
                globals.set(
                    "__riinaRequire",
                    Func::from(
                        move |ctx: Ctx, path: String| -> rquickjs::Result<Option<String>> {
                            let Some((canonical_path, exports)) =
                                load_plugin_module(ctx.clone(), &module_context, &path).map_err(
                                    |error| {
                                        rquickjs::Error::new_from_js_message(
                                            "require",
                                            "package-local module",
                                            error,
                                        )
                                    },
                                )?
                            else {
                                return Ok(None);
                            };
                            ctx.globals().set("__riinaRequiredModule", exports)?;
                            Ok(Some(canonical_path))
                        },
                    ),
                )?;
                ctx.eval::<(), _>(
                    r#"(() => {
                        const cache = new Map();
                        globalThis.require = path => {
                            const requestedPath = String(path);
                            if (cache.has(requestedPath)) return cache.get(requestedPath);
                            const modulePath = __riinaRequire(requestedPath);
                            if (modulePath == null) return undefined;
                            const module = globalThis.__riinaRequiredModule;
                            delete globalThis.__riinaRequiredModule;
                            cache.set(modulePath, module);
                            cache.set(requestedPath, module);
                            return module;
                        };
                        delete globalThis.__riinaRequire;
                    })();"#,
                )?;
            }
            if global_api_mode != GlobalApiMode::None {
                let global_api = Object::new(ctx.clone())?;
                match global_api_mode {
                    GlobalApiMode::Controller => {
                        let message_events = events.clone();
                        global_api.set(
                            "__postMessage",
                            Func::from(
                                move |target: String, name: String, data: String| -> rquickjs::Result<()> {
                                    let target: serde_json::Value =
                                        decode_plugin_json(&target).map_err(plugin_message_js_error)?;
                                    if !target.is_null() {
                                        return Err(plugin_message_js_error(
                                            "Only broadcast messages are available without plugin-created player instances".into(),
                                        ));
                                    }
                                    let data: serde_json::Value =
                                        decode_plugin_json(&data).map_err(plugin_message_js_error)?;
                                    record_plugin_message(&message_events, name, data)
                                        .map_err(plugin_message_js_error)
                                },
                            ),
                        )?;
                    }
                    GlobalApiMode::Child => {
                        let message_events = events.clone();
                        global_api.set(
                            "__postMessage",
                            Func::from(
                                move |name: String, data: String| -> rquickjs::Result<()> {
                                    let data: serde_json::Value =
                                        decode_plugin_json(&data).map_err(plugin_message_js_error)?;
                                    record_plugin_message(&message_events, name, data)
                                        .map_err(plugin_message_js_error)
                                },
                            ),
                        )?;
                    }
                    GlobalApiMode::None => unreachable!(),
                }
                api.set("global", global_api)?;
                let wrapper = match global_api_mode {
                    GlobalApiMode::Controller => {
                        r#"(() => {
                            const api = iina.global;
                            const postMessage = api.__postMessage;
                            const listeners = new Map();
                            api.postMessage = (target, name, data) => postMessage(JSON.stringify(target), name, JSON.stringify(data));
                            api.onMessage = (name, callback) => {
                                if (typeof name !== 'string' || name.length === 0) throw new TypeError('message name must be a non-empty string');
                                if (typeof callback !== 'function') throw new TypeError('callback must be a function');
                                listeners.set(name, [callback]);
                            };
                            api.__deliverMessage = (name, data) => {
                                for (const callback of listeners.get(name) || []) callback(data, null);
                            };
                            delete api.__postMessage;
                        })();"#
                    }
                    GlobalApiMode::Child => {
                        r#"(() => {
                            const api = iina.global;
                            const postMessage = api.__postMessage;
                            const listeners = new Map();
                            api.postMessage = (name, data) => postMessage(name, JSON.stringify(data));
                            api.onMessage = (name, callback) => {
                                if (typeof name !== 'string' || name.length === 0) throw new TypeError('message name must be a non-empty string');
                                if (typeof callback !== 'function') throw new TypeError('callback must be a function');
                                listeners.set(name, [callback]);
                            };
                            api.getLabel = () => null;
                            api.__deliverMessage = (name, data) => {
                                for (const callback of listeners.get(name) || []) callback(data, null);
                            };
                            delete api.__postMessage;
                        })();"#
                    }
                    GlobalApiMode::None => unreachable!(),
                };
                ctx.eval::<(), _>(wrapper)?;
            }
            let timer_queue = timers.clone();
            globals.set(
                "__riinaScheduleTimer",
                Func::from(move |delay_ms: f64| -> rquickjs::Result<u32> {
                    timer_queue.schedule(delay_ms).map_err(timer_js_error)
                }),
            )?;
            let timer_queue = timers.clone();
            globals.set(
                "__riinaCancelTimer",
                Func::from(move |id: u32| {
                    timer_queue.cancel(id);
                }),
            )?;
            ctx.eval::<(), _>(
                r#"(() => {
                    const scheduleNative = __riinaScheduleTimer;
                    const cancelNative = __riinaCancelTimer;
                    const active = new Map();
                    const callbacks = new Map();
                    let nextHandle = 1;
                    const schedule = handle => {
                        const timer = active.get(handle);
                        if (!timer) return;
                        const nativeId = scheduleNative(timer.delay);
                        timer.nativeId = nativeId;
                        callbacks.set(nativeId, () => {
                            if (!active.has(handle)) return;
                            if (!timer.interval) active.delete(handle);
                            try { timer.callback.call(globalThis); }
                            finally { if (timer.interval && active.has(handle)) schedule(handle); }
                        });
                    };
                    const createTimer = (callback, delay, interval) => {
                        if (typeof callback !== 'function') throw new TypeError('callback must be a function');
                        const milliseconds = Number(delay);
                        const normalized = Number.isFinite(milliseconds) ? Math.max(interval ? 1 : 0, milliseconds) : 0;
                        const handle = 'riina-timer-' + (nextHandle++).toString(36);
                        active.set(handle, { callback, delay: normalized, interval, nativeId: 0 });
                        try { schedule(handle); } catch (error) { active.delete(handle); throw error; }
                        return handle;
                    };
                    const clearTimer = handle => {
                        const key = String(handle);
                        const timer = active.get(key);
                        if (!timer) return;
                        active.delete(key);
                        callbacks.delete(timer.nativeId);
                        cancelNative(timer.nativeId);
                    };
                    globalThis.setTimeout = (callback, delay) => createTimer(callback, delay, false);
                    globalThis.setInterval = (callback, delay) => createTimer(callback, delay, true);
                    globalThis.clearTimeout = clearTimer;
                    globalThis.clearInterval = clearTimer;
                    globalThis.__riinaTimers = { fire: id => {
                        const callback = callbacks.get(id);
                        callbacks.delete(id);
                        if (callback) callback();
                    }};
                })();"#,
            )?;
            let overlay = Object::new(ctx.clone())?;
            let overlay_state = overlay_state.clone();
            let overlay_events = events.clone();
            let overlay_state_for_call = overlay_state.clone();
            let can_use_overlay = permissions.video_overlay;
            overlay.set(
                "loadFile",
                Func::from(move |path: String| -> rquickjs::Result<()> {
                    if !can_use_overlay {
                        return Err(rquickjs::Error::new_from_js_message(
                            "plugin permission",
                            "video overlay",
                            "overlay API called without permission",
                        ));
                    }
                    record_overlay(
                        &overlay_events,
                        &overlay_state_for_call,
                        OverlayIntent::LoadFile(path.into()),
                    )
                    .map_err(overlay_js_error)?;
                    Ok(())
                }),
            )?;
            let overlay_events = events.clone();
            let overlay_state_for_call = overlay_state.clone();
            let can_use_overlay = permissions.video_overlay;
            overlay.set(
                "simpleMode",
                Func::from(move || -> rquickjs::Result<()> {
                    if !can_use_overlay {
                        return Err(rquickjs::Error::new_from_js_message(
                            "plugin permission",
                            "video overlay",
                            "overlay API called without permission",
                        ));
                    }
                    record_overlay(
                        &overlay_events,
                        &overlay_state_for_call,
                        OverlayIntent::SimpleMode,
                    )
                    .map_err(overlay_js_error)?;
                    Ok(())
                }),
            )?;
            for (method, intent) in [("show", 0), ("hide", 1)] {
                let overlay_events = events.clone();
                let overlay_state_for_call = overlay_state.clone();
                overlay.set(
                    method,
                    Func::from(move || -> rquickjs::Result<()> {
                        if can_use_overlay {
                            let intent = if intent == 0 {
                                OverlayIntent::Show
                            } else {
                                OverlayIntent::Hide
                            };
                            record_overlay(&overlay_events, &overlay_state_for_call, intent)
                                .map_err(overlay_js_error)?;
                        }
                        Ok(())
                    }),
                )?;
            }
            let overlay_events = events.clone();
            let overlay_state_for_call = overlay_state.clone();
            overlay.set(
                "setOpacity",
                Func::from(move |opacity: f64| -> rquickjs::Result<()> {
                    if can_use_overlay {
                        record_overlay(
                            &overlay_events,
                            &overlay_state_for_call,
                            OverlayIntent::SetOpacity(opacity),
                        )
                        .map_err(overlay_js_error)?;
                    }
                    Ok(())
                }),
            )?;
            for (method, style) in [("setStyle", true), ("setContent", false)] {
                let overlay_events = events.clone();
                let overlay_state_for_call = overlay_state.clone();
                overlay.set(
                    method,
                    Func::from(move |value: String| -> rquickjs::Result<()> {
                        if can_use_overlay {
                            let intent = if style {
                                OverlayIntent::SetStyle(value)
                            } else {
                                OverlayIntent::SetContent(value)
                            };
                            record_overlay(&overlay_events, &overlay_state_for_call, intent)
                                .map_err(overlay_js_error)?;
                        }
                        Ok(())
                    }),
                )?;
            }
            let overlay_events = events.clone();
            let overlay_state_for_call = overlay_state.clone();
            overlay.set(
                "setClickable",
                Func::from(move |clickable: bool| -> rquickjs::Result<()> {
                    if can_use_overlay {
                        record_overlay(
                            &overlay_events,
                            &overlay_state_for_call,
                            OverlayIntent::SetClickable(clickable),
                        )
                        .map_err(overlay_js_error)?;
                    }
                    Ok(())
                }),
            )?;
            let overlay_events = events.clone();
            let overlay_state_for_call = overlay_state.clone();
            let can_use_overlay = permissions.video_overlay;
            overlay.set(
                "postMessage",
                Func::from(move |name: String, data: String| -> rquickjs::Result<()> {
                    if !can_use_overlay {
                        return Ok(());
                    }
                    let data = serde_json::from_str(&data).map_err(|error| {
                        rquickjs::Error::new_from_js_message("string", "JSON", error.to_string())
                    })?;
                    record_overlay(
                        &overlay_events,
                        &overlay_state_for_call,
                        OverlayIntent::PostMessage { name, data },
                    )
                    .map_err(overlay_js_error)?;
                    Ok(())
                }),
            )?;
            let overlay_events = events.clone();
            let overlay_state_for_call = overlay_state.clone();
            let can_use_overlay = permissions.video_overlay;
            overlay.set(
                "onMessage",
                Func::from(move |name: String, _callback: Value| -> rquickjs::Result<()> {
                    if can_use_overlay {
                        record_overlay(
                            &overlay_events,
                            &overlay_state_for_call,
                            OverlayIntent::OnMessage(name),
                        )
                        .map_err(overlay_js_error)?;
                    }
                    Ok(())
                }),
            )?;
            globals.set("__riinaOverlay", overlay)?;
            ctx.eval::<(), _>(
                r#"iina.overlay = __riinaOverlay;
                (() => {
                    const overlay = iina.overlay;
                    const postMessage = overlay.postMessage;
                    const loadFile = overlay.loadFile;
                    const simpleMode = overlay.simpleMode;
                    const onMessage = overlay.onMessage;
                    const listeners = new Map();
                    overlay.postMessage = (name, data) => postMessage(name, JSON.stringify(data));
                    overlay.loadFile = path => { const result = loadFile(path); listeners.clear(); return result; };
                    overlay.simpleMode = () => { const result = simpleMode(); listeners.clear(); return result; };
                    overlay.onMessage = (name, callback) => {
                        if (typeof callback !== 'function') throw new TypeError('callback must be a function');
                        onMessage(name, callback);
                        const callbacks = listeners.get(name) || [];
                        callbacks.push(callback);
                        listeners.set(name, callbacks);
                    };
                    overlay.__deliverMessage = (name, data) => {
                        for (const callback of listeners.get(name) || []) callback(data);
                    };
                })();"#,
            )?;

            let console = Object::new(ctx.clone())?;
            for method in ["log", "info", "warn", "error"] {
                let log_events = events.clone();
                console.set(
                    method,
                    Func::from(move |message: String| {
                        record(&log_events, HostEvent::Log(message));
                    }),
                )?;
            }
            globals.set("console", console)?;

            // Keep the entry's path on the stack only while it is being evaluated:
            // nested CommonJS loads resolve relative to that file.
            if let Some(module_context) = module_context.as_ref() {
                module_context
                    .current_files
                    .lock()
                    .map_err(|_| rquickjs::Error::new_from_js_message("require", "entry path", "module path stack was poisoned"))?
                    .push(module_context.entry.clone());
            }
            let evaluated = ctx.eval::<Value, _>(source.as_str()).map(|_| ());
            if let Some(module_context) = module_context.as_ref()
                && let Ok(mut current_files) = module_context.current_files.lock()
            {
                current_files.pop();
            }
            evaluated
        });
    if let Err(error) = initialized {
        let _ = ready.send(Err(format!(
            "Plugin {plugin_identifier} failed to initialize: {error}"
        )));
        return;
    }
    if let Err(error) = ready.send(Ok(())) {
        let _ = error;
        return;
    }

    loop {
        let command = match worker_timers.next_wait() {
            Some(timeout) => match commands.recv_timeout(timeout) {
                Ok(command) => Some(command),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            },
            None => match commands.recv() {
                Ok(command) => Some(command),
                Err(_) => break,
            },
        };
        let Some(command) = command else {
            let Some(id) = worker_timers.take_due() else {
                continue;
            };
            if let Ok(mut deadline) = deadline.lock() {
                *deadline = Instant::now() + MAX_EXECUTION_TIME;
            }
            let script = format!("__riinaTimers.fire({id})");
            if let Err(error) = context.with(|ctx| ctx.eval::<Value, _>(script).map(|_| ())) {
                record(
                    &events,
                    HostEvent::Log(format!("Plugin timer callback failed: {error}")),
                );
            }
            continue;
        };
        match command {
            RuntimeCommand::Shutdown => break,
            RuntimeCommand::DeliverOverlayMessage(name, data) => {
                let effect = worker_overlay_state
                    .lock()
                    .map_err(|_| "Plugin overlay state was poisoned".to_owned())
                    .and_then(|state| state.receive_message(name.clone(), data.clone()));
                let Ok(Some(_)) = effect else { continue };
                let script = match serde_json::to_string(&name)
                    .and_then(|name| serde_json::to_string(&data).map(|data| (name, data)))
                {
                    Ok((name, data)) => format!("iina.overlay.__deliverMessage({name}, {data})"),
                    Err(error) => {
                        record(
                            &events,
                            HostEvent::Log(format!("Overlay message encoding failed: {error}")),
                        );
                        continue;
                    }
                };
                if let Ok(mut deadline) = deadline.lock() {
                    *deadline = Instant::now() + MAX_EXECUTION_TIME;
                }
                if let Err(error) = context.with(|ctx| ctx.eval::<Value, _>(script).map(|_| ())) {
                    record(
                        &events,
                        HostEvent::Log(format!("Overlay callback failed: {error}")),
                    );
                }
            }
            RuntimeCommand::DeliverPluginMessage(name, data) => {
                let script = match serde_json::to_string(&name)
                    .and_then(|name| serde_json::to_string(&data).map(|data| (name, data)))
                {
                    Ok((name, data)) => {
                        format!("iina.global.__deliverMessage({name}, {data})")
                    }
                    Err(error) => {
                        record(
                            &events,
                            HostEvent::Log(format!("Plugin message encoding failed: {error}")),
                        );
                        continue;
                    }
                };
                if let Ok(mut deadline) = deadline.lock() {
                    *deadline = Instant::now() + MAX_EXECUTION_TIME;
                }
                if let Err(error) = context.with(|ctx| ctx.eval::<Value, _>(script).map(|_| ())) {
                    record(
                        &events,
                        HostEvent::Log(format!("Plugin message callback failed: {error}")),
                    );
                }
            }
            RuntimeCommand::DeliverPlayerEvent(name, data) => {
                let script = match serde_json::to_string(&name)
                    .and_then(|name| serde_json::to_string(&data).map(|data| (name, data)))
                {
                    Ok((name, data)) => format!("iina.event.__dispatch({name}, {data})"),
                    Err(error) => {
                        record(
                            &events,
                            HostEvent::Log(format!("Player event encoding failed: {error}")),
                        );
                        continue;
                    }
                };
                if let Ok(mut deadline) = deadline.lock() {
                    *deadline = Instant::now() + MAX_EXECUTION_TIME;
                }
                if let Err(error) = context.with(|ctx| ctx.eval::<Value, _>(script).map(|_| ())) {
                    record(
                        &events,
                        HostEvent::Log(format!("Player event callback failed: {error}")),
                    );
                }
            }
            RuntimeCommand::DeliverNetworkResult(id, result) => {
                let (response, error) = match result {
                    Ok(response) => (response.to_string(), "null".to_owned()),
                    Err(error) => match serde_json::to_string(&error) {
                        Ok(error) => ("null".to_owned(), error),
                        Err(_) => continue,
                    },
                };
                let script = format!("__riinaDeliverHttp({id}, {response}, {error})");
                if let Ok(mut deadline) = deadline.lock() {
                    *deadline = Instant::now() + MAX_EXECUTION_TIME;
                }
                if let Err(error) = context.with(|ctx| {
                    ctx.eval::<Value, _>(script).map(|_| ())?;
                    drain_pending_jobs(ctx);
                    Ok::<(), rquickjs::Error>(())
                }) {
                    record(
                        &events,
                        HostEvent::Log(format!("HTTP response callback failed: {error}")),
                    );
                }
            }
            RuntimeCommand::DeliverNetworkDownload(id, result) => {
                let error = match result {
                    Ok(()) => "null".to_owned(),
                    Err(error) => match serde_json::to_string(&error) {
                        Ok(error) => error,
                        Err(_) => continue,
                    },
                };
                let script = format!("__riinaDeliverDownload({id}, {error})");
                if let Ok(mut deadline) = deadline.lock() {
                    *deadline = Instant::now() + MAX_EXECUTION_TIME;
                }
                if let Err(error) = context.with(|ctx| {
                    ctx.eval::<Value, _>(script).map(|_| ())?;
                    drain_pending_jobs(ctx);
                    Ok::<(), rquickjs::Error>(())
                }) {
                    record(
                        &events,
                        HostEvent::Log(format!("HTTP download callback failed: {error}")),
                    );
                }
            }
            RuntimeCommand::DeliverXmlRpcResult(id, result) => {
                let (response, error) = match result {
                    Ok(response) => (response.to_string(), "null".to_owned()),
                    Err(error) => match serde_json::to_string(&error) {
                        Ok(error) => ("null".to_owned(), error),
                        Err(_) => continue,
                    },
                };
                let script = format!("__riinaDeliverXmlrpc({id}, {response}, {error})");
                if let Ok(mut deadline) = deadline.lock() {
                    *deadline = Instant::now() + MAX_EXECUTION_TIME;
                }
                if let Err(error) = context.with(|ctx| {
                    ctx.eval::<Value, _>(script).map(|_| ())?;
                    drain_pending_jobs(ctx);
                    Ok::<(), rquickjs::Error>(())
                }) {
                    record(
                        &events,
                        HostEvent::Log(format!("XML-RPC response callback failed: {error}")),
                    );
                }
            }
        }
    }
}

fn drain_pending_jobs(ctx: Ctx<'_>) {
    for _ in 0..256 {
        if !ctx.execute_pending_job() {
            break;
        }
    }
}

fn install_plugin_file_api<'js>(
    ctx: Ctx<'js>,
    globals: Object<'js>,
    file_system: PluginFiles,
    file_system_granted: bool,
) -> rquickjs::Result<()> {
    let native = Object::new(ctx.clone())?;
    let handles = Arc::new(Mutex::new(BTreeMap::<u32, PluginFileHandle>::new()));
    let next_handle = Arc::new(AtomicU32::new(1));
    let files = file_system.clone();
    let handles_for_open = handles.clone();
    let next_handle_for_open = next_handle.clone();
    native.set(
        "__open",
        Func::from(move |path: String, mode: String| -> rquickjs::Result<u32> {
            let handle = files
                .open_handle(&path, &mode, file_system_granted)
                .map_err(plugin_file_js_error)?;
            let id = next_handle_for_open.fetch_add(1, Ordering::Relaxed);
            let mut handles = handles_for_open
                .lock()
                .map_err(|_| plugin_file_js_error("File handle table is unavailable".into()))?;
            if handles.len() >= MAX_OPEN_FILE_HANDLES {
                return Err(plugin_file_js_error(
                    "Plugin file handle limit reached".into(),
                ));
            }
            handles.insert(id, handle);
            Ok(id)
        }),
    )?;
    let handles_for_offset = handles.clone();
    native.set(
        "__offset",
        Func::from(move |id: u32| -> rquickjs::Result<u64> {
            handles_for_offset
                .lock()
                .map_err(|_| plugin_file_js_error("File handle table is unavailable".into()))?
                .get_mut(&id)
                .ok_or_else(|| plugin_file_js_error("File handle is closed".into()))?
                .offset()
                .map_err(plugin_file_js_error)
        }),
    )?;
    let handles_for_seek = handles.clone();
    native.set(
        "__seek",
        Func::from(move |id: u32, offset: u64| -> rquickjs::Result<()> {
            handles_for_seek
                .lock()
                .map_err(|_| plugin_file_js_error("File handle table is unavailable".into()))?
                .get_mut(&id)
                .ok_or_else(|| plugin_file_js_error("File handle is closed".into()))?
                .seek_to(offset)
                .map_err(plugin_file_js_error)
        }),
    )?;
    let handles_for_seek_end = handles.clone();
    native.set(
        "__seekEnd",
        Func::from(move |id: u32| -> rquickjs::Result<()> {
            handles_for_seek_end
                .lock()
                .map_err(|_| plugin_file_js_error("File handle table is unavailable".into()))?
                .get_mut(&id)
                .ok_or_else(|| plugin_file_js_error("File handle is closed".into()))?
                .seek_to_end()
                .map_err(plugin_file_js_error)
        }),
    )?;
    let handles_for_read = handles.clone();
    native.set(
        "__readHandle",
        Func::from(move |id: u32, length: i64| -> rquickjs::Result<String> {
            let length = usize::try_from(length)
                .map_err(|_| plugin_file_js_error("Read length must be non-negative".into()))?;
            let bytes = handles_for_read
                .lock()
                .map_err(|_| plugin_file_js_error("File handle table is unavailable".into()))?
                .get_mut(&id)
                .ok_or_else(|| plugin_file_js_error("File handle is closed".into()))?
                .read(length)
                .map_err(plugin_file_js_error)?;
            serde_json::to_string(&bytes).map_err(|error| plugin_file_js_error(error.to_string()))
        }),
    )?;
    let handles_for_read_end = handles.clone();
    native.set(
        "__readHandleEnd",
        Func::from(move |id: u32| -> rquickjs::Result<String> {
            let bytes = handles_for_read_end
                .lock()
                .map_err(|_| plugin_file_js_error("File handle table is unavailable".into()))?
                .get_mut(&id)
                .ok_or_else(|| plugin_file_js_error("File handle is closed".into()))?
                .read_to_end()
                .map_err(plugin_file_js_error)?;
            serde_json::to_string(&bytes).map_err(|error| plugin_file_js_error(error.to_string()))
        }),
    )?;
    let handles_for_write = handles.clone();
    native.set(
        "__writeHandle",
        Func::from(
            move |id: u32, encoded_bytes: String| -> rquickjs::Result<()> {
                let bytes: Vec<u8> = serde_json::from_str(&encoded_bytes)
                    .map_err(|error| plugin_file_js_error(error.to_string()))?;
                handles_for_write
                    .lock()
                    .map_err(|_| plugin_file_js_error("File handle table is unavailable".into()))?
                    .get_mut(&id)
                    .ok_or_else(|| plugin_file_js_error("File handle is closed".into()))?
                    .write(&bytes)
                    .map_err(plugin_file_js_error)
            },
        ),
    )?;
    let handles_for_write_text = handles.clone();
    native.set(
        "__writeHandleText",
        Func::from(move |id: u32, content: String| -> rquickjs::Result<()> {
            handles_for_write_text
                .lock()
                .map_err(|_| plugin_file_js_error("File handle table is unavailable".into()))?
                .get_mut(&id)
                .ok_or_else(|| plugin_file_js_error("File handle is closed".into()))?
                .write(content.as_bytes())
                .map_err(plugin_file_js_error)
        }),
    )?;
    let handles_for_close = handles.clone();
    native.set(
        "__close",
        Func::from(move |id: u32| -> rquickjs::Result<()> {
            handles_for_close
                .lock()
                .map_err(|_| plugin_file_js_error("File handle table is unavailable".into()))?
                .remove(&id)
                .ok_or_else(|| plugin_file_js_error("File handle is closed".into()))?;
            Ok(())
        }),
    )?;
    let files = file_system.clone();
    native.set(
        "__exists",
        Func::from(move |path: String| -> rquickjs::Result<bool> {
            files
                .exists(&path, file_system_granted)
                .map_err(plugin_file_js_error)
        }),
    )?;
    let files = file_system.clone();
    native.set(
        "__showInFinder",
        Func::from(move |path: String| -> rquickjs::Result<()> {
            let path = files
                .existing_path(&path, file_system_granted)
                .map_err(plugin_file_js_error)?;
            crate::platform::reveal_file(&path)
                .map_err(|error| plugin_file_js_error(format!("Could not reveal file: {error}")))
        }),
    )?;
    let files = file_system.clone();
    native.set(
        "__read",
        Func::from(
            move |path: String, options: String| -> rquickjs::Result<String> {
                files
                    .read_text(&path, &options, file_system_granted)
                    .map_err(plugin_file_js_error)
            },
        ),
    )?;
    let files = file_system.clone();
    native.set(
        "__write",
        Func::from(
            move |path: String, content: String| -> rquickjs::Result<()> {
                files
                    .write_text(&path, &content, file_system_granted)
                    .map_err(plugin_file_js_error)
            },
        ),
    )?;
    let files = file_system.clone();
    native.set(
        "__delete",
        Func::from(move |path: String| -> rquickjs::Result<()> {
            files.delete(&path).map_err(plugin_file_js_error)
        }),
    )?;
    let files = file_system.clone();
    native.set(
        "__trash",
        Func::from(move |path: String| -> rquickjs::Result<()> {
            files
                .trash(&path, file_system_granted)
                .map_err(plugin_file_js_error)
        }),
    )?;
    let files = file_system.clone();
    native.set(
        "__move",
        Func::from(
            move |source: String, destination: String| -> rquickjs::Result<()> {
                files
                    .move_path(&source, &destination, file_system_granted)
                    .map_err(plugin_file_js_error)
            },
        ),
    )?;
    native.set(
        "__list",
        Func::from(
            move |path: String, options: String| -> rquickjs::Result<String> {
                file_system
                    .list(&path, &options, file_system_granted)
                    .map_err(plugin_file_js_error)
            },
        ),
    )?;
    globals.set("__riinaFile", native)?;
    ctx.eval::<(), _>(
        r#"(() => {
            const native = __riinaFile;
            iina.file = {
                exists: path => native.__exists(String(path)),
                showInFinder: path => native.__showInFinder(String(path)),
                read: (path, options = {}) => native.__read(String(path), JSON.stringify(options)),
                write: (path, content) => native.__write(String(path), String(content)),
                delete: path => native.__delete(String(path)),
                trash: path => native.__trash(String(path)),
                move: (source, destination) => native.__move(String(source), String(destination)),
                list: (path, options = {}) => JSON.parse(native.__list(String(path), JSON.stringify(options))),
                handle: (path, mode) => {
                    const id = native.__open(String(path), String(mode));
                    return {
                        offset: () => native.__offset(id),
                        seekTo: offset => native.__seek(id, Number(offset)),
                        seekToEnd: () => native.__seekEnd(id),
                        read: length => new Uint8Array(JSON.parse(native.__readHandle(id, Number(length)))),
                        readToEnd: () => new Uint8Array(JSON.parse(native.__readHandleEnd(id))),
                        write: data => {
                            if (typeof data === 'string') return native.__writeHandleText(id, data);
                            if (data && data.length > 4194304) throw new Error('Handle writes are limited to 4 MiB per call');
                            const bytes = Array.from(data);
                            if (bytes.length > 4194304) throw new Error('Handle writes are limited to 4 MiB per call');
                            return native.__writeHandle(id, JSON.stringify(bytes));
                        },
                        close: () => native.__close(id)
                    };
                }
            };
            globalThis.__riinaFile = undefined;
        })();"#,
    )?;
    Ok(())
}

fn plugin_file_js_error(message: String) -> rquickjs::Error {
    rquickjs::Error::new_from_js_message("plugin file", "an allowed plugin path", message)
}

fn install_plugin_core_api<'js>(
    ctx: Ctx<'js>,
    iina_api: Object<'js>,
    events: Arc<Mutex<Vec<HostEvent>>>,
    files: PluginFiles,
    permissions: AvailablePermissions,
    allowed_domains: Vec<String>,
) -> rquickjs::Result<()> {
    let core_api = Object::new(ctx.clone())?;
    let open_events = events.clone();
    let open_files = files.clone();
    let open_domains = allowed_domains.clone();
    core_api.set(
        "open",
        Func::from(move |value: String| -> rquickjs::Result<()> {
            if value.len() > 16 * 1024 || value.contains('\0') {
                return Err(core_js_error("Media reference is invalid".into()));
            }
            let path = plugin_media_reference(
                &value,
                &open_files,
                permissions.file_system,
                permissions.network_request,
                &open_domains,
            )
            .map_err(core_js_error)?;
            record(&open_events, HostEvent::CoreCommand(Command::Load(path)));
            Ok(())
        }),
    )?;
    let osd_events = events.clone();
    let osd_permitted = permissions.show_osd;
    core_api.set(
        "osd",
        Func::from(move |message: String| {
            if osd_permitted {
                record(&osd_events, HostEvent::ShowOsd(message));
            }
        }),
    )?;
    for (name, command) in [
        ("pause", Command::SetPaused(true)),
        ("resume", Command::SetPaused(false)),
        ("stop", Command::Stop),
    ] {
        let command_events = events.clone();
        core_api.set(
            name,
            Func::from(move || record(&command_events, HostEvent::CoreCommand(command.clone()))),
        )?;
    }
    let seek_events = events.clone();
    core_api.set(
        "seek",
        Func::from(move |seconds: f64, exact: bool| -> rquickjs::Result<()> {
            if !seconds.is_finite() {
                return Err(core_js_error("Seek must be a finite number".into()));
            }
            let seek_type = if exact {
                SeekType::Exact
            } else {
                SeekType::Keyframe
            };
            record(
                &seek_events,
                HostEvent::CoreCommand(Command::SeekWithType { seconds, seek_type }),
            );
            Ok(())
        }),
    )?;
    let seek_events = events.clone();
    core_api.set(
        "seekTo",
        Func::from(move |seconds: f64| -> rquickjs::Result<()> {
            if !seconds.is_finite() || seconds < 0.0 {
                return Err(core_js_error(
                    "Absolute seek must be a finite non-negative number".into(),
                ));
            }
            record(
                &seek_events,
                HostEvent::CoreCommand(Command::Seek {
                    seconds,
                    absolute: true,
                }),
            );
            Ok(())
        }),
    )?;
    let speed_events = events.clone();
    core_api.set(
        "setSpeed",
        Func::from(move |speed: f64| -> rquickjs::Result<()> {
            if !speed.is_finite() || speed < 0.01 {
                return Err(core_js_error(
                    "Speed must be finite and at least 0.01".into(),
                ));
            }
            record(&speed_events, HostEvent::CoreCommand(Command::Speed(speed)));
            Ok(())
        }),
    )?;
    let ui_events = events.clone();
    core_api.set(
        "setUIVisibility",
        Func::from(move |visible: bool| {
            record(&ui_events, HostEvent::SetUiVisibility(visible));
        }),
    )?;
    let chapter_events = events.clone();
    core_api.set(
        "playChapter",
        Func::from(move |index: i32| -> rquickjs::Result<()> {
            let index = usize::try_from(index)
                .map_err(|_| core_js_error("Chapter index must be non-negative".into()))?;
            record(
                &chapter_events,
                HostEvent::CoreCommand(Command::SelectChapter(index)),
            );
            Ok(())
        }),
    )?;
    let status_files = files.clone();
    core_api.set(
        "__readStatus",
        Func::from(move |name: String| -> rquickjs::Result<Option<String>> {
            status_files
                .core_status_property(&name)
                .map(|value| value.map(|value| value.to_string()))
                .map_err(core_js_error)
        }),
    )?;
    let track_read_files = files.clone();
    core_api.set(
        "__readCoreProperty",
        Func::from(move |name: String| -> rquickjs::Result<Option<String>> {
            track_read_files
                .core_status_property(&name)
                .map(|value| value.map(|value| value.to_string()))
                .map_err(core_js_error)
        }),
    )?;
    let track_write_events = events.clone();
    core_api.set(
        "__writeCoreProperty",
        Func::from(
            move |name: String, serialized: String| -> rquickjs::Result<()> {
                let value: serde_json::Value = serde_json::from_str(&serialized)
                    .map_err(|_| core_js_error("Track property value is not valid JSON".into()))?;
                let command = match (name.as_str(), value) {
                    ("audio.id", serde_json::Value::Null) => None,
                    ("audio.id", value) => Some(Command::SelectTrack {
                        kind: riina_core::TrackKind::Audio,
                        id: json_track_id(value)?,
                    }),
                    ("audio.delay", value) => Some(Command::AudioDelay(json_finite_number(
                        value,
                        "Audio delay",
                    )?)),
                    ("audio.volume", value) => {
                        let volume = json_finite_number(value, "Volume")?;
                        if !(0.0..=200.0).contains(&volume) {
                            return Err(core_js_error("Volume must be between 0 and 200".into()));
                        }
                        Some(Command::Volume(volume))
                    }
                    ("audio.muted", serde_json::Value::Bool(muted)) => {
                        Some(Command::SetMuted(muted))
                    }
                    ("audio.muted", _) => {
                        return Err(core_js_error("Muted must be a boolean".into()));
                    }
                    ("subtitle.id", serde_json::Value::Null) => None,
                    ("subtitle.id", value) => Some(Command::SelectTrack {
                        kind: riina_core::TrackKind::Sub,
                        id: json_track_id(value)?,
                    }),
                    ("subtitle.secondID", serde_json::Value::Null) => None,
                    ("subtitle.secondID", value) => Some(Command::SelectSecondarySubtitle(Some(
                        json_track_id(value)?.ok_or_else(|| {
                            core_js_error("Subtitle track id must be an integer".into())
                        })?,
                    ))),
                    ("subtitle.delay", value) => Some(Command::SubtitleDelay(json_finite_number(
                        value,
                        "Subtitle delay",
                    )?)),
                    ("video.id", serde_json::Value::Null) => None,
                    ("video.id", value) => Some(Command::SelectTrack {
                        kind: riina_core::TrackKind::Video,
                        id: json_track_id(value)?,
                    }),
                    _ => None,
                };
                if let Some(command) = command {
                    record(&track_write_events, HostEvent::CoreCommand(command));
                }
                Ok(())
            },
        ),
    )?;
    let track_load_events = events.clone();
    let track_load_files = files.clone();
    let track_load_domains = allowed_domains.clone();
    core_api.set(
        "__loadTrack",
        Func::from(move |kind: String, value: String| -> rquickjs::Result<()> {
            if value.len() > 16 * 1024 || value.contains('\0') {
                return Err(core_js_error("Track media reference is invalid".into()));
            }
            let path = plugin_media_reference(
                &value,
                &track_load_files,
                permissions.file_system,
                permissions.network_request,
                &track_load_domains,
            )
            .map_err(core_js_error)?;
            let command = match kind.as_str() {
                "audio" => Command::AddAudio(path),
                "subtitle" => Command::AddSubtitle(path),
                "video" => Command::AddVideo(path),
                _ => return Err(core_js_error("Unknown track type".into())),
            };
            record(&track_load_events, HostEvent::CoreCommand(command));
            Ok(())
        }),
    )?;
    let chapter_files = files.clone();
    core_api.set(
        "__readChapters",
        Func::from(move || -> rquickjs::Result<String> {
            chapter_files
                .core_chapters()
                .and_then(|chapters| {
                    serde_json::to_string(&chapters).map_err(|error| error.to_string())
                })
                .map_err(core_js_error)
        }),
    )?;
    let recent_document_files = files.clone();
    core_api.set(
        "__readRecentDocuments",
        Func::from(move || -> rquickjs::Result<String> {
            recent_document_files
                .core_recent_documents()
                .and_then(|documents| {
                    serde_json::to_string(&documents).map_err(|error| error.to_string())
                })
                .map_err(core_js_error)
        }),
    )?;
    let history_files = files.clone();
    core_api.set(
        "__readHistory",
        Func::from(move || -> rquickjs::Result<String> {
            history_files
                .core_playback_history()
                .and_then(|history| {
                    serde_json::to_string(&history).map_err(|error| error.to_string())
                })
                .map_err(core_js_error)
        }),
    )?;
    let version_files = files.clone();
    core_api.set(
        "__readVersion",
        Func::from(move || -> rquickjs::Result<String> {
            version_files
                .core_version()
                .and_then(|version| {
                    serde_json::to_string(&version).map_err(|error| error.to_string())
                })
                .map_err(core_js_error)
        }),
    )?;
    let window_files = files.clone();
    core_api.set(
        "__readWindowProperty",
        Func::from(move |name: String| -> rquickjs::Result<Option<String>> {
            window_files
                .core_window_property(&name)
                .and_then(|value| {
                    value
                        .map(|value| {
                            serde_json::to_string(&value).map_err(|error| error.to_string())
                        })
                        .transpose()
                })
                .map_err(core_js_error)
        }),
    )?;
    let window_events = events.clone();
    core_api.set(
        "__writeWindowProperty",
        Func::from(
            move |name: String, serialized: String| -> rquickjs::Result<bool> {
                let value: serde_json::Value = serde_json::from_str(&serialized)
                    .map_err(|_| core_js_error("Window property value is not valid JSON".into()))?;
                let Some(action) = plugin_window_action(&name, value)? else {
                    return Ok(false);
                };
                record(&window_events, HostEvent::WindowAction(action));
                Ok(true)
            },
        ),
    )?;
    iina_api.set("core", core_api)?;
    ctx.eval::<(), _>(
        r#"(() => {
            const core = iina.core;
            const readStatus = core.__readStatus;
            const readChapters = core.__readChapters;
            const readRecentDocuments = core.__readRecentDocuments;
            const readHistory = core.__readHistory;
            const readVersion = core.__readVersion;
            const readWindowProperty = core.__readWindowProperty;
            const writeWindowProperty = core.__writeWindowProperty;
            const readCoreProperty = core.__readCoreProperty;
            const writeCoreProperty = core.__writeCoreProperty;
            const loadTrack = core.__loadTrack;
            core.status = new Proxy(Object.create(null), {
                get(_target, property) {
                    if (typeof property !== 'string') return undefined;
                    const serialized = readStatus(property);
                    return serialized == null ? undefined : JSON.parse(serialized);
                },
                set() { return false; }
            });
            core.getChapters = () => JSON.parse(readChapters());
            core.getRecentDocuments = () => JSON.parse(readRecentDocuments());
            core.getHistory = () => JSON.parse(readHistory()).map(entry => ({
                ...entry,
                date: new Date(entry.date)
            }));
            core.getVersion = () => JSON.parse(readVersion());
            core.window = new Proxy(Object.create(null), {
                get(_target, property) {
                    if (typeof property !== 'string') return undefined;
                    const serialized = readWindowProperty(property);
                    return serialized == null ? undefined : JSON.parse(serialized);
                },
                set(_target, property, value) {
                    if (typeof property !== 'string') return false;
                    return writeWindowProperty(property, JSON.stringify(value));
                }
            });
            for (const namespace of ['audio', 'subtitle', 'video']) {
                const api = new Proxy(Object.create(null), {
                    get(_target, property) {
                        if (property === 'loadTrack') return url => loadTrack(namespace, url);
                        if (typeof property !== 'string') return undefined;
                        const serialized = readCoreProperty(`${namespace}.${property}`);
                        return serialized == null ? undefined : JSON.parse(serialized);
                    },
                    set(_target, property, value) {
                        if (typeof property === 'string') writeCoreProperty(`${namespace}.${property}`, JSON.stringify(value));
                        return true;
                    }
                });
                core[namespace] = api;
            }
            delete core.__readStatus;
            delete core.__readChapters;
            delete core.__readRecentDocuments;
            delete core.__readHistory;
            delete core.__readVersion;
            delete core.__readWindowProperty;
            delete core.__writeWindowProperty;
            delete core.__readCoreProperty;
            delete core.__writeCoreProperty;
            delete core.__loadTrack;
        })();"#,
    )?;
    Ok(())
}

fn json_track_id(value: serde_json::Value) -> rquickjs::Result<Option<i64>> {
    value
        .as_i64()
        .filter(|id| *id > 0)
        .map(Some)
        .ok_or_else(|| core_js_error("Track id must be a positive integer".into()))
}

fn json_finite_number(value: serde_json::Value, label: &str) -> rquickjs::Result<f64> {
    value
        .as_f64()
        .filter(|number| number.is_finite())
        .ok_or_else(|| core_js_error(format!("{label} must be a finite number")))
}

fn plugin_window_action(
    name: &str,
    value: serde_json::Value,
) -> rquickjs::Result<Option<WindowAction>> {
    let invalid = |label: &str| core_js_error(format!("Window {label} has an invalid value"));
    let action = match name {
        "frame" => {
            let object = value.as_object().ok_or_else(|| invalid("frame"))?;
            let number = |name: &str| {
                object
                    .get(name)
                    .cloned()
                    .ok_or_else(|| invalid("frame"))
                    .and_then(|value| json_finite_number(value, "frame"))
            };
            let (x, y, width, height) = (
                number("x")?,
                number("y")?,
                number("width")?,
                number("height")?,
            );
            if width <= 0.0 || height <= 0.0 {
                return Err(invalid("frame"));
            }
            WindowAction::SetFrame {
                x,
                y,
                width,
                height,
            }
        }
        "fullscreen" => {
            WindowAction::SetFullscreen(value.as_bool().ok_or_else(|| invalid("fullscreen"))?)
        }
        "pip" => WindowAction::SetPictureInPicture(value.as_bool().ok_or_else(|| invalid("pip"))?),
        "ontop" => WindowAction::SetOnTop(value.as_bool().ok_or_else(|| invalid("ontop"))?),
        "sidebar" => {
            let name = match value {
                serde_json::Value::Null => None,
                serde_json::Value::String(name) if name.len() <= 128 && !name.contains('\0') => {
                    Some(name)
                }
                _ => return Err(invalid("sidebar")),
            };
            WindowAction::SetSidebar(name)
        }
        "miniaturized" => {
            WindowAction::SetMiniaturized(value.as_bool().ok_or_else(|| invalid("miniaturized"))?)
        }
        _ => return Ok(None),
    };
    Ok(Some(action))
}

fn plugin_media_reference(
    value: &str,
    files: &PluginFiles,
    file_system_granted: bool,
    network_granted: bool,
    allowed_domains: &[String],
) -> Result<String, String> {
    if riina_core::media_reference::is_network_url_candidate(value) {
        riina_core::media_reference::validate_network_url(value).map_err(str::to_owned)?;
        if !network_granted || !plugin_media_url_allowed(value, allowed_domains) {
            return Err("Network permission and an allowed domain are required".into());
        }
        return Ok(value.to_owned());
    }
    if let Ok(url) = url::Url::parse(value) {
        match url.scheme() {
            "file" => {
                let path = url
                    .to_file_path()
                    .map_err(|_| "File URL is invalid".to_owned())?;
                return files
                    .existing_path(&path.to_string_lossy(), file_system_granted)
                    .map(|path| path.to_string_lossy().into_owned());
            }
            "data" | "memory" => return Ok(value.to_owned()),
            _ if !file_system_granted => {
                return Err("The file-system permission is required for this URL scheme".into());
            }
            _ => return Ok(value.to_owned()),
        }
    }
    files
        .existing_path(value, file_system_granted)
        .map(|path| path.to_string_lossy().into_owned())
}

fn plugin_media_url_allowed(value: &str, allowed_domains: &[String]) -> bool {
    if riina_core::media_reference::validate_network_url(value).is_err() {
        return false;
    }
    let Ok(outer_url) = url::Url::parse(value) else {
        return false;
    };
    let url = if outer_url.scheme() == "ytdl" {
        let Some(source) = value.strip_prefix("ytdl://") else {
            return false;
        };
        let Ok(source_url) = url::Url::parse(source) else {
            return false;
        };
        if riina_core::media_reference::validate_network_url(source).is_err() {
            return false;
        }
        source_url
    } else {
        outer_url
    };
    if !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    let Some(host) = url.host_str().map(str::to_ascii_lowercase) else {
        return false;
    };
    allowed_domains.iter().any(|pattern| {
        if pattern == "*" {
            return true;
        }
        if let Some(domain) = pattern.strip_prefix("*.") {
            let domain = domain.to_ascii_lowercase();
            host.len() > domain.len()
                && host
                    .strip_suffix(&domain)
                    .is_some_and(|prefix| prefix.ends_with('.'))
        } else {
            host == pattern.to_ascii_lowercase()
        }
    })
}

fn core_js_error(message: String) -> rquickjs::Error {
    rquickjs::Error::new_from_js_message("iina.core", "a valid permitted player command", message)
}

fn http_js_error(message: String) -> rquickjs::Error {
    rquickjs::Error::new_from_js_message("HTTP request", "valid allowed request", message)
}

fn perform_http_request(
    method: &str,
    raw_url: &str,
    options: &serde_json::Value,
    allowed_domains: &[String],
) -> Result<serde_json::Value, String> {
    let mut current_url = url::Url::parse(raw_url).map_err(|error| error.to_string())?;
    if let Some(params) = options.get("params") {
        let params = params
            .as_object()
            .ok_or_else(|| "HTTP params must be an object".to_owned())?;
        let mut query = current_url.query_pairs_mut();
        for (key, value) in params {
            query.append_pair(key, &http_option_string(value));
        }
    }

    let mut method = ureq::http::Method::from_bytes(method.as_bytes())
        .map_err(|error| format!("Invalid HTTP method: {error}"))?;
    let mut body = Vec::new();
    let data = options.get("data").filter(|value| !value.is_null());
    if let Some(data) = data {
        let fields = data
            .as_object()
            .ok_or_else(|| "HTTP data must be an object".to_owned())?;
        let mut form = url::form_urlencoded::Serializer::new(String::new());
        for (key, value) in fields {
            form.append_pair(key, &http_option_string(value));
        }
        body = form.finish().into_bytes();
    }
    if body.len() > MAX_NETWORK_REQUEST_BYTES {
        return Err("HTTP request body exceeds the 1 MiB limit".into());
    }

    let headers = options
        .get("headers")
        .map(|value| {
            value
                .as_object()
                .ok_or_else(|| "HTTP headers must be an object".to_owned())
        })
        .transpose()?;
    let mut has_content_type = false;
    let mut headers_out = ureq::http::HeaderMap::new();
    if let Some(headers) = headers {
        for (name, value) in headers {
            let name = ureq::http::HeaderName::from_bytes(name.as_bytes())
                .map_err(|error| format!("Invalid HTTP header name: {error}"))?;
            let value = ureq::http::HeaderValue::from_str(&http_option_string(value))
                .map_err(|error| format!("Invalid HTTP header value: {error}"))?;
            has_content_type |= name == ureq::http::header::CONTENT_TYPE;
            headers_out.insert(name, value);
        }
    }
    if data.is_some() && !has_content_type {
        headers_out.insert(
            ureq::http::header::CONTENT_TYPE,
            ureq::http::HeaderValue::from_static("application/x-www-form-urlencoded"),
        );
    }

    let mut response = None;
    let request_started = Instant::now();
    for redirect_count in 0..=10 {
        let mut request = ureq::http::Request::builder()
            .method(method.clone())
            .uri(current_url.as_str())
            .body(body.clone())
            .map_err(|error| format!("Could not construct HTTP request: {error}"))?;
        *request.headers_mut() = headers_out.clone();
        let remaining = NETWORK_TIMEOUT.saturating_sub(request_started.elapsed());
        if remaining.is_zero() {
            return Err("HTTP request exceeded the 30 second timeout".into());
        }
        let agent = ureq::Agent::new_with_config(
            ureq::Agent::config_builder()
                .timeout_global(Some(remaining))
                .max_redirects(0)
                .http_status_as_error(false)
                .build(),
        );
        let current_response = agent
            .run(request)
            .map_err(|error| format!("HTTP request failed: {error}"))?;
        let status = current_response.status();
        let is_redirect = matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308);
        let next_url = if is_redirect {
            current_response
                .headers()
                .get(ureq::http::header::LOCATION)
                .and_then(|location| location.to_str().ok())
                .map(|location| current_url.join(location))
                .transpose()
                .map_err(|error| format!("Invalid HTTP redirect URL: {error}"))?
        } else {
            None
        };
        let Some(next_url) = next_url.filter(|_| redirect_count < 10) else {
            response = Some(current_response);
            break;
        };
        if !network_url_allowed(next_url.as_str(), allowed_domains) {
            return Err("HTTP redirect target is not in the plugin's allowed domains".into());
        }
        if !current_url
            .host_str()
            .zip(next_url.host_str())
            .is_some_and(|(current, next)| current.eq_ignore_ascii_case(next))
        {
            headers_out.remove(ureq::http::header::AUTHORIZATION);
            headers_out.remove(ureq::http::header::COOKIE);
        }
        if status.as_u16() == 303
            || (matches!(status.as_u16(), 301 | 302) && method == ureq::http::Method::POST)
        {
            method = ureq::http::Method::GET;
            body.clear();
            headers_out.remove(ureq::http::header::CONTENT_TYPE);
        }
        current_url = next_url;
        // Dropping a redirect response before following it keeps its body out
        // of memory; the global request deadline still applies to every hop.
        drop(current_response);
    }
    let mut response = response.ok_or_else(|| "HTTP redirect limit was exceeded".to_owned())?;
    let status = response.status();
    let text = response
        .body_mut()
        .with_config()
        .limit(4 * 1024 * 1024)
        .read_to_string()
        .map_err(|error| format!("Could not read HTTP response: {error}"))?;
    let data = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    Ok(serde_json::json!({
        "statusCode": status.as_u16(),
        "reason": status.canonical_reason().unwrap_or(""),
        "data": data,
        "text": text,
    }))
}

fn perform_http_download(
    raw_url: &str,
    options: &serde_json::Value,
    allowed_domains: &[String],
) -> Result<Vec<u8>, String> {
    let mut current_url = url::Url::parse(raw_url).map_err(|error| error.to_string())?;
    if let Some(params) = options.get("params") {
        let params = params
            .as_object()
            .ok_or_else(|| "HTTP params must be an object".to_owned())?;
        let mut query = current_url.query_pairs_mut();
        for (key, value) in params {
            query.append_pair(key, &http_option_string(value));
        }
    }
    let method = options
        .get("method")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("GET");
    let mut method = ureq::http::Method::from_bytes(method.as_bytes())
        .map_err(|error| format!("Invalid HTTP method: {error}"))?;
    let data = options.get("data").filter(|value| !value.is_null());
    let mut body = if let Some(data) = data {
        let fields = data
            .as_object()
            .ok_or_else(|| "HTTP data must be an object".to_owned())?;
        let mut form = url::form_urlencoded::Serializer::new(String::new());
        for (key, value) in fields {
            form.append_pair(key, &http_option_string(value));
        }
        form.finish().into_bytes()
    } else {
        Vec::new()
    };
    if body.len() > MAX_NETWORK_REQUEST_BYTES {
        return Err("HTTP request body exceeds the 1 MiB limit".into());
    }
    let headers = options
        .get("headers")
        .map(|value| {
            value
                .as_object()
                .ok_or_else(|| "HTTP headers must be an object".to_owned())
        })
        .transpose()?;
    let mut has_content_type = false;
    let mut headers_out = ureq::http::HeaderMap::new();
    if let Some(headers) = headers {
        for (name, value) in headers {
            let name = ureq::http::HeaderName::from_bytes(name.as_bytes())
                .map_err(|error| format!("Invalid HTTP header name: {error}"))?;
            let value = ureq::http::HeaderValue::from_str(&http_option_string(value))
                .map_err(|error| format!("Invalid HTTP header value: {error}"))?;
            has_content_type |= name == ureq::http::header::CONTENT_TYPE;
            headers_out.insert(name, value);
        }
    }
    if data.is_some() && !has_content_type {
        headers_out.insert(
            ureq::http::header::CONTENT_TYPE,
            ureq::http::HeaderValue::from_static("application/x-www-form-urlencoded"),
        );
    }

    let mut response = None;
    let request_started = Instant::now();
    for redirect_count in 0..=10 {
        let mut request = ureq::http::Request::builder()
            .method(method.clone())
            .uri(current_url.as_str())
            .body(body.clone())
            .map_err(|error| format!("Could not construct HTTP request: {error}"))?;
        *request.headers_mut() = headers_out.clone();
        let remaining = NETWORK_TIMEOUT.saturating_sub(request_started.elapsed());
        if remaining.is_zero() {
            return Err("HTTP download exceeded the 30 second timeout".into());
        }
        let agent = ureq::Agent::new_with_config(
            ureq::Agent::config_builder()
                .timeout_global(Some(remaining))
                .max_redirects(0)
                .http_status_as_error(false)
                .build(),
        );
        let current_response = agent
            .run(request)
            .map_err(|error| format!("HTTP download failed: {error}"))?;
        let status = current_response.status();
        let is_redirect = matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308);
        let next_url = if is_redirect {
            current_response
                .headers()
                .get(ureq::http::header::LOCATION)
                .and_then(|location| location.to_str().ok())
                .map(|location| current_url.join(location))
                .transpose()
                .map_err(|error| format!("Invalid HTTP redirect URL: {error}"))?
        } else {
            None
        };
        let Some(next_url) = next_url.filter(|_| redirect_count < 10) else {
            response = Some(current_response);
            break;
        };
        if !network_url_allowed(next_url.as_str(), allowed_domains) {
            return Err("HTTP redirect target is not in the plugin's allowed domains".into());
        }
        if !current_url
            .host_str()
            .zip(next_url.host_str())
            .is_some_and(|(current, next)| current.eq_ignore_ascii_case(next))
        {
            headers_out.remove(ureq::http::header::AUTHORIZATION);
            headers_out.remove(ureq::http::header::COOKIE);
        }
        if status.as_u16() == 303
            || (matches!(status.as_u16(), 301 | 302) && method == ureq::http::Method::POST)
        {
            method = ureq::http::Method::GET;
            body.clear();
            headers_out.remove(ureq::http::header::CONTENT_TYPE);
        }
        current_url = next_url;
        drop(current_response);
    }
    let mut response = response.ok_or_else(|| "HTTP redirect limit was exceeded".to_owned())?;
    let status = response.status();
    if !(200..300).contains(&status.as_u16()) {
        return Err(format!(
            "HTTP download failed with status {}",
            status.as_u16()
        ));
    }
    let content = response
        .body_mut()
        .with_config()
        .limit((MAX_PLUGIN_DOWNLOAD_BYTES + 1) as u64)
        .read_to_vec()
        .map_err(|error| format!("Could not read HTTP download: {error}"))?;
    if content.len() > MAX_PLUGIN_DOWNLOAD_BYTES {
        return Err("Download exceeds the 64 MiB limit".into());
    }
    Ok(content)
}

fn perform_xmlrpc_call(
    raw_url: &str,
    method_name: &str,
    arguments: &serde_json::Value,
    allowed_domains: &[String],
) -> Result<serde_json::Value, String> {
    let request_body = build_xmlrpc_request(method_name, arguments)?;
    if request_body.len() > MAX_NETWORK_REQUEST_BYTES {
        return Err("XML-RPC request exceeds the 1 MiB limit".into());
    }
    let mut current_url = url::Url::parse(raw_url).map_err(|error| error.to_string())?;
    let request_started = Instant::now();
    let mut response = None;
    for redirect_count in 0..=10 {
        let request = ureq::http::Request::builder()
            .method(ureq::http::Method::POST)
            .uri(current_url.as_str())
            .header(ureq::http::header::CONTENT_TYPE, "text/xml; charset=utf-8")
            .header(ureq::http::header::ACCEPT, "text/xml")
            .body(request_body.clone())
            .map_err(|error| format!("Could not construct XML-RPC request: {error}"))?;
        let remaining = NETWORK_TIMEOUT.saturating_sub(request_started.elapsed());
        if remaining.is_zero() {
            return Err("XML-RPC request exceeded the 30 second timeout".into());
        }
        let agent = ureq::Agent::new_with_config(
            ureq::Agent::config_builder()
                .timeout_global(Some(remaining))
                .max_redirects(0)
                .http_status_as_error(false)
                .build(),
        );
        let current_response = agent
            .run(request)
            .map_err(|error| format!("XML-RPC request failed: {error}"))?;
        let status = current_response.status();
        let is_redirect = matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308);
        let next_url = if is_redirect {
            current_response
                .headers()
                .get(ureq::http::header::LOCATION)
                .and_then(|location| location.to_str().ok())
                .map(|location| current_url.join(location))
                .transpose()
                .map_err(|error| format!("Invalid XML-RPC redirect URL: {error}"))?
        } else {
            None
        };
        let Some(next_url) = next_url.filter(|_| redirect_count < 10) else {
            response = Some(current_response);
            break;
        };
        if !network_url_allowed(next_url.as_str(), allowed_domains) {
            return Err("XML-RPC redirect target is not in the plugin's allowed domains".into());
        }
        current_url = next_url;
        drop(current_response);
    }
    let mut response = response.ok_or_else(|| "XML-RPC redirect limit was exceeded".to_owned())?;
    let status = response.status();
    let body = response
        .body_mut()
        .with_config()
        .limit((MAX_XMLRPC_RESPONSE_BYTES + 1) as u64)
        .read_to_vec()
        .map_err(|error| format!("Could not read XML-RPC response: {error}"))?;
    if body.len() > MAX_XMLRPC_RESPONSE_BYTES {
        return Err("XML-RPC response exceeds the 4 MiB limit".into());
    }
    if !(200..300).contains(&status.as_u16()) {
        return Err(format!(
            "XML-RPC request failed with status {}",
            status.as_u16()
        ));
    }
    let document = roxmltree::Document::parse(
        std::str::from_utf8(&body)
            .map_err(|error| format!("XML-RPC response is not UTF-8: {error}"))?,
    )
    .map_err(|error| format!("Invalid XML-RPC response: {error}"))?;
    let root = document.root_element();
    if let Some(fault) = root.children().find(|node| node.has_tag_name("fault")) {
        let details = fault
            .descendants()
            .find(|node| node.has_tag_name("value"))
            .map(|value| parse_xmlrpc_value(value, 0))
            .transpose()?
            .unwrap_or(serde_json::Value::Null);
        return Err(format!("XML-RPC fault: {details}"));
    }
    let value = root
        .children()
        .find(|node| node.has_tag_name("params"))
        .and_then(|params| params.children().find(|node| node.has_tag_name("param")))
        .and_then(|param| param.children().find(|node| node.has_tag_name("value")))
        .ok_or_else(|| "XML-RPC server returned no value".to_owned())?;
    parse_xmlrpc_value(value, 0)
}

const MAX_XMLRPC_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

fn build_xmlrpc_request(
    method_name: &str,
    arguments: &serde_json::Value,
) -> Result<Vec<u8>, String> {
    use quick_xml::Writer;

    let mut writer = Writer::new(Vec::new());
    xmlrpc_start(&mut writer, "methodCall")?;
    xmlrpc_text_element(&mut writer, "methodName", method_name)?;
    xmlrpc_start(&mut writer, "params")?;
    for argument in arguments
        .as_array()
        .ok_or_else(|| "XML-RPC arguments must be an array".to_owned())?
    {
        xmlrpc_start(&mut writer, "param")?;
        write_xmlrpc_value(&mut writer, argument, 0)?;
        xmlrpc_end(&mut writer, "param")?;
    }
    xmlrpc_end(&mut writer, "params")?;
    xmlrpc_end(&mut writer, "methodCall")?;
    Ok(writer.into_inner())
}

fn xmlrpc_start(writer: &mut quick_xml::Writer<Vec<u8>>, name: &str) -> Result<(), String> {
    writer
        .write_event(quick_xml::events::Event::Start(
            quick_xml::events::BytesStart::new(name),
        ))
        .map_err(|error| error.to_string())
}

fn xmlrpc_end(writer: &mut quick_xml::Writer<Vec<u8>>, name: &str) -> Result<(), String> {
    writer
        .write_event(quick_xml::events::Event::End(
            quick_xml::events::BytesEnd::new(name),
        ))
        .map_err(|error| error.to_string())
}

fn xmlrpc_text_element(
    writer: &mut quick_xml::Writer<Vec<u8>>,
    name: &str,
    text: &str,
) -> Result<(), String> {
    xmlrpc_start(writer, name)?;
    writer
        .write_event(quick_xml::events::Event::Text(
            quick_xml::events::BytesText::new(text),
        ))
        .map_err(|error| error.to_string())?;
    xmlrpc_end(writer, name)
}

fn write_xmlrpc_value(
    writer: &mut quick_xml::Writer<Vec<u8>>,
    value: &serde_json::Value,
    depth: usize,
) -> Result<(), String> {
    use base64::Engine;
    use quick_xml::events::{BytesStart, Event};

    if depth > 64 {
        return Err("XML-RPC value nesting exceeds 64 levels".into());
    }
    xmlrpc_start(writer, "value")?;
    match value {
        serde_json::Value::Null => {
            writer
                .write_event(Event::Empty(BytesStart::new("nil")))
                .map_err(|error| error.to_string())?;
        }
        serde_json::Value::Bool(value) => {
            xmlrpc_text_element(writer, "boolean", if *value { "1" } else { "0" })?;
        }
        serde_json::Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                xmlrpc_text_element(writer, "int", &value.to_string())?;
            } else if let Some(value) = value.as_u64() {
                xmlrpc_text_element(writer, "i8", &value.to_string())?;
            } else if let Some(value) = value.as_f64().filter(|value| value.is_finite()) {
                xmlrpc_text_element(writer, "double", &value.to_string())?;
            } else {
                return Err("XML-RPC numbers must be finite".into());
            }
        }
        serde_json::Value::String(value) => xmlrpc_text_element(writer, "string", value)?,
        serde_json::Value::Array(values) => {
            xmlrpc_start(writer, "array")?;
            xmlrpc_start(writer, "data")?;
            for value in values {
                write_xmlrpc_value(writer, value, depth + 1)?;
            }
            xmlrpc_end(writer, "data")?;
            xmlrpc_end(writer, "array")?;
        }
        serde_json::Value::Object(values) => {
            if let Some(date) = values
                .get("__riinaXmlrpcDate")
                .and_then(serde_json::Value::as_str)
            {
                let date = chrono::DateTime::parse_from_rfc3339(date)
                    .map_err(|error| format!("Invalid XML-RPC date value: {error}"))?
                    .with_timezone(&chrono::Local)
                    .format("%Y%m%dT%H:%M:%S")
                    .to_string();
                xmlrpc_text_element(writer, "dateTime.iso8601", &date)?;
            } else if let Some(bytes) = values
                .get("__riinaXmlrpcBinary")
                .and_then(serde_json::Value::as_array)
            {
                let bytes = bytes
                    .iter()
                    .map(|value| {
                        value
                            .as_u64()
                            .filter(|value| *value <= 255)
                            .map(|value| value as u8)
                    })
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| "XML-RPC binary data must contain byte values".to_owned())?;
                let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
                xmlrpc_text_element(writer, "base64", &encoded)?;
            } else {
                xmlrpc_start(writer, "struct")?;
                for (name, member) in values {
                    xmlrpc_start(writer, "member")?;
                    xmlrpc_text_element(writer, "name", name)?;
                    write_xmlrpc_value(writer, member, depth + 1)?;
                    xmlrpc_end(writer, "member")?;
                }
                xmlrpc_end(writer, "struct")?;
            }
        }
    }
    xmlrpc_end(writer, "value")
}

fn parse_xmlrpc_value(
    value: roxmltree::Node<'_, '_>,
    depth: usize,
) -> Result<serde_json::Value, String> {
    use base64::Engine;

    if depth > 64 {
        return Err("XML-RPC value nesting exceeds 64 levels".into());
    }
    let Some(typed) = value.children().find(|node| node.is_element()) else {
        return Ok(serde_json::Value::String(
            value.text().unwrap_or_default().to_owned(),
        ));
    };
    let text = typed.text().unwrap_or_default().trim();
    match typed.tag_name().name() {
        "string" => Ok(serde_json::Value::String(
            typed.text().unwrap_or_default().to_owned(),
        )),
        "int" | "i4" | "i8" => text
            .parse::<i64>()
            .map(serde_json::Value::from)
            .map_err(|error| format!("Invalid XML-RPC integer: {error}")),
        "double" => text
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number)
            .ok_or_else(|| "Invalid XML-RPC double".to_owned()),
        "boolean" => match text {
            "1" | "true" => Ok(serde_json::Value::Bool(true)),
            "0" | "false" => Ok(serde_json::Value::Bool(false)),
            _ => Err("Invalid XML-RPC boolean".into()),
        },
        "dateTime.iso8601" => {
            use chrono::TimeZone;
            let date = chrono::NaiveDateTime::parse_from_str(text, "%Y%m%dT%H:%M:%S")
                .map_err(|error| format!("Invalid XML-RPC date: {error}"))?;
            let date = match chrono::Local.from_local_datetime(&date) {
                chrono::LocalResult::Single(date) | chrono::LocalResult::Ambiguous(date, _) => date,
                chrono::LocalResult::None => {
                    return Err("XML-RPC date does not exist in the local time zone".into());
                }
            }
            .to_rfc3339();
            Ok(serde_json::json!({ "__riinaXmlrpcDate": date }))
        }
        "base64" => {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(text)
                .map_err(|error| format!("Invalid XML-RPC base64 data: {error}"))?;
            Ok(serde_json::json!({ "__riinaXmlrpcBinary": bytes }))
        }
        "nil" => Ok(serde_json::Value::Null),
        "array" => {
            let data = typed
                .children()
                .find(|node| node.has_tag_name("data"))
                .ok_or_else(|| "XML-RPC array has no data element".to_owned())?;
            data.children()
                .filter(|node| node.has_tag_name("value"))
                .map(|value| parse_xmlrpc_value(value, depth + 1))
                .collect::<Result<Vec<_>, _>>()
                .map(serde_json::Value::Array)
        }
        "struct" => {
            let mut members = serde_json::Map::new();
            for member in typed.children().filter(|node| node.has_tag_name("member")) {
                let name = member
                    .children()
                    .find(|node| node.has_tag_name("name"))
                    .and_then(|node| node.text())
                    .ok_or_else(|| "XML-RPC struct member has no name".to_owned())?;
                let value = member
                    .children()
                    .find(|node| node.has_tag_name("value"))
                    .ok_or_else(|| "XML-RPC struct member has no value".to_owned())?;
                members.insert(name.to_owned(), parse_xmlrpc_value(value, depth + 1)?);
            }
            Ok(serde_json::Value::Object(members))
        }
        other => Err(format!("Unsupported XML-RPC value type: {other}")),
    }
}

fn http_option_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        value => value.to_string(),
    }
}

fn acquire_network_request_slot(active: &AtomicUsize) -> bool {
    let mut current = active.load(Ordering::Acquire);
    loop {
        if current >= MAX_ACTIVE_NETWORK_REQUESTS {
            return false;
        }
        match active.compare_exchange_weak(
            current,
            current + 1,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => return true,
            Err(updated) => current = updated,
        }
    }
}

fn timer_js_error(message: String) -> rquickjs::Error {
    rquickjs::Error::new_from_js_message("timer delay", "finite delay up to one day", message)
}

fn validate_plugin_message(name: &str, data: &serde_json::Value) -> Result<(), String> {
    if name.is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
        return Err("Plugin message name is invalid or exceeds 256 bytes".into());
    }
    let data_bytes = serde_json::to_vec(data)
        .map_err(|error| format!("Plugin message is not valid JSON: {error}"))?;
    if data_bytes.len() > MAX_PLUGIN_MESSAGE_BYTES {
        return Err("Plugin message exceeds the 1 MiB limit".into());
    }
    Ok(())
}

fn validate_player_event_name(name: &str) -> Result<(), String> {
    let parts = name.split('.').collect::<Vec<_>>();
    let valid_shape = parts.len() == 2 || (parts.len() == 3 && parts[2] == "changed");
    if !valid_shape
        || !matches!(parts[0], "iina" | "mpv")
        || parts[1].is_empty()
        || parts.iter().any(|part| part.is_empty())
        || name.len() > 256
        || name.chars().any(char::is_control)
    {
        return Err(format!("Incorrect event name syntax: \"{name}\""));
    }
    Ok(())
}

fn mpv_property_for_changed_event(name: &str) -> Option<&str> {
    let property = name.strip_prefix("mpv.")?.strip_suffix(".changed")?;
    (!property.is_empty() && !property.contains('.')).then_some(property)
}

fn player_snapshot_mpv_property(name: &str) -> bool {
    matches!(
        name,
        "pause"
            | "idle-active"
            | "time-pos"
            | "duration"
            | "speed"
            | "width"
            | "height"
            | "media-title"
            | "aid"
            | "audio-delay"
            | "volume"
            | "mute"
            | "sid"
            | "secondary-sid"
            | "sub-delay"
    )
}

fn decode_plugin_json(text: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(text).map_err(|error| format!("Plugin message is not valid JSON: {error}"))
}

fn record_plugin_message(
    events: &Mutex<Vec<HostEvent>>,
    name: String,
    data: serde_json::Value,
) -> Result<(), String> {
    validate_plugin_message(&name, &data)?;
    let mut events = events
        .lock()
        .map_err(|_| "Plugin event queue was poisoned".to_owned())?;
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

fn plugin_message_js_error(message: String) -> rquickjs::Error {
    rquickjs::Error::new_from_js_message("plugin message", "valid bounded message", message)
}

#[derive(Clone, Copy, Default)]
struct AvailablePermissions {
    show_osd: bool,
    show_alert: bool,
    video_overlay: bool,
    network_request: bool,
    file_system: bool,
}

fn reviewed_permissions(
    requested: &[String],
    granted: &[String],
) -> Result<AvailablePermissions, String> {
    let mut available = AvailablePermissions::default();
    for permission in requested {
        if !granted.iter().any(|grant| grant == permission) {
            return Err(format!(
                "Plugin permission {permission} has not been granted"
            ));
        }
        match permission.as_str() {
            "show-osd" => available.show_osd = true,
            "show-alert" => available.show_alert = true,
            "video-overlay" => available.video_overlay = true,
            "network-request" => available.network_request = true,
            "file-system" => available.file_system = true,
            _ => return Err(format!("Unknown plugin permission {permission}")),
        }
    }
    Ok(available)
}

fn record_overlay(
    events: &Mutex<Vec<HostEvent>>,
    state: &Mutex<OverlayState>,
    intent: OverlayIntent,
) -> Result<(), String> {
    let effects = state
        .lock()
        .map_err(|_| "Plugin overlay state was poisoned".to_owned())?
        .apply(intent)?;
    let Ok(mut events) = events.lock() else {
        return Err("Plugin event queue was poisoned".into());
    };
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

fn host_event_size(event: &HostEvent) -> usize {
    match event {
        HostEvent::Log(message) | HostEvent::ShowOsd(message) | HostEvent::ShowAlert(message) => {
            message.len()
        }
        HostEvent::Overlay(effect) => overlay_effect_size(effect),
        HostEvent::PluginMessage(name, data) => name
            .len()
            .saturating_add(serde_json::to_vec(data).map_or(usize::MAX, |bytes| bytes.len())),
        HostEvent::CoreCommand(command) => format!("{command:?}").len(),
        HostEvent::SetUiVisibility(_) => 1,
        HostEvent::WindowAction(action) => format!("{action:?}").len(),
    }
}

fn overlay_js_error(message: String) -> rquickjs::Error {
    rquickjs::Error::new_from_js_message("overlay request", "valid overlay effect", message)
}

fn record(events: &Mutex<Vec<HostEvent>>, event: HostEvent) {
    let Ok(mut events) = events.lock() else {
        return;
    };
    if events.len() >= MAX_HOST_EVENTS {
        return;
    }
    if matches!(
        &event,
        HostEvent::CoreCommand(_) | HostEvent::SetUiVisibility(_) | HostEvent::WindowAction(_)
    ) {
        events.push(event);
        return;
    }
    let message = match &event {
        HostEvent::Log(message) | HostEvent::ShowOsd(message) | HostEvent::ShowAlert(message) => {
            message
        }
        HostEvent::Overlay(_)
        | HostEvent::PluginMessage(_, _)
        | HostEvent::CoreCommand(_)
        | HostEvent::SetUiVisibility(_)
        | HostEvent::WindowAction(_) => {
            return;
        }
    };
    let bounded = message.chars().take(MAX_MESSAGE_CHARS).collect::<String>();
    let event = match event {
        HostEvent::Log(_) => HostEvent::Log(bounded),
        HostEvent::ShowOsd(_) => HostEvent::ShowOsd(bounded),
        HostEvent::ShowAlert(_) => HostEvent::ShowAlert(bounded),
        HostEvent::Overlay(effect) => HostEvent::Overlay(effect),
        HostEvent::PluginMessage(_, _) => unreachable!("plugin messages use their bounded path"),
        HostEvent::CoreCommand(_) | HostEvent::SetUiVisibility(_) | HostEvent::WindowAction(_) => {
            unreachable!("player commands retain typed payloads")
        }
    };
    events.push(event);
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_core::plugin::Author;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    fn temporary_directory() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "riina-plugin-runtime-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn manifest(entry: PathBuf) -> Manifest {
        Manifest {
            name: "Example plugin".into(),
            identifier: "org.example.player-tools".into(),
            version: "1.0.0".into(),
            author: Author {
                name: "Example".into(),
                email: None,
                url: None,
            },
            description: None,
            entry,
            global_entry: None,
            permissions: Vec::new(),
            allowed_domains: Vec::new(),
            github_repo: None,
            github_version: None,
        }
    }

    #[test]
    fn exposes_only_the_host_apis_requested_and_granted_by_the_plugin() {
        let requested = vec!["show-osd".to_owned(), "show-alert".to_owned()];
        let events = execute_entry(
            r#"iina.log("ready"); iina.showOSD("Loaded"); iina.showAlert("Notice"); console.warn("done");"#,
            "org.example.player-tools",
            &requested,
            &requested,
        )
        .unwrap();

        assert_eq!(
            events,
            [
                HostEvent::Log("ready".into()),
                HostEvent::ShowOsd("Loaded".into()),
                HostEvent::ShowAlert("Notice".into()),
                HostEvent::Log("done".into()),
            ]
        );
        assert!(
            execute_entry(
                "iina.showOSD('not granted')",
                "org.example.player-tools",
                &[],
                &[],
            )
            .is_err()
        );
    }

    #[test]
    fn refuses_ungranted_or_unsupported_capabilities_before_execution() {
        let requested = vec!["show-osd".to_owned()];
        assert!(
            execute_entry(
                "throw new Error('must not run')",
                "org.example.player-tools",
                &requested,
                &[],
            )
            .is_err()
        );
        assert!(
            execute_entry(
                "iina.overlay.loadFile('overlay.html')",
                "org.example.player-tools",
                &[],
                &[]
            )
            .is_err()
        );
        assert!(
            execute_entry(
                "iina.overlay.simpleMode()",
                "org.example.player-tools",
                &[],
                &[]
            )
            .is_err()
        );
        assert!(
            execute_entry("iina.overlay.show()", "org.example.player-tools", &[], &[])
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn permitted_overlay_calls_become_validated_platform_effects() {
        use riina_core::plugin::OverlayEffect;
        let permissions = vec!["video-overlay".to_owned()];
        let events = execute_entry(
            "iina.overlay.show(); iina.overlay.loadFile('ui/overlay.html'); iina.overlay.setClickable(true); iina.overlay.show(); iina.overlay.setOpacity(2); iina.overlay.postMessage('ready', {version: 2});",
            "org.example.player-tools",
            &permissions,
            &permissions,
        )
        .unwrap();

        assert_eq!(
            events,
            [
                HostEvent::Overlay(OverlayEffect::LoadPluginFile("ui/overlay.html".into())),
                HostEvent::Overlay(OverlayEffect::SetClickable(true)),
                HostEvent::Overlay(OverlayEffect::Show),
                HostEvent::Overlay(OverlayEffect::SetOpacity(1.0)),
                HostEvent::Overlay(OverlayEffect::PostMessage {
                    name: "ready".into(),
                    data: serde_json::json!({"version": 2}),
                }),
            ]
        );
        assert!(
            execute_entry(
                "iina.overlay.loadFile('../outside.html')",
                "org.example.player-tools",
                &permissions,
                &permissions,
            )
            .is_err()
        );
    }

    #[test]
    fn plugin_file_api_scopes_private_paths_and_checks_external_access_grants() {
        let identifier = format!("org.example.files-{}", std::process::id());
        let data_root = std::env::temp_dir()
            .join("riina-plugin-storage")
            .join(".data")
            .join(&identifier);
        let _ = std::fs::remove_dir_all(&data_root);
        let requested = vec!["file-system".to_owned()];
        let source = r#"
            iina.file.write('@data/state.txt', 'saved');
            iina.file.move('@data/state.txt', '@data/moved.txt');
            if (iina.file.exists('@data/state.txt')) throw new Error('source remains after move');
            if (!iina.file.exists('@data/moved.txt')) throw new Error('missing moved file');
            if (iina.file.read('@data/moved.txt') !== 'saved') throw new Error('read mismatch');
            if (!iina.file.list('@data/').some(entry => entry.filename === 'moved.txt')) {
                throw new Error('listing mismatch');
            }
            iina.log(iina.file.read('@data/moved.txt'));
            iina.file.delete('@data/moved.txt');
        "#;
        let events = execute_entry(source, &identifier, &requested, &requested).unwrap();
        assert_eq!(events, [HostEvent::Log("saved".into())]);
        let _ = std::fs::remove_dir_all(data_root);

        assert!(
            execute_entry(
                "iina.file.read('C:/Windows/win.ini')",
                &identifier,
                &[],
                &[],
            )
            .is_err()
        );
        assert!(
            execute_entry(
                "iina.file.showInFinder('C:/Windows/win.ini')",
                &identifier,
                &[],
                &[],
            )
            .is_err()
        );
        execute_entry(
            r#"try {
                iina.file.trash('@current/movie.mkv');
                throw new Error('ungranted trash succeeded');
            } catch (error) {
                if (!String(error).includes('file-system permission')) throw error;
            }"#,
            &identifier,
            &[],
            &[],
        )
        .unwrap();
    }

    #[test]
    fn plugin_file_handles_expose_binary_seek_read_write_and_close() {
        let identifier = format!("org.example.file-handles-{}", std::process::id());
        let data_root = std::env::temp_dir()
            .join("riina-plugin-storage")
            .join(".data")
            .join(&identifier);
        let _ = std::fs::remove_dir_all(&data_root);
        let requested = vec!["file-system".to_owned()];
        let source = r#"
            iina.file.write('@data/binary.dat', 'abcd');
            const writer = iina.file.handle('@data/binary.dat', 'write');
            writer.write(new Uint8Array([0, 255]));
            if (writer.offset() !== 2) throw new Error('write offset mismatch');
            writer.close();
            const reader = iina.file.handle('@data/binary.dat', 'read');
            const prefix = reader.read(2);
            if (prefix[0] !== 0 || prefix[1] !== 255) throw new Error('binary read mismatch');
            if (reader.offset() !== 2) throw new Error('read offset mismatch');
            reader.seekToEnd();
            if (reader.offset() !== 4) throw new Error('seek-to-end mismatch');
            reader.seekTo(0);
            const content = reader.readToEnd();
            if (content.length !== 4 || content[2] !== 99 || content[3] !== 100) {
                throw new Error('read-to-end mismatch');
            }
            reader.close();
        "#;
        execute_entry(source, &identifier, &requested, &requested).unwrap();
        assert_eq!(
            std::fs::read(data_root.join("binary.dat")).unwrap(),
            [0, 255, b'c', b'd']
        );
        let _ = std::fs::remove_dir_all(data_root);
    }

    #[test]
    fn core_playback_methods_emit_typed_player_commands() {
        let events = execute_entry(
            "iina.core.pause(); iina.core.resume(); iina.core.stop(); iina.core.seek(-4.5, true); iina.core.seekTo(12); iina.core.setSpeed(1.25); iina.core.playChapter(2);",
            "org.example.core-api",
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(
            events,
            [
                HostEvent::CoreCommand(Command::SetPaused(true)),
                HostEvent::CoreCommand(Command::SetPaused(false)),
                HostEvent::CoreCommand(Command::Stop),
                HostEvent::CoreCommand(Command::SeekWithType {
                    seconds: -4.5,
                    seek_type: SeekType::Exact,
                }),
                HostEvent::CoreCommand(Command::Seek {
                    seconds: 12.0,
                    absolute: true,
                }),
                HostEvent::CoreCommand(Command::Speed(1.25)),
                HostEvent::CoreCommand(Command::SelectChapter(2)),
            ]
        );
        assert!(
            execute_entry(
                "iina.core.seek(NaN, false)",
                "org.example.core-api",
                &[],
                &[]
            )
            .is_err()
        );
        assert!(execute_entry("iina.core.setSpeed(0)", "org.example.core-api", &[], &[]).is_err());
    }

    #[test]
    fn core_ui_visibility_emits_the_iina_disable_ui_flag() {
        let events = execute_entry(
            "iina.core.setUIVisibility(true); iina.core.setUIVisibility(false);",
            "org.example.core-ui-visibility",
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(
            events,
            [
                HostEvent::SetUiVisibility(true),
                HostEvent::SetUiVisibility(false),
            ]
        );
    }

    #[test]
    fn core_window_proxy_exposes_window_state_and_typed_mutations() {
        let events = execute_entry(
            r#"
                iina.log(String(iina.core.window.loaded));
                iina.log(String(iina.core.window.frame));
                iina.core.window.frame = {x: 12, y: 34, width: 640, height: 360};
                iina.core.window.fullscreen = true;
                iina.core.window.pip = false;
                iina.core.window.ontop = true;
                iina.core.window.sidebar = 'playlist';
                iina.core.window.sidebar = null;
                iina.core.window.miniaturized = false;
            "#,
            "org.example.core-window",
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(
            events,
            [
                HostEvent::Log("false".into()),
                HostEvent::Log("null".into()),
                HostEvent::WindowAction(WindowAction::SetFrame {
                    x: 12.0,
                    y: 34.0,
                    width: 640.0,
                    height: 360.0,
                }),
                HostEvent::WindowAction(WindowAction::SetFullscreen(true)),
                HostEvent::WindowAction(WindowAction::SetPictureInPicture(false)),
                HostEvent::WindowAction(WindowAction::SetOnTop(true)),
                HostEvent::WindowAction(WindowAction::SetSidebar(Some("playlist".into()))),
                HostEvent::WindowAction(WindowAction::SetSidebar(None)),
                HostEvent::WindowAction(WindowAction::SetMiniaturized(false)),
            ]
        );
        assert!(
            execute_entry(
                "iina.core.window.frame = {x: 0, y: 0, width: Infinity, height: 10};",
                "org.example.core-window-invalid",
                &[],
                &[],
            )
            .is_err()
        );
        assert!(
            execute_entry(
                "iina.core.window.fullscreen = 'yes';",
                "org.example.core-window-invalid-type",
                &[],
                &[],
            )
            .is_err()
        );
    }

    #[test]
    fn core_track_apis_emit_typed_commands() {
        let events = execute_entry(
            "iina.core.audio.id = 4; iina.core.audio.id = null; iina.core.audio.delay = 0.25; iina.core.audio.volume = 125; iina.core.audio.muted = true; iina.core.subtitle.id = 5; iina.core.subtitle.secondID = 6; iina.core.subtitle.secondID = null; iina.core.subtitle.delay = -0.5; iina.core.video.id = 7;",
            "org.example.core-track-api",
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(
            events,
            [
                HostEvent::CoreCommand(Command::SelectTrack {
                    kind: riina_core::TrackKind::Audio,
                    id: Some(4),
                }),
                HostEvent::CoreCommand(Command::AudioDelay(0.25)),
                HostEvent::CoreCommand(Command::Volume(125.0)),
                HostEvent::CoreCommand(Command::SetMuted(true)),
                HostEvent::CoreCommand(Command::SelectTrack {
                    kind: riina_core::TrackKind::Sub,
                    id: Some(5),
                }),
                HostEvent::CoreCommand(Command::SelectSecondarySubtitle(Some(6))),
                HostEvent::CoreCommand(Command::SubtitleDelay(-0.5)),
                HostEvent::CoreCommand(Command::SelectTrack {
                    kind: riina_core::TrackKind::Video,
                    id: Some(7),
                }),
            ]
        );
        assert!(
            execute_entry(
                "iina.core.audio.volume = 201;",
                "org.example.core-track-api",
                &[],
                &[],
            )
            .is_err()
        );
        assert!(
            execute_entry(
                "iina.core.audio.loadTrack('https://media.example/commentary.m4a');",
                "org.example.core-track-api",
                &[],
                &[],
            )
            .is_err()
        );
    }

    #[test]
    fn core_status_chapter_recent_document_and_version_reads_follow_playback_context() {
        let identifier = "org.example.core-snapshot";
        let files = PluginFiles::temporary(identifier);
        files.set_player_context(
            PluginPlayerFileContext::new(None, BTreeMap::new())
                .with_core_snapshot(
                    BTreeMap::from([
                        ("paused".into(), serde_json::json!(true)),
                        ("position".into(), serde_json::json!(17.25)),
                        ("title".into(), serde_json::json!("Sample")),
                    ]),
                    vec![serde_json::json!({"title":"Opening", "start":0.0})],
                )
                .with_core_version(serde_json::json!({
                    "iina": "0.3.1",
                    "build": "1234",
                    "mpv": "mpv 0.41.0-git"
                }))
                .with_recent_documents(vec![serde_json::json!({
                    "name": "sample.mp4",
                    "url": "file:///Media/sample.mp4"
                })])
                .with_playback_history(vec![serde_json::json!({
                    "name": "sample.mp4",
                    "url": "file:///Media/sample.mp4",
                    "date": 1_750_000_000_000_u64,
                    "progress": 12.5,
                    "duration": 80.0
                })]),
        );
        let script = r#"
            if (iina.core.status.paused !== true) throw new Error('pause status mismatch');
            if (iina.core.status.position !== 17.25) throw new Error('position mismatch');
            if (iina.core.status.title !== 'Sample') throw new Error('title mismatch');
            if (iina.core.status.unknown !== undefined) throw new Error('unknown status should be undefined');
            const chapters = iina.core.getChapters();
            if (chapters.length !== 1 || chapters[0].title !== 'Opening' || chapters[0].start !== 0) {
                throw new Error('chapter snapshot mismatch');
            }
            const recent = iina.core.getRecentDocuments();
            if (recent.length !== 1 || recent[0].name !== 'sample.mp4' || recent[0].url !== 'file:///Media/sample.mp4') {
                throw new Error('recent-document snapshot mismatch');
            }
            const history = iina.core.getHistory();
            if (history.length !== 1 || history[0].name !== 'sample.mp4' ||
                history[0].url !== 'file:///Media/sample.mp4' ||
                !(history[0].date instanceof Date) || history[0].date.getTime() !== 1750000000000 ||
                history[0].progress !== 12.5 || history[0].duration !== 80) {
                throw new Error('playback-history snapshot mismatch');
            }
            const version = iina.core.getVersion();
            if (version.iina !== '0.3.1' || version.build !== '1234' || version.mpv !== 'mpv 0.41.0-git') {
                throw new Error('version snapshot mismatch');
            }
        "#;
        let session = PluginRuntimeSession::start_entry_with_context(
            script,
            identifier,
            &[],
            &[],
            GlobalApiMode::None,
            None,
            PluginStorageContext {
                file_system: files,
                allowed_domains: Vec::new(),
            },
        )
        .unwrap();
        assert!(session.drain_events().unwrap().is_empty());
    }

    #[test]
    fn core_open_requires_the_matching_network_or_filesystem_grant() {
        let identifier = format!("org.example.core-open-{}", std::process::id());
        let files = PluginFiles::temporary(&identifier);
        files
            .write_text("@data/clip.mkv", "fixture", false)
            .unwrap();
        let path = files
            .existing_path("@data/clip.mkv", false)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert_eq!(
            execute_entry("iina.core.open('@data/clip.mkv')", &identifier, &[], &[],).unwrap(),
            [HostEvent::CoreCommand(Command::Load(path))]
        );
        let _ = std::fs::remove_dir_all(
            std::env::temp_dir()
                .join("riina-plugin-storage")
                .join(".data")
                .join(&identifier),
        );
        let files = PluginFiles::temporary("org.example.core-open-policy");
        assert!(
            plugin_media_reference(
                "https://media.example/video.mp4",
                &files,
                false,
                false,
                &["media.example".into()],
            )
            .is_err()
        );
        assert_eq!(
            plugin_media_reference(
                "https://media.example/video.mp4",
                &files,
                false,
                true,
                &["media.example".into()],
            )
            .unwrap(),
            "https://media.example/video.mp4"
        );
        assert_eq!(
            plugin_media_reference(
                "smb://media.example/share/video.mkv",
                &files,
                false,
                true,
                &["media.example".into()],
            )
            .unwrap(),
            "smb://media.example/share/video.mkv"
        );
        assert!(
            plugin_media_reference(
                "rtsp://media.example/live",
                &files,
                false,
                false,
                &["media.example".into()],
            )
            .is_err()
        );
        assert_eq!(
            plugin_media_reference(
                "rtsp://media.example/live",
                &files,
                false,
                true,
                &["media.example".into()],
            )
            .unwrap(),
            "rtsp://media.example/live"
        );
        assert!(
            plugin_media_reference(
                "rtmp://media.example/live/channel",
                &files,
                true,
                false,
                &["media.example".into()],
            )
            .is_err()
        );
        assert_eq!(
            plugin_media_reference(
                "rtmp://media.example/live/channel",
                &files,
                false,
                true,
                &["media.example".into()],
            )
            .unwrap(),
            "rtmp://media.example/live/channel"
        );
        assert_eq!(
            plugin_media_reference(
                "ytdl://https://media.example/watch?id=42",
                &files,
                false,
                true,
                &["media.example".into()],
            )
            .unwrap(),
            "ytdl://https://media.example/watch?id=42"
        );
        assert!(
            plugin_media_reference(
                "ytdl://https://media.example/watch?id=42",
                &files,
                false,
                true,
                &["elsewhere.example".into()],
            )
            .is_err()
        );
        assert!(
            plugin_media_reference(
                "https://media.example/video.mp4",
                &files,
                false,
                true,
                &["elsewhere.example".into()],
            )
            .is_err()
        );
        assert!(
            plugin_media_reference("C:/private/video.mp4", &files, false, false, &[],).is_err()
        );
    }

    #[test]
    fn permission_gated_http_transport_returns_json_from_an_allowed_host() {
        use std::{io::Write, net::TcpListener};

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = [0; 4096];
            let _ = stream.read(&mut request).unwrap();
            let body = r#"{"ready":true}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });
        let result = perform_http_request(
            "GET",
            &format!("http://{address}/health"),
            &serde_json::json!({}),
            &["127.0.0.1".into()],
        )
        .unwrap();
        server.join().unwrap();
        assert_eq!(result["statusCode"], 200);
        assert_eq!(result["data"]["ready"], true);
        assert_eq!(result["text"], r#"{"ready":true}"#);
    }

    #[test]
    fn simple_mode_style_and_content_are_forwarded_after_activation() {
        use riina_core::plugin::OverlayEffect;
        let permissions = vec!["video-overlay".to_owned()];
        let events = execute_entry(
            "iina.overlay.setStyle('ignored before mode'); iina.overlay.simpleMode(); iina.overlay.setStyle('body { color: white; }'); iina.overlay.setContent('<b>Ready</b>');",
            "org.example.player-tools",
            &permissions,
            &permissions,
        )
        .unwrap();

        assert_eq!(
            events,
            [
                HostEvent::Overlay(OverlayEffect::EnterSimpleMode),
                HostEvent::Overlay(OverlayEffect::SetStyle("body { color: white; }".into())),
                HostEvent::Overlay(OverlayEffect::SetContent("<b>Ready</b>".into())),
            ]
        );
    }

    #[test]
    fn overlay_listener_registration_keeps_the_source_api_shape() {
        use riina_core::plugin::OverlayEffect;
        let permissions = vec!["video-overlay".to_owned()];
        let events = execute_entry(
            "iina.overlay.simpleMode(); iina.overlay.onMessage('reply', () => {});",
            "org.example.player-tools",
            &permissions,
            &permissions,
        )
        .unwrap();
        assert_eq!(
            events,
            [
                HostEvent::Overlay(OverlayEffect::EnterSimpleMode),
                HostEvent::Overlay(OverlayEffect::ListenMessage("reply".into())),
            ]
        );
    }

    #[test]
    fn persistent_session_delivers_overlay_messages_to_registered_callbacks() {
        let permissions = vec!["video-overlay".to_owned()];
        let session = PluginRuntimeSession::start_entry(
            "iina.overlay.simpleMode(); iina.overlay.onMessage('reply', value => iina.log(value.answer));",
            "org.example.player-tools",
            &permissions,
            &permissions,
        )
        .unwrap();
        assert!(session.drain_events().unwrap().iter().any(|event| {
            matches!(event, HostEvent::Overlay(OverlayEffect::ListenMessage(name)) if name == "reply")
        }));
        session
            .deliver_overlay_message("unregistered".into(), serde_json::json!(null))
            .unwrap();
        session
            .deliver_overlay_message("reply".into(), serde_json::json!({"answer": "received"}))
            .unwrap();

        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            let events = session.drain_events().unwrap();
            if events
                .iter()
                .any(|event| matches!(event, HostEvent::Log(message) if message == "received"))
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "plugin callback was not delivered"
            );
            std::thread::yield_now();
        }
    }

    #[test]
    fn persistent_session_delivers_player_events_and_unregisters_listeners() {
        let session = PluginRuntimeSession::start_entry(
            "iina.event.on('iina.file-loaded', url => iina.log(url)); iina.event.on('mpv.pause.changed', value => iina.log(String(value))); iina.event.on('mpv.cache-buffering-state.changed', value => iina.log(String(value)));",
            "org.example.player-events",
            &[],
            &[],
        )
        .unwrap();
        assert!(session.is_subscribed_to_player_event("iina.file-loaded"));
        assert!(session.is_subscribed_to_player_event("mpv.pause.changed"));
        assert!(session.is_subscribed_to_player_event("mpv.cache-buffering-state.changed"));
        assert_eq!(
            session.take_mpv_property_observation_requests(),
            ["cache-buffering-state"]
        );
        assert!(session.take_mpv_property_observation_requests().is_empty());
        session
            .deliver_player_event(
                "iina.file-loaded".into(),
                serde_json::json!("file:///media/example.mkv"),
            )
            .unwrap();
        session
            .deliver_player_event("mpv.pause.changed".into(), serde_json::json!(true))
            .unwrap();

        let deadline = Instant::now() + Duration::from_secs(1);
        let mut seen = Vec::new();
        loop {
            seen.extend(session.drain_events().unwrap());
            let file_loaded = seen.iter().any(|event| {
                matches!(event, HostEvent::Log(message) if message == "file:///media/example.mkv")
            });
            let paused = seen
                .iter()
                .any(|event| matches!(event, HostEvent::Log(message) if message == "true"));
            if file_loaded && paused {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "player event callback was not delivered: {seen:?}"
            );
            std::thread::yield_now();
        }

        let removed = PluginRuntimeSession::start_entry(
            "const listener = iina.event.on('iina.file-loaded', url => iina.log(url)); iina.event.off('iina.file-loaded', listener);",
            "org.example.player-events-off",
            &[],
            &[],
        )
        .unwrap();
        assert!(!removed.is_subscribed_to_player_event("iina.file-loaded"));
        removed
            .deliver_player_event(
                "iina.file-loaded".into(),
                serde_json::json!("file:///media/ignored.mkv"),
            )
            .unwrap();
        assert!(removed.drain_events().unwrap().is_empty());
    }

    #[test]
    fn player_event_listener_rejects_invalid_names() {
        assert!(
            execute_entry(
                "iina.event.on('mpv..changed', () => {});",
                "org.example.invalid-player-event",
                &[],
                &[],
            )
            .is_err()
        );
    }

    #[test]
    fn overlay_host_output_is_bounded_across_repeated_large_calls() {
        let permissions = vec!["video-overlay".to_owned()];
        let events = execute_entry(
            "iina.overlay.simpleMode(); const payload = 'x'.repeat(1048576); for (let i = 0; i < 8; i++) iina.overlay.setContent(payload);",
            "org.example.player-tools",
            &permissions,
            &permissions,
        )
        .unwrap();
        let output_bytes = events
            .iter()
            .filter_map(|event| match event {
                HostEvent::Overlay(OverlayEffect::SetContent(content)) => Some(content.len()),
                _ => None,
            })
            .sum::<usize>();
        assert!(output_bytes <= MAX_HOST_OUTPUT_BYTES);
        assert!(output_bytes < 8 * 1024 * 1024);
    }

    #[test]
    fn interrupts_unbounded_entry_code() {
        let started = Instant::now();
        let result = execute_entry("while (true) {}", "org.example.player-tools", &[], &[]);
        assert!(result.is_err());
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn bounds_source_and_host_output() {
        let too_large = " ".repeat(MAX_ENTRY_BYTES + 1);
        assert!(execute_entry(&too_large, "org.example.player-tools", &[], &[]).is_err());

        let many_logs = (0..MAX_HOST_EVENTS + 10)
            .map(|index| format!("iina.log('{index}')"))
            .collect::<Vec<_>>()
            .join(";");
        let events = execute_entry(&many_logs, "org.example.player-tools", &[], &[]).unwrap();
        assert_eq!(events.len(), MAX_HOST_EVENTS);
    }

    #[test]
    fn package_entry_must_resolve_inside_its_installed_root() {
        let root = temporary_directory();
        std::fs::write(root.join("entry.js"), "iina.log('inside')").unwrap();
        let events = execute_package_entry(&root, &manifest("entry.js".into()), &[]).unwrap();
        assert_eq!(events, [HostEvent::Log("inside".into())]);

        let sibling_entry = root.with_extension("js");
        std::fs::write(&sibling_entry, "iina.log('outside')").unwrap();
        let traversal_entry = PathBuf::from("..").join(
            sibling_entry
                .file_name()
                .expect("temporary file has a name"),
        );
        assert!(execute_package_entry(&root, &manifest(traversal_entry), &[]).is_err());

        let _ = std::fs::remove_file(sibling_entry);
        let _ = std::fs::remove_dir_all(root);
    }
}
