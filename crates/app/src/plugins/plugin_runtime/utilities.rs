//! Utility capabilities shared by player and global plugin entries.
mod dialogs;
mod paths;
mod process;
mod process_platform;
#[cfg(test)]
mod tests;

use super::{AvailablePermissions, ExecutionControl, RuntimeCommand};
use crate::{platform, plugin_files::PluginFiles};
pub(crate) use dialogs::{
    DialogRequest, FileRequest, InteractionKind, InteractionRequest, PluginDialogs,
};
use process::{Channel, Completed, Invocation};
use rquickjs::{Context, Ctx, Function, Object, Value, function::Func};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc::{SyncSender, TrySendError},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

const BINARY_NOT_FOUND: i32 = -1;
const RUNTIME_ERROR: i32 = -2;
const MAX_HELPERS: usize = 2;
const MAX_ARGUMENT_BYTES: usize = 24 * 1024;
const HELPER_TIMEOUT: Duration = Duration::from_secs(120);
const BACKPRESSURE_RETRY: Duration = Duration::from_millis(12);

pub(super) fn install<'js>(
    api: &Object<'js>,
    files: &PluginFiles,
    permissions: AvailablePermissions,
    execution: Arc<ExecutionControl>,
    commands: SyncSender<RuntimeCommand>,
    dialogs: PluginDialogs,
    workers: Workers,
) -> rquickjs::Result<()> {
    let ctx = api.ctx();
    let native = Object::new(ctx.clone())?;
    install_file_queries(&native, files, permissions)?;
    install_credentials(&native, files)?;
    install_dialogs(&native, dialogs, execution.clone(), commands.clone())?;
    native.set(
        "languages",
        Func::from(|| sys_locale::get_locales().take(64).collect::<Vec<_>>()),
    )?;
    native.set(
        "canExecute",
        permissions.file_system && permissions.process_execution,
    )?;
    let worker = Helpers {
        files: files.clone(),
        execution,
        commands,
        active: Arc::default(),
        workers,
    };
    native.set(
        "exec",
        Func::from(
            move |id: u32, name: String, args: Vec<String>, cwd: Option<String>, hooks: u8| {
                if !permissions.file_system || !permissions.process_execution {
                    return Err(js_error(
                        "Launching helpers requires file-system and process-execution",
                    ));
                }
                worker.start(id, name, args, cwd, hooks).map_err(js_error)
            },
        ),
    )?;
    let setup: Function<'_> = ctx.eval(include_str!("utilities/api.js"))?;
    let deliver: Function<'_> =
        setup.call((api.clone(), native, BINARY_NOT_FOUND, RUNTIME_ERROR))?;
    ctx.globals().set("__riinaUtilityResult", deliver)
}

fn install_dialogs(
    native: &Object<'_>,
    dialogs: PluginDialogs,
    execution: Arc<ExecutionControl>,
    commands: SyncSender<RuntimeCommand>,
) -> rquickjs::Result<()> {
    for (method, kind) in [
        ("ask", InteractionKind::Confirm),
        ("prompt", InteractionKind::Prompt),
    ] {
        let dialogs = dialogs.clone();
        let execution = execution.clone();
        native.set(
            method,
            Func::from(move |title: String| {
                dialogs.interact(title, kind, &execution).map_err(js_error)
            }),
        )?;
    }
    native.set(
        "chooseFile",
        Func::from(move |id: u32, title: String, options: String| {
            dialogs
                .open(id, title, &options, execution.clone(), commands.clone())
                .map_err(js_error)
        }),
    )
}

fn install_file_queries(
    native: &Object<'_>,
    files: &PluginFiles,
    permissions: AvailablePermissions,
) -> rquickjs::Result<()> {
    let lookup = files.clone();
    native.set(
        "fileInPath",
        Func::from(move |name: String| {
            permissions.file_system && paths::binary_found(&lookup, &name)
        }),
    )?;
    let resolve = files.clone();
    native.set(
        "resolvePath",
        Func::from(move |name: String| {
            if !permissions.file_system {
                return None;
            }
            paths::resolve(&resolve, &name, true, true)?
                .to_str()
                .map(str::to_owned)
        }),
    )?;
    let open = files.clone();
    native.set(
        "open",
        Func::from(move |target: String| {
            if !permissions.open_external || !paths::valid(&target) {
                return false;
            }
            if platform::is_http_url(&target) {
                return platform::open_http_url_in_browser(&target).is_ok();
            }
            paths::resolve(&open, &target, permissions.file_system, false)
                // A file association can start native code, even for plugin-owned files.
                .filter(|path| path.is_dir() || permissions.process_execution)
                .is_some_and(|path| platform::open_associated_path(&path).is_ok())
        }),
    )
}

fn install_credentials<'js>(native: &Object<'js>, files: &PluginFiles) -> rquickjs::Result<()> {
    let identity = files.storage_identity().map(str::to_owned);
    let writer = identity.clone();
    native.set(
        "keychainWrite",
        Func::from(move |service: String, name: String, secret: String| {
            secret.len() <= 64 * 1024
                && credential(writer.as_deref(), &service, &name)
                    .is_some_and(|entry| entry.set_password(&secret).is_ok())
        }),
    )?;
    native.set(
        "keychainRead",
        Func::from(move |ctx: Ctx<'js>, service: String, name: String| {
            use rquickjs::IntoJs;
            match credential(identity.as_deref(), &service, &name)
                .and_then(|entry| entry.get_password().ok())
            {
                Some(secret) => secret.into_js(&ctx),
                None => false.into_js(&ctx),
            }
        }),
    )
}

fn credential(identity: Option<&str>, service: &str, name: &str) -> Option<keyring::Entry> {
    let (service, account) = credential_target(identity?, service, name)?;
    keyring::Entry::new(&service, &account).ok()
}

fn credential_target(identity: &str, service: &str, name: &str) -> Option<(String, String)> {
    use sha2::{Digest, Sha256};
    if service.is_empty()
        || [identity, service, name]
            .iter()
            .any(|value| value.len() > 1024 || value.contains('\0'))
    {
        return None;
    }
    let service_hash = Sha256::new()
        .chain_update(identity)
        .chain_update([0])
        .chain_update(service);
    Some((
        format!("com.riina.plugin.{:x}", service_hash.finalize()),
        format!("{:x}", Sha256::digest(name)),
    ))
}

struct Helpers {
    files: PluginFiles,
    execution: Arc<ExecutionControl>,
    commands: SyncSender<RuntimeCommand>,
    active: Arc<AtomicUsize>,
    workers: Workers,
}

impl Helpers {
    fn start(
        &self,
        id: u32,
        name: String,
        args: Vec<String>,
        cwd: Option<String>,
        hooks: u8,
    ) -> Result<(), String> {
        if !paths::valid(&name)
            || args.len() > 256
            || args.iter().map(String::len).sum::<usize>() > MAX_ARGUMENT_BYTES
            || args.iter().any(|argument| argument.contains('\0'))
        {
            return Err("Invalid helper name or arguments".into());
        }
        let slot = Slot::acquire(&self.active).ok_or("Plugin already has two running helpers")?;
        let files = self.files.clone();
        let execution = self.execution.clone();
        let commands = self.commands.clone();
        self.workers.start(move || {
            let deadline = Instant::now() + HELPER_TIMEOUT;
            let result =
                build_invocation(&files, &name, args, cwd.as_deref()).and_then(|invocation| {
                    invocation
                        .run(
                            deadline,
                            || execution.is_cancelled(),
                            |channel, text| {
                                let flag = match channel {
                                    Channel::Stdout => 1,
                                    Channel::Stderr => 2,
                                };
                                if hooks & flag == 0 {
                                    return true;
                                }
                                send(
                                    &commands,
                                    &execution,
                                    Delivery::Output(id, channel, text),
                                    Some(deadline),
                                )
                            },
                        )
                        .map_err(|error| (RUNTIME_ERROR, error.to_string()))
                });
            drop(slot);
            send(&commands, &execution, Delivery::Finished(id, result), None);
        })
    }
}

#[derive(Clone, Default)]
pub(super) struct Workers(Arc<Mutex<Vec<JoinHandle<()>>>>);

impl Workers {
    fn start(&self, task: impl FnOnce() + Send + 'static) -> Result<(), String> {
        let mut workers = self
            .0
            .lock()
            .map_err(|_| "Helper supervisor is unavailable")?;
        workers.retain(|worker| !worker.is_finished());
        if workers.len() >= MAX_HELPERS + 2 {
            return Err("The plugin helper queue is full".into());
        }
        let worker = crate::background::spawn_named("riina-plugin-helper", task)
            .map_err(|error| error.to_string())?;
        workers.push(worker);
        Ok(())
    }

    pub(super) fn finish(&self) {
        let mut workers = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let deadline = Instant::now() + Duration::from_millis(150);
        while Instant::now() < deadline && workers.iter().any(|worker| !worker.is_finished()) {
            std::thread::sleep(BACKPRESSURE_RETRY);
        }
        // A host filesystem call may be stalled. Such a worker checks
        // cancellation again before launching a process when it resumes.
        for worker in workers.drain(..).filter(JoinHandle::is_finished) {
            let _ = worker.join();
        }
    }
}

fn build_invocation(
    files: &PluginFiles,
    name: &str,
    args: Vec<String>,
    cwd: Option<&str>,
) -> Result<Invocation, (i32, String)> {
    let executable = paths::find_helper(files, name).ok_or_else(|| {
        (
            BINARY_NOT_FOUND,
            "Could not find the plugin helper".to_owned(),
        )
    })?;
    let directory = paths::resolve(files, cwd.unwrap_or("@data/"), true, false)
        .filter(|path| path.is_dir())
        .ok_or_else(|| {
            (
                RUNTIME_ERROR,
                "The helper working directory is unavailable".to_owned(),
            )
        })?;
    paths::prepare_executable(files, &executable).map_err(|error| (RUNTIME_ERROR, error))?;
    Ok(Invocation {
        executable,
        args,
        directory,
    })
}

struct Slot(Arc<AtomicUsize>);

impl Slot {
    fn acquire(active: &Arc<AtomicUsize>) -> Option<Self> {
        active
            .try_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < MAX_HELPERS).then_some(count + 1)
            })
            .ok()?;
        Some(Self(active.clone()))
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Release);
    }
}

pub(super) enum Delivery {
    Output(u32, Channel, String),
    Finished(u32, Result<Completed, (i32, String)>),
    FileSelected(u32, Result<Option<String>, (i32, String)>),
}

impl Delivery {
    pub(super) fn dispatch(self, context: &Context) -> Result<(), String> {
        context
            .with(|ctx| {
                let deliver: Function<'_> = ctx.globals().get("__riinaUtilityResult")?;
                match self {
                    Self::Output(id, channel, text) => {
                        let stage = match channel {
                            Channel::Stdout => "stdout",
                            Channel::Stderr => "stderr",
                        };
                        deliver.call::<_, ()>((id, stage, text))
                    }
                    Self::Finished(id, Ok(result)) => {
                        let json = serde_json::to_string(&result)
                            .map_err(|error| js_error(error.to_string()))?;
                        let result: Value<'_> = ctx.json_parse(json)?;
                        deliver.call::<_, ()>((id, "done", result))
                    }
                    Self::Finished(id, Err((code, message))) => {
                        deliver.call::<_, ()>((id, "error", message, code))
                    }
                    Self::FileSelected(id, Ok(path)) => deliver.call::<_, ()>((id, "done", path)),
                    Self::FileSelected(id, Err((code, message))) => {
                        deliver.call::<_, ()>((id, "error", message, code))
                    }
                }
                .inspect_err(|error| {
                    if error.is_exception() {
                        ctx.catch();
                    }
                })
            })
            .map_err(|error: rquickjs::Error| error.to_string())
    }
}

fn send(
    commands: &SyncSender<RuntimeCommand>,
    execution: &ExecutionControl,
    delivery: Delivery,
    deadline: Option<Instant>,
) -> bool {
    let mut command = RuntimeCommand::DeliverUtility(delivery);
    while !execution.is_cancelled() && deadline.is_none_or(|end| Instant::now() < end) {
        match commands.try_send(command) {
            Ok(()) => return true,
            Err(TrySendError::Disconnected(_)) => return false,
            Err(TrySendError::Full(returned)) => {
                command = returned;
                std::thread::sleep(BACKPRESSURE_RETRY);
            }
        }
    }
    false
}

fn js_error(message: impl Into<String>) -> rquickjs::Error {
    rquickjs::Error::new_from_js_message("iina.utils", "utility", message.into())
}
