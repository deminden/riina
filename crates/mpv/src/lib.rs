//! The event worker owns the player; the GUI owns only its OpenGL render context.
#[cfg(test)]
mod demuxer_tests;
mod ffi;
mod seek_feedback;
pub mod thumbnail_generator;
pub mod video;
#[cfg(windows)]
mod windows_apartment;

use ffi::{Api, EndFile, Property};
use riina_core::{Command, Phase, Playback};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    ffi::{CStr, CString, c_void},
    path::Path,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

enum Message {
    Intent(Command),
    Wake,
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
    thread: Option<JoinHandle<()>>,
}

impl Drop for Owner {
    fn drop(&mut self) {
        // Renderers retain the owner, so this runs only after render contexts are freed.
        let _ = self.sender.send(Message::Intent(Command::Shutdown));
        trace_shutdown("shutdown intent delivered; joining event worker");
        if let Some(thread) = self.thread.take() {
            if thread.join().is_ok() {
                trace_shutdown("event worker joined");
            } else {
                eprintln!("The native player event worker panicked during shutdown");
            }
        }
    }
}

fn trace_shutdown(stage: &str) {
    if std::env::var_os("RIINA_TEST_TRACE_SHUTDOWN").is_some() {
        eprintln!("mpv shutdown: {stage}");
    }
}

#[derive(Clone)]
pub struct Player {
    owner: Rc<Owner>,
    api: Api,
    handle: usize,
}

#[derive(Clone)]
pub struct CommandSender(SyncSender<Message>);

impl CommandSender {
    pub fn send(&self, command: Command) -> Result<(), String> {
        self.0
            .try_send(Message::Intent(command))
            .map_err(|e| format!("Player command was not accepted: {e}"))
    }

    /// The pause click toggles the worker's latest native state without blocking the UI.
    pub fn toggle_pause(&self) -> Result<(), String> {
        self.send_without_blocking(Command::TogglePause)
    }

    /// Mouse preferences may request an absolute pause state; retain it under queue pressure too.
    pub fn set_paused(&self, paused: bool) -> Result<(), String> {
        self.send_without_blocking(Command::SetPaused(paused))
    }

    fn send_without_blocking(&self, command: Command) -> Result<(), String> {
        let message = Message::Intent(command);
        match self.0.try_send(message) {
            Ok(()) => Ok(()),
            Err(mpsc::TrySendError::Disconnected(_)) => {
                Err("Player command was not accepted: playback worker has stopped".into())
            }
            Err(mpsc::TrySendError::Full(message)) => {
                let sender = self.0.clone();
                std::thread::Builder::new()
                    .name("mpv-command-delivery".into())
                    .spawn(move || {
                        let _ = sender.send(message);
                    })
                    .map(|_| ())
                    .map_err(|error| format!("Could not queue the playback action: {error}"))
            }
        }
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
                    for (name, format) in [("time-pos", 5), ("duration", 5), ("pause", 3),
                        ("paused-for-cache", 3), ("seeking", 3), ("seekable", 3),
                        ("ignore-path-in-watch-later-config", 3), ("save-position-on-quit", 3),
                        ("volume", 5), ("mute", 3), ("speed", 5), ("media-title", 1), ("mpv-version", 1), ("ffmpeg-version", 1), ("eof-reached", 3),
                        ("current-ao", 1), ("audio-device", 1), ("hwdec-current", 1), ("hwdec", 1),
                        ("vf", 6), ("af", 6), ("track-list", 6), ("chapter-list", 6), ("playlist", 6), ("audio-device-list", 6), ("video-params", 6), ("video-target-params", 6), ("metadata", 6),
                        ("video-aspect-override", 5), ("video-rotate", 4),
                        ("deinterlace", 1), ("deinterlace-active", 3),
                        ("video-frame-info", 6),
                        ("video-out-params", 6),
                        ("brightness", 4), ("contrast", 4), ("saturation", 4), ("gamma", 4), ("hue", 4),
                        ("audio-delay", 5), ("sub-delay", 5), ("sub-visibility", 3),
                        ("sub-scale", 5), ("sub-pos", 5), ("sub-font-size", 5), ("sub-outline-size", 5),
                        ("sub-blur", 5), ("sub-spacing", 5), ("sub-shadow-offset", 5),
                        ("sub-color", 1), ("sub-outline-color", 1), ("sub-back-color", 1),
                        ("sub-bold", 3), ("sub-italic", 3),
                        ("sub-font", 1),
                        ("secondary-sub-visibility", 3), ("secondary-sub-text", 1),
                        ("secondary-sub-delay", 5),
                        ("secondary-sub-pos", 5),
                        ("sub-text", 1), ("loop-file", 1), ("loop-playlist", 1),
                        ("ab-loop-a", 6), ("ab-loop-b", 6)] {
                        let name = CString::new(name).unwrap();
                        worker_api.check((worker_api.observe)(handle, 0, name.as_ptr(), format))?;
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
                let mut pending_screenshots = HashMap::new();
                let mut pending_frame_captures = HashSet::new();
                let mut pending_transition_captures = HashSet::new();
                let mut pending_incoming_transition_captures = HashSet::new();
                let mut pending_material_captures = HashSet::new();
                let mut pending_drop_subtitles = Vec::<String>::new();
                let mut resume_on_load = false;
                let mut seek_pending = false;
                let mut seek_feedback = seek_feedback::SeekFeedback::default();
                let mut auto_seek_exact = true;
                let mut auto_seek_measurement_armed = false;
                let mut auto_seek_measurement_started = None;
                let mut auto_seek_decided = false;
                while let Ok(message) = receiver.recv() {
                    if matches!(message, Message::Wake) {
                        callback.queued.store(false, Ordering::Release);
                    }
                    if let Message::Intent(command) = message {
                        if command == Command::Shutdown {
                            // Termination must start before waiting for pending work:
                            // frame/screenshot commands can require playback progress.
                            // mpv_terminate_destroy owns quitting and joining the core;
                            // a separate wait_async_requests here can stall that quit.
                            trace_shutdown("shutdown accepted; terminating pending playback work");
                            break;
                        }
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
                                                    6,
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
                            continue;
                        }
                        if matches!(command, Command::Load(_) | Command::LoadAtPosition { .. } | Command::OpenFiles(_) | Command::OpenFilesWithSubtitles { .. } | Command::Stop) {
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
                                if matches!(command, Command::Load(_) | Command::LoadAtPosition { .. } | Command::OpenFiles(_) | Command::OpenFilesWithSubtitles { .. } | Command::Stop) { resume_on_load = false; }
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
                                    if is_auto_seek && !auto_seek_decided
                                        && args.first().is_some_and(|argument| argument == "seek")
                                        && args.len() >= 3
                                    {
                                        // IINA starts Auto seeking exactly, then chooses one mode
                                        // for the remainder of this media after measuring its
                                        // first SEEK-to-PLAYBACK_RESTART interval.
                                        args[2] = if auto_seek_exact {
                                            "relative+exact".into()
                                        } else {
                                            "relative".into()
                                        };
                                    }
                                    // Decoder option changes can produce their own SEEK /
                                    // RESTART pair. They replace pending navigation feedback,
                                    // while the raw native completion counter remains intact.
                                    if matches!(command, Command::HardwareDecoding(_) | Command::VideoAspect(_) | Command::VideoRotation(_) | Command::Deinterlace(_) | Command::ToggleDeinterlace | Command::VideoCrop(_) | Command::VideoCropRectangle(_) | Command::VideoReflection { .. } | Command::ToggleVideoReflection(_) | Command::EnterCrop(_) | Command::ExitCrop { .. } | Command::Load(_) | Command::LoadAtPosition { .. } | Command::OpenFiles(_) | Command::OpenFilesWithSubtitles { .. } | Command::Stop) {
                                        seek_feedback.clear();
                                    }
                                    if let Err(error) = issue(&worker_api, handle, &args, &mut next_id, &mut pending, state.generation) {
                                        if matches!(command, Command::CaptureMaterialFrame) {
                                            eprintln!("Could not request a Windows video material frame: {error}");
                                        } else {
                                            state.error = Some(error);
                                            resume_on_load = false;
                                        }
                                        command_issue_succeeded = false;
                                        break;
                                    }
                                    if is_auto_seek && !auto_seek_decided {
                                        auto_seek_measurement_armed = true;
                                    }
                                    if matches!(command, Command::TogglePlaylistShuffle) {
                                        state.playlist_shuffled = !state.playlist_shuffled;
                                    } else if matches!(
                                        command,
                                        Command::SortPlaylist(_)
                                            | Command::MovePlaylistEntry { .. }
                                            | Command::MovePlaylistId { .. }
                                            | Command::RemovePlaylistEntry(_)
                                            | Command::RemovePlaylistId(_)
                                            | Command::RemovePlaylistIds(_)
                                            | Command::ClearPlaylist
                                            | Command::AppendFiles(_)
                                            | Command::OpenFiles(_)
                                            | Command::OpenFilesWithSubtitles { .. }
                                    ) {
                                        state.playlist_shuffled = false;
                                    }
                                    if seek_feedback::navigation(&args, &state) { seek_feedback.submitted(next_id); }
                                    if let Command::ScreenshotToFile { path, .. } = &command {
                                        pending_screenshots.insert(next_id, Some(path.clone()));
                                    } else if matches!(command, Command::Screenshot { .. })
                                        && args.first().is_some_and(|name| name == "screenshot") {
                                        pending_screenshots.insert(next_id, None);
                                    } else if matches!(command, Command::CaptureFrame) {
                                        pending_frame_captures.insert(next_id);
                                    } else if matches!(command, Command::CaptureTransitionFrame) {
                                        pending_transition_captures.insert(next_id);
                                    } else if matches!(command, Command::CaptureIncomingTransitionFrame) {
                                        pending_incoming_transition_captures.insert(next_id);
                                    } else if matches!(command, Command::CaptureMaterialFrame) {
                                        pending_material_captures.insert(next_id);
                                    }
                                }
                                if command_issue_succeeded
                                    && let Some(subtitles) = dropped_subtitles
                                {
                                    pending_drop_subtitles = subtitles;
                                }
                            }
                            Err(error) => state.error = Some(error),
                        }
                    }
                    loop {
                        let event = &*(worker_api.wait)(handle, 0.0);
                        if event.id == 0 { break; }
                        let event_name = (worker_api.event_name)(event.id);
                        if !event_name.is_null() {
                            state.record_mpv_event(
                                CStr::from_ptr(event_name).to_string_lossy().into_owned(),
                            );
                        }
                        match event.id {
                            5 => {
                                seek_feedback.replied(event.userdata, event.error < 0);
                                let screenshot = pending_screenshots.remove(&event.userdata);
                                let frame_capture = pending_frame_captures.remove(&event.userdata);
                                let transition_capture =
                                    pending_transition_captures.remove(&event.userdata);
                                let incoming_transition_capture = pending_incoming_transition_captures
                                    .remove(&event.userdata);
                                let material_capture = pending_material_captures.remove(&event.userdata);
                                if let Some(pending_command) = pending.remove(&event.userdata)
                                    && (pending_command.generation == state.generation
                                        || (transition_capture
                                            && state.generation.saturating_sub(
                                                pending_command.generation,
                                            ) <= 1
                                            && state
                                                .transition_frame_generation
                                                .is_none_or(|generation| {
                                                    generation <= pending_command.generation
                                                }))) {
                                    if event.error < 0 {
                                        if material_capture {
                                            let reason = worker_api
                                                .check(event.error)
                                                .err()
                                                .unwrap_or_else(|| format!("mpv error {}", event.error));
                                            eprintln!("Could not capture the Windows video material frame: {reason}");
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
                                                eprintln!("mpv returned no result data for the Windows video material frame");
                                            } else {
                                                eprintln!("mpv returned no frame for an asynchronous frame capture");
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
                                                            eprintln!("Could not reduce an asynchronous material frame: {error}");
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
                                                    eprintln!("Could not decode the Windows video material frame: {error}");
                                                }
                                                Err(error) => eprintln!("Could not copy an asynchronous frame capture: {error}"),
                                            }
                                        }
                                    } else if let Some(path) = screenshot {
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
                            6 => {
                                seek_pending = false;
                                seek_feedback.clear();
                                auto_seek_exact = true;
                                auto_seek_measurement_armed = false;
                                auto_seek_measurement_started = None;
                                auto_seek_decided = false;
                                state.begin_load();
                            },
                            7 if !event.data.is_null() => {
                                let end = &*event.data.cast::<EndFile>();
                                seek_pending = false;
                                seek_feedback.clear();
                                auto_seek_measurement_armed = false;
                                auto_seek_measurement_started = None;
                                state.phase = match end.reason { 0 => Phase::Ended, 4 => Phase::Failed, _ => Phase::Stopped };
                                if matches!(state.phase, Phase::Stopped | Phase::Failed) {
                                    state.ab_loop_a = None;
                                    state.ab_loop_b = None;
                                }
                                if end.error < 0 { state.error = worker_api.check(end.error).err(); }
                                if end.reason == 4 { resume_on_load = false; }
                            }
                            8 => {
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
                                    ) {
                                        state.error = Some(error);
                                        break;
                                    }
                                }
                                if resume_on_load {
                                    resume_on_load = false;
                                    let args = ["set".into(), "pause".into(), "no".into()];
                                    if let Err(error) = issue(&worker_api, handle, &args, &mut next_id, &mut pending, state.generation) { state.error = Some(error); }
                                }
                            },
                            20 => {
                                seek_pending = true;
                                seek_feedback.started();
                                if auto_seek_measurement_armed
                                    && auto_seek_measurement_started.is_none()
                                {
                                    auto_seek_measurement_armed = false;
                                    auto_seek_measurement_started = Some(Instant::now());
                                }
                            },
                            21 if seek_pending => {
                                // Restart also occurs on initial load. Only a preceding
                                // native SEEK establishes a completed seek notification.
                                seek_pending = false;
                                state.completed_seeks = state.completed_seeks.wrapping_add(1);
                                if let Some(started) = auto_seek_measurement_started.take() {
                                    auto_seek_exact = started.elapsed() < Duration::from_millis(50);
                                    auto_seek_decided = true;
                                }
                                if seek_feedback.completed() {
                                    state.completed_navigation_seeks = state.completed_navigation_seeks.wrapping_add(1);
                                }
                            }
                            22 if !event.data.is_null() => {
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
                                            eprintln!(
                                                "Could not copy observed mpv plugin property: {error}"
                                            );
                                        }
                                    }
                                }
                                apply_property(&mut state, property);
                            }
                            24 => { state.error = Some("The libmpv event queue overflowed; playback state may be incomplete".into()); }
                            _ => {}
                        }
                    }
                    state.reconcile();
                    notify(state.clone());
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
                thread: Some(thread),
            }),
            api,
            handle,
        })
    }

    pub fn send(&self, command: Command) -> Result<(), String> {
        self.owner
            .sender
            .try_send(Message::Intent(command))
            .map_err(|e| format!("Player command was not accepted: {e}"))
    }

    pub fn command_sender(&self) -> CommandSender {
        CommandSender(self.owner.sender.clone())
    }
}

unsafe fn issue(
    api: &Api,
    handle: *mut c_void,
    args: &[String],
    next_id: &mut u64,
    pending: &mut HashMap<u64, PendingCommand>,
    generation: u64,
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
    pending.insert(*next_id, PendingCommand { generation, label });
    Ok(())
}

struct PendingCommand {
    generation: u64,
    label: String,
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

fn decode_audio_devices(
    value: serde_json::Value,
) -> Result<Vec<riina_core::audio_preferences::OutputDevice>, String> {
    let devices: Vec<riina_core::audio_preferences::OutputDevice> =
        serde_json::from_value(value).map_err(|error| error.to_string())?;
    if devices.iter().any(|device| {
        device.name.is_empty() || device.name.contains('\0') || device.description.contains('\0')
    }) {
        return Err("mpv returned an invalid audio device entry".into());
    }
    Ok(devices)
}

unsafe fn observed_property_value(property: &Property) -> Result<serde_json::Value, String> {
    if property.data.is_null() {
        return Ok(serde_json::Value::Null);
    }
    if property.format != 6 {
        return Err(format!(
            "mpv returned format {} instead of a generic node",
            property.format
        ));
    }
    // SAFETY: MPV_FORMAT_NODE selects a live node owned by the current wait_event.
    // SAFETY: MPV_FORMAT_NODE selects the live node arm and the callback copies
    // it before the current mpv_wait_event result is replaced.
    let node = unsafe { &*property.data.cast::<ffi::Node>() };
    let value = unsafe { node.copy_json()? };
    let encoded = serde_json::to_vec(&value).map_err(|error| error.to_string())?;
    if encoded.len() > 1024 * 1024 {
        return Err("mpv property value exceeds the 1 MiB plugin event limit".into());
    }
    Ok(value)
}

fn metadata_value(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .as_object()?
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(key))?
        .1
        .as_str()
        .map(str::to_owned)
}

unsafe fn apply_property(state: &mut Playback, property: &Property) {
    // SAFETY: caller processes the event before asking mpv for another one. Format
    // tags are checked before dereferencing; unavailable properties remain unknown.
    let name = unsafe { CStr::from_ptr(property.name) }.to_bytes();
    if property.data.is_null() {
        if let Some(option) = riina_core::video_controls::Equalizer::ALL
            .into_iter()
            .find(|option| option.property().as_bytes() == name)
        {
            state.video_equalizer[option.index()] = None;
        }
        match name {
            b"time-pos" => state.position = None,
            b"duration" => state.duration = None,
            b"mpv-version" => state.mpv_version = None,
            b"ffmpeg-version" => state.ffmpeg_version = None,
            b"ignore-path-in-watch-later-config" => state.ignore_path_in_watch_later_config = None,
            b"save-position-on-quit" => state.save_position_on_quit = None,
            b"track-list" => state.tracks.clear(),
            b"chapter-list" => state.chapters.clear(),
            b"playlist" => state.playlist.clear(),
            b"metadata" => {
                state.artist = None;
                state.album = None;
            }
            b"audio-device-list" => state.audio_devices.clear(),
            b"sub-text" => state.subtitle_text = None,
            b"secondary-sub-text" => state.secondary_subtitle_text = None,
            b"secondary-sub-visibility" => state.secondary_subtitle_visible = None,
            b"secondary-sub-delay" => state.secondary_subtitle_delay = None,
            b"secondary-sub-pos" => state.secondary_subtitle_position = None,
            b"sub-scale" => state.subtitle_scale = None,
            b"sub-font-size" => state.subtitle_font_size = None,
            b"sub-font" => state.subtitle_font = None,
            b"sub-outline-size" => state.subtitle_outline_size = None,
            b"sub-color" => state.subtitle_text_color = None,
            b"sub-outline-color" => state.subtitle_border_color = None,
            b"sub-back-color" => state.subtitle_shadow_color = None,
            b"sub-bold" => state.subtitle_bold = None,
            b"sub-italic" => state.subtitle_italic = None,
            b"sub-blur" => state.subtitle_blur = None,
            b"sub-spacing" => state.subtitle_spacing = None,
            b"sub-shadow-offset" => state.subtitle_shadow_offset = None,
            b"sub-pos" => state.subtitle_position = None,
            b"video-params" => state.video = None,
            b"video-target-params" => state.video_target = None,
            b"video-out-params" => state.video_output = None,
            b"vf" => state.video_filters = None,
            b"af" => {
                state.audio_equalizer = None;
                state.audio_filters = None;
            }
            b"video-aspect-override" => state.video_aspect_override = None,
            b"video-rotate" => state.video_rotation = None,
            b"deinterlace" => state.deinterlace = None,
            b"deinterlace-active" => state.deinterlace_active = None,
            b"video-frame-info" => state.video_frame_info = None,
            b"hwdec-current" => state.hardware_decoder = None,
            b"hwdec" => state.hardware_decoding = None,
            b"current-ao" => state.audio_output = None,
            b"audio-device" => state.audio_device = None,
            b"loop-file" => state.loop_file = None,
            b"loop-playlist" => state.loop_playlist = None,
            b"ab-loop-a" => state.ab_loop_a = None,
            b"ab-loop-b" => state.ab_loop_b = None,
            _ => {}
        }
        return;
    }
    if property.format == 6 {
        let node = unsafe { &*property.data.cast::<ffi::Node>() };
        let result = unsafe { node.copy_json() }.and_then(|value| {
            match name {
                b"ab-loop-a" | b"ab-loop-b" => {
                    // Native values are either a numeric timestamp or "no".
                    // Unknown/unavailable points must not become a zero marker.
                    let point = value
                        .as_f64()
                        .filter(|point| point.is_finite() && *point >= 0.0);
                    if name == b"ab-loop-a" {
                        state.ab_loop_a = point;
                    } else {
                        state.ab_loop_b = point;
                    }
                }
                b"track-list" => {
                    state.tracks = serde_json::from_value(value).map_err(|e| e.to_string())?;
                    state.track_list_observed = true;
                }
                b"vf" => {
                    state.video_filters =
                        Some(serde_json::from_value(value).map_err(|e| e.to_string())?);
                }
                b"af" => {
                    let raw_filters: Vec<serde_json::Value> =
                        serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
                    let filters: Vec<riina_core::crop::VideoFilter> =
                        serde_json::from_value(value).map_err(|e| e.to_string())?;
                    state.audio_equalizer =
                        riina_core::audio_equalizer::observed_gains(&raw_filters);
                    state.audio_filters = Some(filters);
                }
                b"chapter-list" => {
                    state.chapters = serde_json::from_value(value).map_err(|e| e.to_string())?
                }
                b"playlist" => {
                    state.playlist = serde_json::from_value(value).map_err(|e| e.to_string())?
                }
                b"audio-device-list" => state.audio_devices = decode_audio_devices(value)?,
                b"metadata" => {
                    state.artist = metadata_value(&value, "artist");
                    state.album = metadata_value(&value, "album");
                }
                b"video-frame-info" => {
                    state.video_frame_info =
                        if value.is_null() || value.as_object().is_some_and(|map| map.is_empty()) {
                            None
                        } else {
                            Some(serde_json::from_value(value).map_err(|e| e.to_string())?)
                        };
                }
                b"video-params" | b"video-target-params" | b"video-out-params" => {
                    // The native VO target describes presentation, while source
                    // parameters can still carry a PQ/HLG transfer function.
                    let destination = if name == b"video-params" {
                        &mut state.video
                    } else if name == b"video-target-params" {
                        &mut state.video_target
                    } else {
                        &mut state.video_output
                    };
                    *destination =
                        if value.is_null() || value.as_object().is_some_and(|map| map.is_empty()) {
                            None
                        } else {
                            Some(serde_json::from_value(value).map_err(|e| e.to_string())?)
                        };
                }
                _ => {}
            }
            Ok(())
        });
        if let Err(error) = result {
            state.error = Some(format!("Cannot decode mpv property: {error}"));
        }
    } else if property.format == 4 {
        let value = unsafe { *property.data.cast::<i64>() };
        if name == b"video-rotate" {
            state.video_rotation = Some(value);
        }
        if let Some(option) = riina_core::video_controls::Equalizer::ALL
            .into_iter()
            .find(|option| option.property().as_bytes() == name)
        {
            // Unavailable or invalid native values must not masquerade as a
            // neutral setting in controls that would overwrite an unknown state.
            state.video_equalizer[option.index()] = (-100..=100).contains(&value).then_some(value);
        }
    } else if property.format == 5 {
        let value = unsafe { *property.data.cast::<f64>() };
        if !value.is_finite() {
            match name {
                b"video-aspect-override" => state.video_aspect_override = None,
                b"sub-scale" => state.subtitle_scale = None,
                b"sub-font-size" => state.subtitle_font_size = None,
                b"sub-outline-size" => state.subtitle_outline_size = None,
                b"sub-blur" => state.subtitle_blur = None,
                b"sub-spacing" => state.subtitle_spacing = None,
                b"sub-shadow-offset" => state.subtitle_shadow_offset = None,
                b"sub-pos" => state.subtitle_position = None,
                b"secondary-sub-delay" => state.secondary_subtitle_delay = None,
                b"secondary-sub-pos" => state.secondary_subtitle_position = None,
                _ => {}
            }
            return;
        }
        match name {
            b"time-pos" => state.position = (value >= 0.0).then_some(value),
            b"duration" => state.duration = (value >= 0.0).then_some(value),
            b"volume" => state.volume = value,
            b"speed" => state.speed = value,
            b"video-aspect-override" => state.video_aspect_override = Some(value),
            b"audio-delay" => state.audio_delay = value,
            b"sub-delay" => state.subtitle_delay = value,
            b"secondary-sub-delay" => state.secondary_subtitle_delay = Some(value),
            b"secondary-sub-pos" => {
                state.secondary_subtitle_position = (0.0..=150.0).contains(&value).then_some(value)
            }
            b"sub-scale" => state.subtitle_scale = (0.0..=100.0).contains(&value).then_some(value),
            b"sub-font-size" => state.subtitle_font_size = (value > 0.0).then_some(value),
            b"sub-outline-size" => state.subtitle_outline_size = (value >= 0.0).then_some(value),
            b"sub-blur" => state.subtitle_blur = (0.0..=20.0).contains(&value).then_some(value),
            b"sub-spacing" => state.subtitle_spacing = Some(value),
            b"sub-shadow-offset" => state.subtitle_shadow_offset = (value >= 0.0).then_some(value),
            b"sub-pos" => state.subtitle_position = (0.0..=150.0).contains(&value).then_some(value),
            _ => {}
        }
    } else if property.format == 3 {
        let flag = unsafe { *property.data.cast::<i32>() } != 0;
        match name {
            b"pause" => state.paused = flag,
            b"ignore-path-in-watch-later-config" => {
                state.ignore_path_in_watch_later_config = Some(flag)
            }
            b"save-position-on-quit" => state.save_position_on_quit = Some(flag),
            b"paused-for-cache" => state.buffering = flag,
            b"seeking" => state.seeking = flag,
            b"seekable" => state.seekable = flag,
            b"mute" => state.muted = flag,
            b"sub-visibility" => state.subtitle_visible = flag,
            b"secondary-sub-visibility" => state.secondary_subtitle_visible = Some(flag),
            b"sub-bold" => state.subtitle_bold = Some(flag),
            b"sub-italic" => state.subtitle_italic = Some(flag),
            b"deinterlace-active" => state.deinterlace_active = Some(flag),
            b"eof-reached" => {
                if flag {
                    state.phase = Phase::Ended;
                } else if state.phase == Phase::Ended {
                    state.phase = Phase::Playing;
                }
            }
            _ => {}
        }
    } else if property.format == 1 {
        let text = unsafe { *property.data.cast::<*const std::ffi::c_char>() };
        if !text.is_null() {
            let value = unsafe { CStr::from_ptr(text) }
                .to_string_lossy()
                .into_owned();
            match name {
                b"media-title" => state.title = value,
                b"mpv-version" => state.mpv_version = Some(value),
                b"ffmpeg-version" => state.ffmpeg_version = Some(value),
                b"current-ao" => state.audio_output = Some(value),
                b"audio-device" => state.audio_device = Some(value),
                b"hwdec-current" => state.hardware_decoder = Some(value),
                b"hwdec" => state.hardware_decoding = Some(value),
                b"deinterlace" => {
                    state.deinterlace = match value.as_str() {
                        "yes" => Some(true),
                        "no" => Some(false),
                        _ => None,
                    }
                }
                b"sub-text" => state.subtitle_text = Some(value),
                b"sub-font" => {
                    state.subtitle_font =
                        riina_core::subtitle_controls::valid_font_name(&value).then_some(value);
                }
                b"sub-color" => {
                    state.subtitle_text_color =
                        riina_core::subtitle_color::SubtitleColor::from_mpv(&value)
                }
                b"sub-outline-color" => {
                    state.subtitle_border_color =
                        riina_core::subtitle_color::SubtitleColor::from_mpv(&value)
                }
                b"sub-back-color" => {
                    state.subtitle_shadow_color =
                        riina_core::subtitle_color::SubtitleColor::from_mpv(&value)
                }
                b"secondary-sub-text" => state.secondary_subtitle_text = Some(value),
                b"loop-file" => state.loop_file = Some(value),
                b"loop-playlist" => state.loop_playlist = Some(value),
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod version_property_tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn loaded_engine_version_is_observed_and_can_become_unavailable() {
        let name = CString::new("mpv-version").unwrap();
        let version = CString::new("mpv 0.40.0-123-gabcdef").unwrap();
        let mut data = version.as_ptr();
        let property = Property {
            name: name.as_ptr(),
            format: 1,
            data: (&mut data as *mut *const std::ffi::c_char).cast(),
        };
        let mut playback = Playback::default();
        // SAFETY: both pointers remain valid for the complete property copy.
        unsafe { apply_property(&mut playback, &property) };
        assert_eq!(
            playback.mpv_version.as_deref(),
            Some("mpv 0.40.0-123-gabcdef")
        );

        let unavailable = Property {
            name: name.as_ptr(),
            format: 1,
            data: std::ptr::null_mut(),
        };
        // SAFETY: the null event is valid and does not dereference data.
        unsafe { apply_property(&mut playback, &unavailable) };
        assert_eq!(playback.mpv_version, None);
    }

    #[test]
    fn watch_later_path_policy_is_observed_and_can_become_unavailable() {
        let name = CString::new("ignore-path-in-watch-later-config").unwrap();
        let mut flag = 1_i32;
        let property = Property {
            name: name.as_ptr(),
            format: 3,
            data: (&mut flag as *mut i32).cast(),
        };
        let mut playback = Playback::default();
        // SAFETY: the flag remains valid for the complete property copy.
        unsafe { apply_property(&mut playback, &property) };
        assert_eq!(playback.ignore_path_in_watch_later_config, Some(true));

        let unavailable = Property {
            name: name.as_ptr(),
            format: 3,
            data: std::ptr::null_mut(),
        };
        // SAFETY: the null event is valid and does not dereference data.
        unsafe { apply_property(&mut playback, &unavailable) };
        assert_eq!(playback.ignore_path_in_watch_later_config, None);
    }

    #[test]
    fn save_position_policy_is_observed_and_can_become_unavailable() {
        let name = CString::new("save-position-on-quit").unwrap();
        let mut flag = 1_i32;
        let property = Property {
            name: name.as_ptr(),
            format: 3,
            data: (&mut flag as *mut i32).cast(),
        };
        let mut playback = Playback::default();
        // SAFETY: the flag remains valid for the complete property copy.
        unsafe { apply_property(&mut playback, &property) };
        assert_eq!(playback.save_position_on_quit, Some(true));

        let unavailable = Property {
            name: name.as_ptr(),
            format: 3,
            data: std::ptr::null_mut(),
        };
        // SAFETY: the null event is valid and does not dereference data.
        unsafe { apply_property(&mut playback, &unavailable) };
        assert_eq!(playback.save_position_on_quit, None);
    }
}

#[cfg(test)]
mod audio_device_property_tests {
    use super::*;

    #[test]
    fn observed_audio_devices_keep_engine_ids_and_descriptions() {
        let devices = decode_audio_devices(serde_json::json!([
            {"name": "auto", "description": "Autoselect device"},
            {"name": "wasapi/{endpoint}", "description": "Speakers"}
        ]))
        .unwrap();
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[1].name, "wasapi/{endpoint}");
        assert_eq!(devices[1].description, "Speakers");
    }

    #[test]
    fn malformed_native_audio_device_entries_are_rejected() {
        assert!(
            decode_audio_devices(serde_json::json!([
                {"name": "", "description": "No identifier"}
            ]))
            .is_err()
        );
        assert!(
            decode_audio_devices(serde_json::json!([
                {"name": "wasapi/device\u{0000}", "description": "Bad ID"}
            ]))
            .is_err()
        );
    }
}

#[cfg(test)]
mod metadata_property_tests {
    use super::metadata_value;

    #[test]
    fn metadata_fields_are_read_case_insensitively_and_reject_non_text_values() {
        let metadata = serde_json::json!({
            "ARTIST": "Björk",
            "album": "Debut",
            "track": 7
        });
        assert_eq!(
            metadata_value(&metadata, "artist").as_deref(),
            Some("Björk")
        );
        assert_eq!(metadata_value(&metadata, "ALBUM").as_deref(), Some("Debut"));
        assert_eq!(metadata_value(&metadata, "track"), None);
        assert_eq!(metadata_value(&metadata, "title"), None);
    }
}

#[cfg(test)]
mod dynamic_property_tests {
    use super::*;

    #[test]
    fn plugin_observed_node_properties_are_copied_as_json() {
        let name = CString::new("cache-buffering-state").unwrap();
        let mut node = ffi::Node {
            value: ffi::NodeValue { integer: 42 },
            format: 4,
        };
        let property = Property {
            name: name.as_ptr(),
            format: 6,
            data: (&mut node as *mut ffi::Node).cast(),
        };
        // SAFETY: the node and name remain valid while the helper copies them.
        assert_eq!(
            unsafe { observed_property_value(&property) }.unwrap(),
            serde_json::json!(42)
        );
    }

    #[test]
    fn unavailable_plugin_observed_properties_become_json_null() {
        let name = CString::new("cache-buffering-state").unwrap();
        let property = Property {
            name: name.as_ptr(),
            format: 6,
            data: std::ptr::null_mut(),
        };
        // SAFETY: null property data is explicitly represented as JSON null.
        assert_eq!(
            unsafe { observed_property_value(&property) }.unwrap(),
            serde_json::Value::Null
        );
    }
}

#[cfg(test)]
mod command_sender_tests {
    use super::{Command, CommandSender, Message, WakeupContext};
    use std::sync::{atomic::Ordering, mpsc};

    #[test]
    fn pause_toggle_returns_promptly_and_is_delivered_after_queue_pressure() {
        let (sender, receiver) = mpsc::sync_channel(1);
        sender.try_send(Message::Wake).unwrap();
        let command_sender = CommandSender(sender);
        command_sender.toggle_pause().unwrap();

        assert!(matches!(receiver.recv().unwrap(), Message::Wake));
        assert!(matches!(
            receiver.recv().unwrap(),
            Message::Intent(Command::TogglePause)
        ));
    }

    #[test]
    fn native_wake_notifications_share_one_queued_message() {
        let (sender, receiver) = mpsc::sync_channel(64);
        let wakeup = WakeupContext {
            sender,
            queued: std::sync::atomic::AtomicBool::new(false),
        };
        for _ in 0..100 {
            wakeup.signal();
        }
        assert!(matches!(receiver.try_recv(), Ok(Message::Wake)));
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn full_intent_queue_does_not_leave_wakeup_permanently_armed() {
        let (sender, receiver) = mpsc::sync_channel(1);
        sender.try_send(Message::Intent(Command::Stop)).unwrap();
        let wakeup = WakeupContext {
            sender,
            queued: std::sync::atomic::AtomicBool::new(false),
        };
        wakeup.signal();
        assert!(!wakeup.queued.load(Ordering::Acquire));
        assert!(matches!(
            receiver.recv(),
            Ok(Message::Intent(Command::Stop))
        ));

        wakeup.signal();
        assert!(matches!(receiver.recv(), Ok(Message::Wake)));
    }
}
