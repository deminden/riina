//! Utility dialogs belong to one player and return only to the requesting realm.
use super::{Delivery, RUNTIME_ERROR, send};
use crate::plugin_runtime::{ExecutionControl, RuntimeCommand};
use serde::Deserialize;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, SyncSender, TrySendError},
};
use std::time::Duration;

type Present = Box<dyn Fn(DialogRequest) -> Result<(), String> + Send>;

#[derive(Clone, Default)]
pub(crate) struct PluginDialogs {
    present: Arc<Mutex<Option<Present>>>,
    busy: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
}

impl PluginDialogs {
    pub(crate) fn install(
        &self,
        presenter: impl Fn(DialogRequest) -> Result<(), String> + Send + 'static,
    ) {
        if let Ok(mut present) = self.present.lock() {
            *present = Some(Box::new(presenter));
        }
    }

    pub(crate) fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
    }

    pub(crate) fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn notice(&self, title: String, message: String) -> Result<(), String> {
        self.present(DialogRequest::Notice(Notice {
            title,
            message,
            slot: self.claim()?,
        }))
    }

    pub(super) fn open(
        &self,
        id: u32,
        title: String,
        options: &str,
        execution: Arc<ExecutionControl>,
        commands: SyncSender<RuntimeCommand>,
    ) -> Result<(), String> {
        if !valid_title(&title) || options.len() > 4096 {
            return Err("The plugin file picker title or options are invalid".into());
        }
        let options: Options = serde_json::from_str(options).map_err(|error| error.to_string())?;
        if options.extensions.len() > 32
            || options.extensions.iter().any(|extension| {
                extension.is_empty()
                    || extension.len() > 32
                    || !extension.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.')
                    })
            })
        {
            return Err("The plugin file picker extensions are invalid".into());
        }
        self.present(DialogRequest::File(FileRequest {
            id,
            title,
            folder: options.folder,
            extensions: options.extensions,
            execution,
            commands,
            slot: self.claim()?,
        }))
    }

    pub(super) fn interact(
        &self,
        title: String,
        kind: InteractionKind,
        execution: &Arc<ExecutionControl>,
    ) -> Result<Option<String>, String> {
        if !valid_title(&title) {
            return Err("The plugin dialog title is invalid".into());
        }
        let slot = self.claim()?;
        let _pause = execution
            .wait_for_user()
            .ok_or("The plugin turn has ended")?;
        let (answer, response) = mpsc::sync_channel(1);
        self.present(DialogRequest::Interaction(InteractionRequest {
            title,
            kind,
            execution: execution.clone(),
            answer,
            slot,
        }))?;
        while !execution.is_cancelled() && !self.is_stopped() {
            match response.recv_timeout(Duration::from_millis(30)) {
                Ok(answer) => return answer,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        Err("The plugin dialog owner has closed".into())
    }

    fn claim(&self) -> Result<Slot, String> {
        if self.is_stopped() {
            return Err("The plugin dialog owner has closed".into());
        }
        self.busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "A plugin dialog is already open")?;
        Ok(Slot {
            busy: self.busy.clone(),
            stopped: self.stopped.clone(),
        })
    }

    fn present(&self, request: DialogRequest) -> Result<(), String> {
        let presenter = self
            .present
            .lock()
            .map_err(|_| "Plugin dialogs are unavailable")?;
        presenter.as_ref().ok_or("Plugin dialogs are unavailable")?(request)
    }
}

fn valid_title(title: &str) -> bool {
    !title.is_empty() && title.len() <= 1024 && !title.contains('\0')
}

pub(crate) enum DialogRequest {
    File(FileRequest),
    Interaction(InteractionRequest),
    #[cfg(target_os = "macos")]
    Notice(Notice),
}

impl DialogRequest {
    pub(crate) fn fail(self, message: String) {
        match self {
            Self::File(request) => request.finish(Err(message)),
            Self::Interaction(request) => request.finish(Err(message)),
            #[cfg(target_os = "macos")]
            Self::Notice(_) => crate::diagnostics::warn!("{message}"),
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) struct Notice {
    pub(crate) title: String,
    pub(crate) message: String,
    slot: Slot,
}

#[cfg(target_os = "macos")]
impl Notice {
    pub(crate) fn is_cancelled(&self) -> bool {
        self.slot.stopped.load(Ordering::Acquire)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum InteractionKind {
    Confirm,
    Prompt,
}

pub(crate) struct InteractionRequest {
    pub(crate) title: String,
    pub(crate) kind: InteractionKind,
    execution: Arc<ExecutionControl>,
    answer: SyncSender<Result<Option<String>, String>>,
    slot: Slot,
}

impl InteractionRequest {
    pub(crate) fn is_cancelled(&self) -> bool {
        self.execution.is_cancelled() || self.slot.stopped.load(Ordering::Acquire)
    }

    pub(crate) fn cancellation(&self) -> impl Fn() -> bool + use<> {
        let execution = self.execution.clone();
        let stopped = self.slot.stopped.clone();
        move || execution.is_cancelled() || stopped.load(Ordering::Acquire)
    }

    pub(crate) fn finish(self, result: Result<Option<String>, String>) {
        let cancelled = self.is_cancelled();
        let Self { answer, slot, .. } = self;
        drop(slot);
        if !cancelled {
            let _ = answer.try_send(result);
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Options {
    #[serde(rename = "chooseDir")]
    folder: bool,
    #[serde(rename = "allowedFileTypes")]
    extensions: Vec<String>,
}

pub(crate) struct FileRequest {
    id: u32,
    pub(crate) title: String,
    pub(crate) folder: bool,
    pub(crate) extensions: Vec<String>,
    execution: Arc<ExecutionControl>,
    commands: SyncSender<RuntimeCommand>,
    slot: Slot,
}

impl FileRequest {
    pub(crate) fn is_cancelled(&self) -> bool {
        self.execution.is_cancelled() || self.slot.stopped.load(Ordering::Acquire)
    }

    pub(crate) fn finish(self, result: Result<Option<String>, String>) {
        let cancelled = self.is_cancelled();
        let Self {
            id,
            execution,
            commands,
            slot,
            ..
        } = self;
        drop(slot);
        if cancelled {
            return;
        }
        let delivery =
            Delivery::FileSelected(id, result.map_err(|message| (RUNTIME_ERROR, message)));
        // A large callback batch can fill the realm's queue. The UI must never
        // wait on that queue while returning from a native picker.
        if let Err(TrySendError::Full(RuntimeCommand::DeliverUtility(delivery))) =
            commands.try_send(RuntimeCommand::DeliverUtility(delivery))
        {
            let _ = crate::background::spawn_named("riina-plugin-file-result", move || {
                send(&commands, &execution, delivery, None);
            });
        }
    }
}

struct Slot {
    busy: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
}

impl Drop for Slot {
    fn drop(&mut self) {
        self.busy.store(false, Ordering::Release);
    }
}
