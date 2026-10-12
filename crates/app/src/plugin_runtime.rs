//! Bounded JavaScript entry evaluation for permission-reviewed plugins.

use riina_core::plugin::{Manifest, OverlayEffect, OverlayIntent, OverlayState};
use rquickjs::{Context, Ctx, Function, Object, Runtime, Value, function::Func};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
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

#[derive(Debug, Clone, PartialEq)]
pub enum HostEvent {
    Log(String),
    ShowOsd(String),
    ShowAlert(String),
    Overlay(OverlayEffect),
    PluginMessage(String, serde_json::Value),
}

enum RuntimeCommand {
    DeliverOverlayMessage(String, serde_json::Value),
    DeliverPluginMessage(String, serde_json::Value),
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
    events: Arc<Mutex<Vec<HostEvent>>>,
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
    worker: Option<JoinHandle<()>>,
}

impl PluginRuntimeSession {
    pub fn start_package_entry(
        package_root: &Path,
        manifest: &Manifest,
        granted_permissions: &[String],
    ) -> Result<Self, String> {
        let (source, module_context) = read_package_script(package_root, &manifest.entry)?;
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
        )
    }

    fn start_entry_with_context(
        source: &str,
        plugin_identifier: &str,
        requested_permissions: &[String],
        granted_permissions: &[String],
        global_api_mode: GlobalApiMode,
        module_context: Option<PluginModuleContext>,
    ) -> Result<Self, String> {
        if source.len() > MAX_ENTRY_BYTES {
            return Err("Plugin entry exceeds the 1 MiB execution limit".into());
        }
        let permissions = reviewed_permissions(requested_permissions, granted_permissions)?;
        let (commands, receiver) = mpsc::sync_channel(32);
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let events = Arc::new(Mutex::new(Vec::new()));
        let worker_events = events.clone();
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
                        events: worker_events,
                    },
                    receiver,
                    ready_sender,
                );
            })
            .map_err(|error| format!("Could not start plugin worker: {error}"))?;
        match ready_receiver.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(())) => Ok(Self {
                commands,
                events,
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
) {
    let PluginWorkerHost {
        global_api_mode,
        module_context,
        events,
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

#[derive(Default)]
struct AvailablePermissions {
    show_osd: bool,
    show_alert: bool,
    video_overlay: bool,
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
            "network-request" | "file-system" => {
                return Err(format!(
                    "Plugin permission {permission} is not available yet"
                ));
            }
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
    let message = match &event {
        HostEvent::Log(message) | HostEvent::ShowOsd(message) | HostEvent::ShowAlert(message) => {
            message
        }
        HostEvent::Overlay(_) | HostEvent::PluginMessage(_, _) => return,
    };
    let bounded = message.chars().take(MAX_MESSAGE_CHARS).collect::<String>();
    let event = match event {
        HostEvent::Log(_) => HostEvent::Log(bounded),
        HostEvent::ShowOsd(_) => HostEvent::ShowOsd(bounded),
        HostEvent::ShowAlert(_) => HostEvent::ShowAlert(bounded),
        HostEvent::Overlay(effect) => HostEvent::Overlay(effect),
        HostEvent::PluginMessage(_, _) => unreachable!("plugin messages use their bounded path"),
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
    fn refuses_unimplemented_or_ungranted_capabilities_before_execution() {
        for permission in ["network-request", "file-system"] {
            let requested = vec![permission.to_owned()];
            assert!(
                execute_entry(
                    "throw new Error('must not run')",
                    "org.example.player-tools",
                    &requested,
                    &requested,
                )
                .is_err()
            );
        }
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
