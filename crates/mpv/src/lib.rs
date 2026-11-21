//! The event worker owns the player; the GUI owns only its OpenGL render context.
mod auto_seek;
#[cfg(test)]
mod demuxer_tests;
mod ffi;
mod pending;
mod properties;
#[cfg(feature = "test-support")]
mod recording;
mod seek_feedback;
pub mod thumbnail_generator;
pub mod video;
#[cfg(windows)]
mod windows_apartment;

use ffi::{Api, EndFile, Property, end_reason, event_id, format};
use pending::{PendingCommand, ReplyKind};
use properties::{apply_property, observed_property_value};
#[cfg(feature = "test-support")]
pub use recording::RecordingEngine;
use riina_core::{Command, Phase, Playback};
use std::{
    collections::{BTreeMap, HashMap},
    ffi::{CStr, CString, c_void},
    panic::{AssertUnwindSafe, catch_unwind},
    path::Path,
    rc::Rc,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::JoinHandle,
    time::Instant,
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

macro_rules! diagnostic {
    ($($args:tt)*) => { diag(&format!($($args)*)) };
}

enum Message {
    Intent(Command),
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
    pub fn send(&self, command: Command) -> Result<(), String> {
        if command == Command::Shutdown {
            return Err("Player shutdown is reserved for its final owner".into());
        }
        if self.stopped.load(Ordering::Acquire) {
            return Err("The playback engine has stopped".into());
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
    pub fn toggle_pause(&self) -> Result<(), String> {
        self.send(Command::TogglePause)
    }

    pub fn set_paused(&self, paused: bool) -> Result<(), String> {
        self.send(Command::SetPaused(paused))
    }
}

fn send_error<T>(error: mpsc::TrySendError<T>) -> String {
    match error {
        mpsc::TrySendError::Full(_) => format!("Player command was not accepted: {error}"),
        mpsc::TrySendError::Disconnected(_) => "The playback engine has stopped".into(),
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
        notify: impl Fn(Playback) + Send + 'static,
    ) -> Result<Self, String> {
        Self::start_with_options(path, headless, Vec::new(), notify)
    }

    pub fn start_with_options(
        path: &Path,
        headless: bool,
        options: Vec<(String, String)>,
        notify: impl Fn(Playback) + Send + 'static,
    ) -> Result<Self, String> {
        Self::start_options(path, headless, options, notify)
    }

    /// Experimental native VO. The caller must retain the HWND until Player drops.
    /// mpv creates its child video window; no render API context may use this player.
    #[cfg(target_os = "windows")]
    pub fn start_native_window(
        path: &Path,
        window: u32,
        mut options: Vec<(String, String)>,
        notify: impl Fn(Playback) + Send + 'static,
    ) -> Result<Self, String> {
        if window == 0 {
            return Err("Native output requires a live HWND".into());
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

    #[expect(
        clippy::too_many_lines,
        reason = "decomposition tracked in docs/review"
    )]
    #[expect(
        clippy::unwrap_used,
        reason = "Fixed mpv option and property names contain no interior NULs"
    )]
    fn start_options(
        path: &Path,
        headless: bool,
        options: Vec<(String, String)>,
        notify: impl Fn(Playback) + Send + 'static,
    ) -> Result<Self, String> {
        let api = Api::load(path)?;
        let worker_api = api.clone();
        let (sender, receiver) = mpsc::sync_channel(64);
        let callback_sender = sender.clone();
        let stopped = Arc::new(AtomicBool::new(false));
        let worker_stopped = stopped.clone();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let thread = std::thread::Builder::new().name("mpv-events".into()).spawn(move || {
            #[cfg(windows)]
            let _apartment = match windows_apartment::Apartment::initialize() {
                Ok(apartment) => apartment,
                Err(error) => { let _ = ready_tx.send(Err(error)); return; }
            };
            // SAFETY: all ordinary client calls stay on this worker. Only the render API
            // sees the handle on the GUI thread, as permitted by render.h's threading contract.
            unsafe {
                let handle = (worker_api.create)();
                if handle.is_null() { let _ = ready_tx.send(Err("mpv_create failed".into())); return; }
                let mut startup_option_errors = Vec::new();
                let initialized = (|| -> Result<(), String> {
                    for (key, value) in [("config", "no"), ("load-scripts", "no"), ("idle", "yes"),
                        ("vo", if headless { "null" } else { "libmpv" }),
                        ("volume-max", "100"), ("terminal", "no"), ("hwdec", "auto-safe"), ("keep-open", "yes"),
                        // IINA's secondary override defaults to scale, whereas
                        // mpv defaults to stripping the secondary lane's styles.
                        ("sub-ass-override", "scale"),
                        ("secondary-sub-ass-override", "scale"),
                        ("deinterlace", "no"),
                        ("screenshot-format", "png"),
                        ("screenshot-template", "%F-%n"),
                        // Reference still-image encoding uses SVT's AVIF mode;
                        // the engine's default encoder/options can differ.
                        ("screenshot-avif-encoder", "libsvtav1"),
                        ("screenshot-avif-opts", "svtav1-params=avif=1"),
                        ("sub-font-size", "55"),
                        ("sub-font", riina_core::subtitle_controls::DEFAULT_SUBTITLE_FONT),
                        ("sub-outline-size", "3"),
                        ("sub-color", "1/1/1/1"),
                        ("sub-outline-color", "0/0/0/1"),
                        ("sub-back-color", "0/0/0/0"),
                        ("sub-bold", "no"),
                        ("sub-italic", "no"),
                        ("sub-blur", "0"),
                        ("sub-spacing", "0"),
                        ("sub-shadow-offset", "0"),
                        // Loop points belong to the current media, unlike volume
                        // and speed. Reset in mpv too, not just in our UI snapshot.
                        ("reset-on-next-file", "ab-loop-a,ab-loop-b")] {
                        let key = CString::new(key).unwrap();
                        let value = CString::new(value).unwrap();
                        worker_api.check((worker_api.option)(handle, key.as_ptr(), value.as_ptr()))?;
                    }
                    for (key, value) in &options {
                        let key = CString::new(key.as_str()).map_err(|e| e.to_string())?;
                        let value = CString::new(value.as_str()).map_err(|e| e.to_string())?;
                        let result = (worker_api.option)(handle, key.as_ptr(), value.as_ptr());
                        if let Err(error) = worker_api.check(result) {
                            // IINA reports invalid optional mpv preferences but still
                            // starts the player; a typo must not make media unplayable.
                            startup_option_errors.push(format!(
                                "{}: {error}",
                                key.to_string_lossy()
                            ));
                        }
                    }
                    worker_api.check((worker_api.initialize)(handle))?;
                    for property in properties::OBSERVED {
                        worker_api.check((worker_api.observe)(
                            handle, 0, property.name.as_ptr(), property.format,
                        ))?;
                    }
                    Ok(())
                })();
                if let Err(error) = initialized {
                    let _ = ready_tx.send(Err(error));
                    (worker_api.destroy)(handle);
                    return;
                }
                let mut callback = Box::new(WakeupContext {
                    sender: callback_sender,
                    queued: AtomicBool::new(false),
                });
                (worker_api.wakeup)(handle, Some(wakeup), (&mut *callback as *mut WakeupContext).cast());
                // The numeric token is opaque outside video.rs. No Rust object is given a
                // fabricated Send implementation; the worker owns and destroys this handle.
                let _ = ready_tx.send(Ok(handle as usize));
                let mut state = Playback {
                    volume: 100.0,
                    speed: 1.0,
                    error: (!startup_option_errors.is_empty()).then(|| {
                        format!(
                            "Some mpv options were ignored: {}",
                            startup_option_errors.join("; ")
                        )
                    }),
                    ..Default::default()
                };
                let mut next_id = 0u64;
                let mut dynamic_property_observers = BTreeMap::<u64, String>::new();
                let mut next_dynamic_property_observer = 1_u64;
                let mut pending: HashMap<u64, PendingCommand> = HashMap::new();
                let mut pending_drop_subtitles = Vec::<String>::new();
                let mut resume_on_load = false;
                let mut seek_pending = false;
                let mut seek_feedback = seek_feedback::SeekFeedback::default();
                let mut auto_seek = auto_seek::AutoSeek::default();
                while let Ok(message) = receiver.recv() {
                    if matches!(message, Message::Shutdown) {
                        // Only the final owner can terminate the core, after every render
                        // context has been freed. Pending captures must not delay quitting.
                        trace_shutdown("shutdown accepted; terminating pending playback work");
                        break;
                    }
                    if catch_unwind(AssertUnwindSafe(|| {
                    if matches!(message, Message::Wake) {
                        callback.queued.store(false, Ordering::Release);
                    }
                    if let Message::Intent(command) = message {
                        if let Command::ObserveMpvProperty(property) = &command {
                            let already_observed = dynamic_property_observers
                                .values()
                                .any(|observed| observed == property);
                            if !already_observed {
                                if dynamic_property_observers.len()
                                    >= riina_core::MAX_PLUGIN_OBSERVED_MPV_PROPERTIES
                                {
                                    state.error = Some(format!(
                                        "Player has reached the {} observed mpv property limit",
                                        riina_core::MAX_PLUGIN_OBSERVED_MPV_PROPERTIES
                                    ));
                                } else if let Err(error) = command.plan(&state) {
                                    state.error = Some(error);
                                } else {
                                    match CString::new(property.as_str()) {
                                        Ok(name) => {
                                            let observer_id = next_dynamic_property_observer;
                                            // SAFETY: the player worker owns the live handle and
                                            // libmpv copies the property name during this call.
                                            match worker_api.check({
                                                (worker_api.observe)(
                                                    handle,
                                                    observer_id,
                                                    name.as_ptr(),
                                                    format::NODE,
                                                )
                                            }) {
                                                Ok(()) => {
                                                    dynamic_property_observers.insert(
                                                        observer_id,
                                                        property.clone(),
                                                    );
                                                    next_dynamic_property_observer =
                                                        next_dynamic_property_observer
                                                            .saturating_add(1);
                                                }
                                                Err(error) => {
                                                    state.error = Some(format!(
                                                        "Could not observe mpv property {property}: {error}"
                                                    ));
                                                }
                                            }
                                        }
                                        Err(error) => state.error = Some(error.to_string()),
                                    }
                                }
                                if state.error.is_some() {
                                    state.reconcile();
                                    notify(state.clone());
                                }
                            }
                            return;
                        }
                        if command.replaces_media() {
                            pending_drop_subtitles.clear();
                        }
                        match command.plan(&state) {
                            Ok(plan) => {
                                if let Command::SetPauseWhenOpen(pause) = &command {
                                    // Changing the policy must not pause current playback.
                                    state.pause_when_open = *pause;
                                }
                                // Requested text is player intent, not proof of the native ratio.
                                // Keep it even on native failure and across source replacement:
                                // aspect is a player option, unlike generation-bound crop geometry.
                                if let Command::VideoAspect(aspect) = &command {
                                    state.video_aspect = aspect.clone();
                                    state.aspect_requests = state.aspect_requests.wrapping_add(1);
                                }
                                if matches!(command, Command::TogglePause | Command::SetPaused(_)) { seek_feedback.playback_transition(); }
                                let dropped_subtitles = match &command {
                                    Command::OpenFilesWithSubtitles { subtitles, .. } => {
                                        Some(subtitles.clone())
                                    }
                                    _ => None,
                                };
                                if command.replaces_media() { resume_on_load = false; }
                                resume_on_load |= plan.resume_on_load;
                                let is_auto_seek = matches!(
                                    &command,
                                    Command::SeekWithType {
                                        seek_type: riina_core::control_preferences::SeekType::Auto,
                                        ..
                                    }
                                );
                                // Validate the whole intent before submitting any part. Multi-file
                                // additions consume one queue slot and cannot be partially dropped.
                                let mut command_issue_succeeded = true;
                                for mut args in plan.commands {
                                    if is_auto_seek
                                        && args.first().is_some_and(|argument| argument == "seek")
                                        && args.len() >= 3
                                    {
                                        args[2] = auto_seek.flags().into();
                                    } else if seek_feedback::navigation(&args, &state) {
                                        auto_seek.interrupt_measurement();
                                    }
                                    // Decoder option changes can produce their own SEEK /
                                    // RESTART pair. They replace pending navigation feedback,
                                    // while the raw native completion counter remains intact.
                                    if command.restarts_decoder() {
                                        seek_feedback.clear();
                                        auto_seek.interrupt_measurement();
                                    }
                                    let reply = ReplyKind::for_command(
                                        &command, args.first().map(String::as_str),
                                    );
                                    if let Err(error) = issue(
                                        &worker_api,
                                        handle,
                                        &args,
                                        &mut next_id,
                                        &mut pending,
                                        state.generation,
                                        reply,
                                    ) {
                                        if matches!(command, Command::CaptureMaterialFrame) {
                                            diagnostic!("Could not request a Windows video material frame: {error}");
                                        } else {
                                            state.error = Some(error);
                                            resume_on_load = false;
                                        }
                                        command_issue_succeeded = false;
                                        break;
                                    }
                                    if is_auto_seek && args.first().is_some_and(|arg| arg == "seek") {
                                        auto_seek.submitted(next_id);
                                    }
                                    if matches!(command, Command::TogglePlaylistShuffle) {
                                        state.playlist_shuffled = !state.playlist_shuffled;
                                    } else if command.resets_playlist_shuffle() {
                                        state.playlist_shuffled = false;
                                    }
                                    if seek_feedback::navigation(&args, &state) { seek_feedback.submitted(next_id); }
                                }
                                if command_issue_succeeded
                                    && let Some(subtitles) = dropped_subtitles
                                {
                                    pending_drop_subtitles = subtitles;
                                }
                                if !command_issue_succeeded && matches!(command, Command::CaptureFrame) {
                                    state.finished_frame_capture_requests = state.finished_frame_capture_requests.wrapping_add(1);
                                }
                            }
                            Err(error) => {
                                if matches!(command, Command::CaptureFrame) {
                                    state.finished_frame_capture_requests = state.finished_frame_capture_requests.wrapping_add(1);
                                }
                                state.error = Some(error);
                            }
                        }
                    }
                    loop {
                        let event = &*(worker_api.wait)(handle, 0.0);
                        if event.id == event_id::NONE { break; }
                        let event_name = (worker_api.event_name)(event.id);
                        if !event_name.is_null() {
                            state.record_mpv_event(
                                CStr::from_ptr(event_name).to_string_lossy().into_owned(),
                            );
                        }
                        match event.id {
                            event_id::COMMAND_REPLY => {
                                seek_feedback.replied(event.userdata, event.error < 0);
                                if event.error < 0 {
                                    auto_seek.failed(event.userdata);
                                }
                                if let Some(pending_command) = pending.remove(&event.userdata)
                                    && pending_command.belongs_to(&state)
                                {
                                    let frame_capture = matches!(pending_command.kind, ReplyKind::FrameCapture);
                                    let transition_capture =
                                        matches!(pending_command.kind, ReplyKind::TransitionCapture);
                                    let incoming_transition_capture =
                                        matches!(pending_command.kind, ReplyKind::IncomingTransitionCapture);
                                    let material_capture = matches!(pending_command.kind, ReplyKind::MaterialCapture);
                                    if frame_capture {
                                        state.finished_frame_capture_requests = state.finished_frame_capture_requests.wrapping_add(1);
                                    }
                                    if event.error < 0 {
                                        if material_capture {
                                            let reason = worker_api
                                                .check(event.error)
                                                .err()
                                                .unwrap_or_else(|| format!("mpv error {}", event.error));
                                            diagnostic!("Could not capture the Windows video material frame: {reason}");
                                        } else if !frame_capture
                                            && !transition_capture
                                            && !incoming_transition_capture
                                        {
                                            state.error = worker_api.check(event.error).err().map(|error| {
                                                format!("mpv {}: {error}", pending_command.label)
                                            });
                                            resume_on_load = false;
                                        }
                                    } else if frame_capture || transition_capture || incoming_transition_capture || material_capture {
                                        if event.data.is_null() {
                                            if material_capture {
                                                diagnostic!("mpv returned no result data for the Windows video material frame");
                                            } else {
                                                diagnostic!("mpv returned no frame for an asynchronous frame capture");
                                            }
                                        } else {
                                            // SAFETY: COMMAND_REPLY keeps this raw frame alive only until
                                            // the next wait_event, so copy its pixels before continuing.
                                            let reply = &*event.data.cast::<ffi::CommandReply>();
                                            match reply.result.copy_rgba_frame() {
                                                Ok(frame) => {
                                                    let frame = if material_capture {
                                                        frame.downsample_to_fit(320, 180)
                                                    } else {
                                                        Ok(frame)
                                                    };
                                                    let frame = match frame {
                                                        Ok(frame) => Arc::new(frame),
                                                        Err(error) => {
                                                            diagnostic!("Could not reduce an asynchronous material frame: {error}");
                                                            continue;
                                                        }
                                                    };
                                                    if incoming_transition_capture {
                                                        state.incoming_transition_frame = Some(frame);
                                                        state.incoming_transition_frame_revision = state
                                                            .incoming_transition_frame_revision
                                                            .wrapping_add(1);
                                                        state.incoming_transition_frame_generation =
                                                            Some(state.generation);
                                                        state.incoming_transition_frame_position =
                                                            state.position;
                                                        state.incoming_transition_frame_captured_at =
                                                            Some(std::time::Instant::now());
                                                    } else if material_capture {
                                                        state.material_frame = Some(frame);
                                                        state.material_frame_generation =
                                                            Some(state.generation);
                                                        state.material_frame_position =
                                                            state.position;
                                                        state.material_frame_captured_at =
                                                            Some(std::time::Instant::now());
                                                        state.material_frame_revision = state
                                                            .material_frame_revision
                                                            .wrapping_add(1);
                                                    } else if transition_capture {
                                                        state.transition_frame = Some(frame);
                                                        state.transition_frame_generation =
                                                            Some(pending_command.generation);
                                                        state.transition_frame_revision = state
                                                            .transition_frame_revision
                                                            .wrapping_add(1);
                                                        state.transition_frame_captured_at =
                                                            Some(std::time::Instant::now());
                                                    } else {
                                                        state.captured_frame = Some(frame);
                                                        state.completed_frame_captures = state.completed_frame_captures.wrapping_add(1);
                                                    }
                                                }
                                                Err(error) if material_capture => {
                                                    diagnostic!("Could not decode the Windows video material frame: {error}");
                                                }
                                                Err(error) => diagnostic!("Could not copy an asynchronous frame capture: {error}"),
                                            }
                                        }
                                    } else if let ReplyKind::Screenshot { path } = pending_command.kind {
                                        // Queue acceptance is not capture success. Retire replies
                                        // from replaced sources without replaying old feedback.
                                        let path = path.or_else(|| {
                                            if event.data.is_null() { return None; }
                                            // SAFETY: COMMAND_REPLY owns this node until the
                                            // next wait_event; copy its returned filename now.
                                            let reply = &*event.data.cast::<ffi::CommandReply>();
                                            reply.result.copy_json().ok()?.get("filename")?
                                                .as_str().map(str::to_owned)
                                        });
                                        if let Some(path) = path {
                                            state.screenshot_file = Some(path);
                                            state.completed_screenshots = state.completed_screenshots.wrapping_add(1);
                                        } else {
                                            state.error = Some("Screenshot completed without a saved filename".into());
                                        }
                                    }
                                }
                            }
                            event_id::START_FILE => {
                                seek_pending = false;
                                seek_feedback.clear();
                                auto_seek = auto_seek::AutoSeek::default();
                                state.begin_load();
                            },
                            event_id::END_FILE if !event.data.is_null() => {
                                let end = &*event.data.cast::<EndFile>();
                                seek_pending = false;
                                seek_feedback.clear();
                                auto_seek.interrupt_measurement();
                                state.phase = match end.reason {
                                    end_reason::EOF => Phase::Ended,
                                    end_reason::ERROR => Phase::Failed,
                                    _ => Phase::Stopped,
                                };
                                if matches!(state.phase, Phase::Stopped | Phase::Failed) {
                                    state.ab_loop_a = None;
                                    state.ab_loop_b = None;
                                }
                                if end.error < 0 { state.error = worker_api.check(end.error).err(); }
                                if end.reason == end_reason::ERROR { resume_on_load = false; }
                            }
                            event_id::FILE_LOADED => {
                                state.phase = Phase::Playing;
                                for path in std::mem::take(&mut pending_drop_subtitles) {
                                    let args = vec!["sub-add".into(), path, "auto".into()];
                                    if let Err(error) = issue(
                                        &worker_api,
                                        handle,
                                        &args,
                                        &mut next_id,
                                        &mut pending,
                                        state.generation,
                                        ReplyKind::Plain,
                                    ) {
                                        state.error = Some(error);
                                        break;
                                    }
                                }
                                if resume_on_load {
                                    resume_on_load = false;
                                    let args = ["set".into(), "pause".into(), "no".into()];
                                    if let Err(error) = issue(
                                        &worker_api,
                                        handle,
                                        &args,
                                        &mut next_id,
                                        &mut pending,
                                        state.generation,
                                        ReplyKind::Plain,
                                    ) {
                                        state.error = Some(error);
                                    }
                                }
                            },
                            event_id::SEEK => {
                                seek_pending = true;
                                seek_feedback.started();
                                auto_seek.seek_started(Instant::now());
                            },
                            event_id::PLAYBACK_RESTART if seek_pending => {
                                // Restart also occurs on initial load. Only a preceding
                                // native SEEK establishes a completed seek notification.
                                seek_pending = false;
                                state.completed_seeks = state.completed_seeks.wrapping_add(1);
                                auto_seek.playback_restarted(Instant::now());
                                if seek_feedback.completed() {
                                    state.completed_navigation_seeks = state.completed_navigation_seeks.wrapping_add(1);
                                }
                            }
                            event_id::PROPERTY_CHANGE if !event.data.is_null() => {
                                let property = &*event.data.cast::<Property>();
                                if let Some(name) = dynamic_property_observers
                                    .get(&event.userdata)
                                    .cloned()
                                {
                                    match observed_property_value(property) {
                                        Ok(value) => {
                                            state.observed_mpv_properties.insert(name, value);
                                        }
                                        Err(error) => {
                                            diagnostic!(
                                                "Could not copy observed mpv plugin property: {error}"
                                            );
                                        }
                                    }
                                }
                                apply_property(&mut state, property);
                            }
                            event_id::QUEUE_OVERFLOW => {
                                state.error = Some(
                                    "The libmpv event queue overflowed; playback state may be incomplete".into(),
                                );
                            }
                            _ => {}
                        }
                    }
                    state.reconcile();
                    notify(state.clone());
                    })).is_err() {
                        worker_stopped.store(true, Ordering::Release);
                        diag("The native player event worker stopped unexpectedly");
                        state.phase = Phase::Failed;
                        state.error = Some(
                            "The playback engine stopped unexpectedly; reopen the file".into(),
                        );
                        // The event worker still owns the handle. Stop media without
                        // destroying a core that a shared OpenGL surface still uses.
                        let stop = [c"stop".as_ptr(), std::ptr::null()];
                        (worker_api.command)(handle, 0, stop.as_ptr());
                        let _ = catch_unwind(AssertUnwindSafe(|| notify(state.clone())));
                        for queued in &receiver {
                            if matches!(queued, Message::Shutdown) {
                                trace_shutdown("shutdown accepted; terminating pending playback work");
                                break;
                            }
                        }
                        break;
                    }
                }
                (worker_api.wakeup)(handle, None, std::ptr::null_mut());
                trace_shutdown("wakeup callback unregistered; terminating native core");
                // Keep the callback context alive through destroy: unregistering prevents
                // future wakeups, but an mpv worker may already have entered the callback.
                (worker_api.destroy)(handle);
                trace_shutdown("native core terminated");
                drop(callback);
                trace_shutdown("callback context released");
            }
        }).map_err(|e| e.to_string())?;
        let handle = ready_rx.recv().map_err(|e| e.to_string())??;
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

    pub fn send(&self, command: Command) -> Result<(), String> {
        self.command_sender().send(command)
    }

    pub fn command_sender(&self) -> CommandSender {
        CommandSender {
            sender: CommandQueue::Native(self.owner.sender.clone()),
            stopped: self.owner.stopped.clone(),
        }
    }
}

unsafe fn issue(
    api: &Api,
    handle: *mut c_void,
    args: &[String],
    next_id: &mut u64,
    pending: &mut HashMap<u64, PendingCommand>,
    generation: u64,
    kind: ReplyKind,
) -> Result<(), String> {
    let args: Vec<_> = args
        .iter()
        .map(|arg| CString::new(arg.as_str()))
        .collect::<Result<_, _>>()
        .map_err(|error| error.to_string())?;
    let mut pointers: Vec<_> = args.iter().map(|arg| arg.as_ptr()).collect();
    pointers.push(std::ptr::null());
    *next_id += 1;
    let label = command_label(&args);
    // SAFETY: worker owns the live handle; command_async copies these strings
    // before returning, as specified by client.h. Reply ownership stays with mpv.
    api.check(unsafe { (api.command)(handle, *next_id, pointers.as_ptr()) })
        .map_err(|error| format!("mpv {label}: {error}"))?;
    pending.insert(
        *next_id,
        PendingCommand {
            generation,
            label,
            kind,
        },
    );
    Ok(())
}

// Keep diagnostics actionable without exposing media paths or arbitrary option values.
fn command_label(args: &[CString]) -> String {
    let command = args
        .first()
        .and_then(|arg| arg.to_str().ok())
        .unwrap_or("command");
    let target = (command == "set")
        .then(|| args.get(1).and_then(|arg| arg.to_str().ok()))
        .flatten();
    target.map_or_else(
        || command.to_owned(),
        |target| format!("{command} {target}"),
    )
}

#[cfg(test)]
mod command_sender_tests;
