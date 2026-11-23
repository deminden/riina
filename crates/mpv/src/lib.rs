//! The event worker owns the player; the GUI owns only its OpenGL render context.
mod auto_seek;
mod error;
pub use error::EngineError;
mod ffi;
pub mod gl_support;
mod pending;
mod properties;
mod property_read;
pub use property_read::MpvPropertyFormat;
#[cfg(feature = "test-support")]
mod recording;
mod seek_feedback;
pub mod thumbnail_generator;
pub mod video;
#[cfg(windows)]
mod windows_apartment;

use ffi::Api;
#[cfg(feature = "test-support")]
pub use recording::RecordingEngine;
use riina_core::{Command, Playback};
use std::{
    ffi::c_void,
    path::Path,
    rc::Rc,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::JoinHandle,
};

static DIAGNOSTIC_SINK: OnceLock<fn(&str)> = OnceLock::new();

pub fn set_diagnostic_sink(sink: fn(&str)) {
    let _ = DIAGNOSTIC_SINK.set(sink);
}

fn diag(message: &str) {
    if let Some(sink) = DIAGNOSTIC_SINK.get() {
        sink(message);
    } else {
        // Native test binaries and library callers can run without an app log.
        use std::io::Write;
        let _ = writeln!(std::io::stderr(), "{message}");
    }
}

fn record_notice(playback: &mut Playback, severity: riina_core::NoticeSeverity, message: String) {
    diag(&format!("[{severity:?}] {message}"));
    playback.notices.push(severity, message);
}

macro_rules! diagnostic {
    ($($args:tt)*) => { diag(&format!($($args)*)) };
}

mod worker;

enum Message {
    Intent(Command),
    ReadProperty(property_read::Request),
    Wake,
    Shutdown,
}

struct WakeupContext {
    sender: SyncSender<Message>,
    queued: AtomicBool,
}

impl WakeupContext {
    fn signal(&self) {
        if self.queued.swap(true, Ordering::AcqRel) {
            return;
        }
        if self.sender.try_send(Message::Wake).is_err() {
            // A full queue already guarantees the worker has work to consume;
            // let the next native notification request another wake if needed.
            self.queued.store(false, Ordering::Release);
        }
    }
}

struct Owner {
    sender: SyncSender<Message>,
    stopped: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Owner {
    fn drop(&mut self) {
        // Renderers retain the owner, so this runs only after render contexts are freed.
        let _ = self.sender.send(Message::Shutdown);
        trace_shutdown("shutdown intent delivered; joining event worker");
        if let Some(thread) = self.thread.take() {
            if thread.join().is_ok() {
                trace_shutdown("event worker joined");
            } else {
                diagnostic!("The native player event worker panicked during shutdown");
            }
        }
    }
}

fn trace_shutdown(stage: &str) {
    if std::env::var_os("RIINA_TEST_TRACE_SHUTDOWN").is_some() {
        diagnostic!("mpv shutdown: {stage}");
    }
}

#[derive(Clone)]
pub struct Player {
    owner: Rc<Owner>,
    api: Api,
    handle: usize,
}

#[derive(Clone)]
enum CommandQueue {
    Native(SyncSender<Message>),
    #[cfg(feature = "test-support")]
    Recording(SyncSender<Command>),
}

#[derive(Clone)]
pub struct CommandSender {
    sender: CommandQueue,
    stopped: Arc<AtomicBool>,
}

impl CommandSender {
    pub fn send(&self, command: Command) -> Result<(), EngineError> {
        if command == Command::Shutdown {
            return Err(EngineError::ReservedShutdown);
        }
        if self.stopped.load(Ordering::Acquire) {
            return Err(EngineError::Stopped);
        }
        match &self.sender {
            CommandQueue::Native(sender) => sender
                .try_send(Message::Intent(command))
                .map_err(send_error),
            #[cfg(feature = "test-support")]
            CommandQueue::Recording(sender) => sender.try_send(command).map_err(send_error),
        }
    }

    /// All accepted intents use the same FIFO; a full queue rejects explicitly.
    /// Never defer an action on an independent thread where later input can pass it.
    pub fn toggle_pause(&self) -> Result<(), EngineError> {
        self.send(Command::TogglePause)
    }

    pub fn set_paused(&self, paused: bool) -> Result<(), EngineError> {
        self.send(Command::SetPaused(paused))
    }
}

fn send_error<T>(error: mpsc::TrySendError<T>) -> EngineError {
    match error {
        mpsc::TrySendError::Full(_) => EngineError::QueueFull,
        mpsc::TrySendError::Disconnected(_) => EngineError::Stopped,
    }
}

unsafe extern "C" fn wakeup(context: *mut c_void) {
    // SAFETY: the worker retains this context through native core destruction,
    // including callbacks already in flight when it unregisters the wakeup callback.
    let wakeup = unsafe { &*(context.cast::<WakeupContext>()) };
    wakeup.signal();
}

impl Player {
    pub fn start(
        path: &Path,
        headless: bool,
        notify: impl Fn(Arc<Playback>) + Send + 'static,
    ) -> Result<Self, EngineError> {
        Self::start_with_options(path, headless, Vec::new(), notify)
    }

    pub fn start_with_options(
        path: &Path,
        headless: bool,
        options: Vec<(String, String)>,
        notify: impl Fn(Arc<Playback>) + Send + 'static,
    ) -> Result<Self, EngineError> {
        Self::start_options(path, headless, options, notify)
    }

    /// Experimental native VO. The caller must retain the HWND until Player drops.
    /// mpv creates its child video window; no render API context may use this player.
    #[cfg(target_os = "windows")]
    pub fn start_native_window(
        path: &Path,
        window: u32,
        mut options: Vec<(String, String)>,
        notify: impl Fn(Arc<Playback>) + Send + 'static,
    ) -> Result<Self, EngineError> {
        if window == 0 {
            return Err(EngineError::InvalidArgument {
                reason: "Native output requires a live HWND",
            });
        }
        options.extend([
            ("vo".into(), "gpu-next".into()),
            ("wid".into(), window.to_string()),
            ("gpu-api".into(), "d3d11".into()),
            ("gpu-context".into(), "d3d11".into()),
            ("d3d11-output-format".into(), "rgba16f".into()),
            ("target-colorspace-hint".into(), "auto".into()),
            // Riina owns video-size policy and synchronizes its host and mpv
            // child surface. Letting mpv resize the embedded window itself
            // races that geometry update during playlist handoffs.
            ("auto-window-resize".into(), "no".into()),
            ("prefetch-playlist".into(), "yes".into()),
        ]);
        Self::start_options(path, false, options, notify)
    }

    fn start_options(
        path: &Path,
        headless: bool,
        options: Vec<(String, String)>,
        notify: impl Fn(Arc<Playback>) + Send + 'static,
    ) -> Result<Self, EngineError> {
        let api = Api::load(path)?;
        let (sender, receiver) = mpsc::sync_channel(64);
        let stopped = Arc::new(AtomicBool::new(false));
        let (ready, started) = mpsc::sync_channel(1);
        let worker = worker::Start {
            api: api.clone(),
            headless,
            options,
            notify,
            receiver,
            callback_sender: sender.clone(),
            stopped: stopped.clone(),
            ready,
        };
        let thread = std::thread::Builder::new()
            .name("mpv-events".into())
            .spawn(move || worker.run())
            .map_err(EngineError::WorkerStart)?;
        let handle = started.recv().map_err(EngineError::WorkerHandshake)??;
        Ok(Self {
            owner: Rc::new(Owner {
                sender,
                stopped,
                thread: Some(thread),
            }),
            api,
            handle,
        })
    }

    pub fn send(&self, command: Command) -> Result<(), EngineError> {
        self.command_sender().send(command)
    }

    pub fn command_sender(&self) -> CommandSender {
        CommandSender {
            sender: CommandQueue::Native(self.owner.sender.clone()),
            stopped: self.owner.stopped.clone(),
        }
    }
}

#[cfg(test)]
mod command_sender_tests;
