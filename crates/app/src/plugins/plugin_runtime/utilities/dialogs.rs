//! An asynchronous picker request goes to its player's UI and back to its realm.
use super::{Delivery, RUNTIME_ERROR, send};
use crate::plugin_runtime::{ExecutionControl, RuntimeCommand};
use serde::Deserialize;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::{SyncSender, TrySendError},
};

type Present = Box<dyn Fn(FileRequest) -> Result<(), String> + Send>;

#[derive(Clone, Default)]
pub(crate) struct FileDialogs {
    present: Arc<Mutex<Option<Present>>>,
    busy: Arc<AtomicBool>,
}

impl FileDialogs {
    pub(crate) fn install(
        &self,
        presenter: impl Fn(FileRequest) -> Result<(), String> + Send + 'static,
    ) {
        if let Ok(mut present) = self.present.lock() {
            *present = Some(Box::new(presenter));
        }
    }

    pub(super) fn open(
        &self,
        id: u32,
        title: String,
        options: &str,
        execution: Arc<ExecutionControl>,
        commands: SyncSender<RuntimeCommand>,
    ) -> Result<(), String> {
        if title.is_empty() || title.len() > 1024 || title.contains('\0') || options.len() > 4096 {
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
        let presenter = self
            .present
            .lock()
            .map_err(|_| "Plugin file picker is unavailable")?;
        let presenter = presenter
            .as_ref()
            .ok_or("Plugin file picker is unavailable")?;
        self.busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "A plugin file picker is already open")?;
        presenter(FileRequest {
            id,
            title,
            folder: options.folder,
            extensions: options.extensions,
            execution,
            commands,
            slot: Slot(self.busy.clone()),
        })
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
        self.execution.is_cancelled()
    }

    pub(crate) fn finish(self, result: Result<Option<String>, String>) {
        let Self {
            id,
            execution,
            commands,
            slot,
            ..
        } = self;
        drop(slot);
        if execution.is_cancelled() {
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

struct Slot(Arc<AtomicBool>);

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
