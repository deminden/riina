//! Native commands and events are serialized on the event thread.
mod handle;

use self::handle::{EventRef, Handle};
use super::{Message, WakeupContext, diag, trace_shutdown};
use crate::{
    auto_seek::AutoSeek,
    ffi::{Api, end_reason, event_id, format},
    pending::{PendingCommand, ReplyKind},
    properties::OBSERVED,
    seek_feedback::{self, SeekFeedback},
};
use riina_core::{CapturedFrame, Command, CommandPlan, Phase, Playback};
use std::{
    collections::{BTreeMap, HashMap},
    ffi::CString,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, SyncSender},
    },
    time::Instant,
};

pub(super) struct Start<N> {
    pub(super) api: Api,
    pub(super) headless: bool,
    pub(super) options: Vec<(String, String)>,
    pub(super) notify: N,
    pub(super) receiver: Receiver<Message>,
    pub(super) callback_sender: SyncSender<Message>,
    pub(super) stopped: Arc<AtomicBool>,
    pub(super) ready: SyncSender<Result<usize, String>>,
}

impl<N: Fn(Playback)> Start<N> {
    pub(super) fn run(self) {
        let Self {
            api,
            headless,
            options,
            notify,
            receiver,
            callback_sender,
            stopped,
            ready,
        } = self;
        #[cfg(windows)]
        let _apartment = match crate::windows_apartment::Apartment::initialize() {
            Ok(apartment) => apartment,
            Err(error) => {
                let _ = ready.send(Err(error));
                return;
            }
        };
        let mut handle = match Handle::new(api) {
            Ok(handle) => handle,
            Err(error) => {
                let _ = ready.send(Err(error));
                return;
            }
        };
        let error = match configure(&handle, headless, options) {
            Ok(ignored) => (!ignored.is_empty())
                .then(|| format!("Some mpv options were ignored: {}", ignored.join("; "))),
            Err(error) => {
                let _ = ready.send(Err(error));
                return;
            }
        };
        handle.register_wakeup(Box::new(WakeupContext {
            sender: callback_sender,
            queued: AtomicBool::new(false),
        }));
        // The render API may borrow the registered address. The worker owns the core.
        let _ = ready.send(Ok(handle.address()));
        let mut state = WorkerState {
            playback: Playback {
                volume: 100.0,
                speed: 1.0,
                error,
                ..Default::default()
            },
            next_observer: 1,
            ..Default::default()
        };

        while let Ok(message) = receiver.recv() {
            if accept_shutdown(&message) {
                break;
            }
            let handled = catch_unwind(AssertUnwindSafe(|| {
                match message {
                    Message::Wake => handle.acknowledge_wakeup(),
                    Message::Intent(command @ Command::ObserveMpvProperty(_)) => {
                        state.observe_dynamic(&handle, &command, &notify);
                        return;
                    }
                    Message::Intent(command) => state.handle_intent(&handle, command),
                    Message::Shutdown => return,
                }
                state.drain_events(&mut handle);
                state.publish(&notify);
            }));
            if handled.is_err() {
                stopped.store(true, Ordering::Release);
                diag("The native player event worker stopped unexpectedly");
                state.playback.phase = Phase::Failed;
                state.playback.error =
                    Some("The playback engine stopped unexpectedly; reopen the file".into());
                // The shared OpenGL surface can still use the core until its owner releases it.
                handle.stop_after_panic();
                let _ = catch_unwind(AssertUnwindSafe(|| notify(state.playback.clone())));
                for queued in &receiver {
                    if accept_shutdown(&queued) {
                        break;
                    }
                }
                break;
            }
        }
    }
}

fn accept_shutdown(message: &Message) -> bool {
    if matches!(message, Message::Shutdown) {
        // Only the final owner can terminate the core after the render contexts are freed.
        trace_shutdown("shutdown accepted; terminating pending playback work");
        true
    } else {
        false
    }
}

#[expect(
    clippy::unwrap_used,
    reason = "Fixed startup option strings contain no interior NULs"
)]
fn configure(
    handle: &Handle,
    headless: bool,
    options: Vec<(String, String)>,
) -> Result<Vec<String>, String> {
    for (key, value) in [
        ("config", "no"),
        ("load-scripts", "no"),
        ("idle", "yes"),
        ("vo", if headless { "null" } else { "libmpv" }),
        ("volume-max", "100"),
        ("terminal", "no"),
        ("hwdec", "auto-safe"),
        ("keep-open", "yes"),
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
        (
            "sub-font",
            riina_core::subtitle_controls::DEFAULT_SUBTITLE_FONT,
        ),
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
        ("reset-on-next-file", "ab-loop-a,ab-loop-b"),
    ] {
        let key = CString::new(key).unwrap();
        let value = CString::new(value).unwrap();
        handle.set_option(&key, &value)?;
    }
    let mut ignored = Vec::new();
    for (key, value) in options {
        let key = CString::new(key).map_err(|error| error.to_string())?;
        let value = CString::new(value).map_err(|error| error.to_string())?;
        if let Err(error) = handle.set_option(&key, &value) {
            // IINA reports invalid optional preferences while keeping media playable.
            ignored.push(format!("{}: {error}", key.to_string_lossy()));
        }
    }
    handle.initialize()?;
    for property in OBSERVED {
        handle.observe(0, property.name, property.format)?;
    }
    Ok(ignored)
}

#[derive(Default)]
struct WorkerState {
    playback: Playback,
    next_id: u64,
    observers: BTreeMap<u64, String>,
    next_observer: u64,
    pending: HashMap<u64, PendingCommand>,
    pending_drop_subtitles: Vec<String>,
    resume_on_load: bool,
    seek_pending: bool,
    seek_feedback: SeekFeedback,
    auto_seek: AutoSeek,
}

impl WorkerState {
    fn publish(&mut self, notify: &impl Fn(Playback)) {
        self.playback.reconcile();
        notify(self.playback.clone());
    }

    fn observe_dynamic(&mut self, handle: &Handle, command: &Command, notify: &impl Fn(Playback)) {
        let Command::ObserveMpvProperty(property) = command else {
            return;
        };
        if self.observers.values().any(|observed| observed == property) {
            return;
        }
        if let Err(error) = self.add_observer(handle, command, property) {
            self.playback.error = Some(error);
        }
        if self.playback.error.is_some() {
            self.publish(notify);
        }
    }

    fn add_observer(
        &mut self,
        handle: &Handle,
        command: &Command,
        property: &str,
    ) -> Result<(), String> {
        if self.observers.len() >= riina_core::MAX_PLUGIN_OBSERVED_MPV_PROPERTIES {
            return Err(format!(
                "Player has reached the {} observed mpv property limit",
                riina_core::MAX_PLUGIN_OBSERVED_MPV_PROPERTIES
            ));
        }
        command.plan(&self.playback)?;
        let name = CString::new(property).map_err(|error| error.to_string())?;
        handle
            .observe(self.next_observer, &name, format::NODE)
            .map_err(|error| format!("Could not observe mpv property {property}: {error}"))?;
        self.observers
            .insert(self.next_observer, property.to_owned());
        self.next_observer = self.next_observer.saturating_add(1);
        Ok(())
    }

    fn handle_intent(&mut self, handle: &Handle, command: Command) {
        if command.replaces_media() {
            self.pending_drop_subtitles.clear();
        }
        let plan = match command.plan(&self.playback) {
            Ok(plan) => plan,
            Err(error) => {
                if matches!(command, Command::CaptureFrame) {
                    self.finish_frame_request();
                }
                self.playback.error = Some(error);
                return;
            }
        };
        if let Command::SetPauseWhenOpen(pause) = &command {
            // Changing the policy must leave the current playback state alone.
            self.playback.pause_when_open = *pause;
        }
        if let Command::VideoAspect(aspect) = &command {
            // The requested aspect remains visible across failure and source replacement.
            self.playback.video_aspect.clone_from(aspect);
            self.playback.aspect_requests = self.playback.aspect_requests.wrapping_add(1);
        }
        if matches!(command, Command::TogglePause | Command::SetPaused(_)) {
            self.seek_feedback.playback_transition();
        }
        if command.replaces_media() {
            self.resume_on_load = false;
        }
        self.resume_on_load |= plan.resume_on_load;
        if let Err(error) = self.issue_plan(handle, &command, plan) {
            if matches!(command, Command::CaptureMaterialFrame) {
                diagnostic!("Could not request a Windows video material frame: {error}");
            } else {
                self.playback.error = Some(error);
                self.resume_on_load = false;
            }
            if matches!(command, Command::CaptureFrame) {
                self.finish_frame_request();
            }
        } else if let Command::OpenFilesWithSubtitles { subtitles, .. } = command {
            self.pending_drop_subtitles = subtitles;
        }
    }

    fn issue_plan(
        &mut self,
        handle: &Handle,
        command: &Command,
        plan: CommandPlan,
    ) -> Result<(), String> {
        let is_auto_seek = matches!(
            command,
            Command::SeekWithType {
                seek_type: riina_core::control_preferences::SeekType::Auto,
                ..
            }
        );
        // The complete intent is planned before native submission begins.
        for mut args in plan.commands {
            let rewriting_seek = is_auto_seek && args.first().is_some_and(|arg| arg == "seek");
            if rewriting_seek && args.len() >= 3 {
                args[2] = self.auto_seek.flags().into();
            } else if seek_feedback::navigation(&args, &self.playback) {
                self.auto_seek.interrupt_measurement();
            }
            if command.restarts_decoder() {
                self.seek_feedback.clear();
                self.auto_seek.interrupt_measurement();
            }
            let reply = ReplyKind::for_command(command, args.first().map(String::as_str));
            self.issue(handle, &args, reply)?;
            if rewriting_seek {
                self.auto_seek.submitted(self.next_id);
            }
            if matches!(command, Command::TogglePlaylistShuffle) {
                self.playback.playlist_shuffled = !self.playback.playlist_shuffled;
            } else if command.resets_playlist_shuffle() {
                self.playback.playlist_shuffled = false;
            }
            if seek_feedback::navigation(&args, &self.playback) {
                self.seek_feedback.submitted(self.next_id);
            }
        }
        Ok(())
    }

    fn issue(&mut self, handle: &Handle, args: &[String], kind: ReplyKind) -> Result<(), String> {
        let args: Vec<_> = args
            .iter()
            .map(|arg| CString::new(arg.as_str()))
            .collect::<Result<_, _>>()
            .map_err(|error| error.to_string())?;
        self.next_id += 1;
        let label = command_label(&args);
        handle
            .command(self.next_id, &args)
            .map_err(|error| format!("mpv {label}: {error}"))?;
        self.pending.insert(
            self.next_id,
            PendingCommand {
                generation: self.playback.generation,
                label,
                kind,
            },
        );
        Ok(())
    }

    fn drain_events(&mut self, handle: &mut Handle) {
        while let Some(event) = handle.wait() {
            if let Some(name) = event.name() {
                self.playback
                    .record_mpv_event(name.to_string_lossy().into_owned());
            }
            if event.id() == event_id::FILE_LOADED {
                self.on_file_loaded(handle);
            } else {
                self.on_event(&event);
            }
        }
    }

    fn on_event(&mut self, event: &EventRef<'_>) {
        match event.id() {
            event_id::COMMAND_REPLY => self.on_command_reply(event),
            event_id::START_FILE => {
                self.seek_pending = false;
                self.seek_feedback.clear();
                self.auto_seek = AutoSeek::default();
                self.playback.begin_load();
            }
            event_id::END_FILE => self.on_end_file(event),
            event_id::SEEK => {
                self.seek_pending = true;
                self.seek_feedback.started();
                self.auto_seek.seek_started(Instant::now());
            }
            event_id::PLAYBACK_RESTART if self.seek_pending => {
                // Only a preceding native SEEK distinguishes this from initial playback.
                self.seek_pending = false;
                self.playback.completed_seeks = self.playback.completed_seeks.wrapping_add(1);
                self.auto_seek.playback_restarted(Instant::now());
                if self.seek_feedback.completed() {
                    let completed = &mut self.playback.completed_navigation_seeks;
                    *completed = completed.wrapping_add(1);
                }
            }
            event_id::PROPERTY_CHANGE => {
                let observer = self.observers.get(&event.userdata()).cloned();
                event.apply_property(&mut self.playback, observer);
            }
            event_id::QUEUE_OVERFLOW => {
                self.playback.error = Some(
                    "The libmpv event queue overflowed; playback state may be incomplete".into(),
                );
            }
            _ => {}
        }
    }

    fn on_end_file(&mut self, event: &EventRef<'_>) {
        let Some(end) = event.end_file() else {
            return;
        };
        self.seek_pending = false;
        self.seek_feedback.clear();
        self.auto_seek.interrupt_measurement();
        self.playback.phase = match end.reason {
            end_reason::EOF => Phase::Ended,
            end_reason::ERROR => Phase::Failed,
            _ => Phase::Stopped,
        };
        if matches!(self.playback.phase, Phase::Stopped | Phase::Failed) {
            self.playback.ab_loop_a = None;
            self.playback.ab_loop_b = None;
        }
        if end.error < 0 {
            self.playback.error = event.end_error(end);
        }
        if end.reason == end_reason::ERROR {
            self.resume_on_load = false;
        }
    }

    fn on_file_loaded(&mut self, handle: &Handle) {
        self.playback.phase = Phase::Playing;
        for path in std::mem::take(&mut self.pending_drop_subtitles) {
            let args = ["sub-add".into(), path, "auto".into()];
            if let Err(error) = self.issue(handle, &args, ReplyKind::Plain) {
                self.playback.error = Some(error);
                break;
            }
        }
        if self.resume_on_load {
            self.resume_on_load = false;
            let args = ["set".into(), "pause".into(), "no".into()];
            if let Err(error) = self.issue(handle, &args, ReplyKind::Plain) {
                self.playback.error = Some(error);
            }
        }
    }

    fn on_command_reply(&mut self, event: &EventRef<'_>) {
        let failed = event.failed();
        self.seek_feedback.replied(event.userdata(), failed);
        if failed {
            self.auto_seek.failed(event.userdata());
        }
        let Some(pending) = self
            .pending
            .remove(&event.userdata())
            .filter(|reply| reply.belongs_to(&self.playback))
        else {
            return;
        };
        if pending.kind == ReplyKind::FrameCapture {
            self.finish_frame_request();
        }
        if failed {
            self.on_failed_reply(event, pending);
            return;
        }
        match pending.kind {
            ReplyKind::Plain => {}
            ReplyKind::Screenshot { path } => {
                if let Some(path) = path.or_else(|| event.screenshot_filename()) {
                    self.playback.screenshot_file = Some(path);
                    let completed = &mut self.playback.completed_screenshots;
                    *completed = completed.wrapping_add(1);
                } else {
                    self.playback.error =
                        Some("Screenshot completed without a saved filename".into());
                }
            }
            kind => self.on_capture_reply(event, kind, pending.generation),
        }
    }

    fn on_failed_reply(&mut self, event: &EventRef<'_>, pending: PendingCommand) {
        match pending.kind {
            ReplyKind::MaterialCapture => {
                let reason = event
                    .error()
                    .unwrap_or_else(|| format!("mpv error {}", event.error_code()));
                diagnostic!("Could not capture the Windows video material frame: {reason}");
            }
            ReplyKind::FrameCapture
            | ReplyKind::TransitionCapture
            | ReplyKind::IncomingTransitionCapture => {}
            ReplyKind::Plain | ReplyKind::Screenshot { .. } => {
                self.playback.error = event
                    .error()
                    .map(|error| format!("mpv {}: {error}", pending.label));
                self.resume_on_load = false;
            }
        }
    }

    fn finish_frame_request(&mut self) {
        let finished = &mut self.playback.finished_frame_capture_requests;
        *finished = finished.wrapping_add(1);
    }

    fn on_capture_reply(&mut self, event: &EventRef<'_>, kind: ReplyKind, generation: u64) {
        let material = kind == ReplyKind::MaterialCapture;
        match event.frame() {
            None if material => {
                diagnostic!("mpv returned no result data for the Windows video material frame")
            }
            None => diagnostic!("mpv returned no frame for an asynchronous frame capture"),
            Some(Err(error)) if material => {
                diagnostic!("Could not decode the Windows video material frame: {error}")
            }
            Some(Err(error)) => {
                diagnostic!("Could not copy an asynchronous frame capture: {error}")
            }
            Some(Ok(frame)) => {
                let frame = if material {
                    match frame.downsample_to_fit(320, 180) {
                        Ok(frame) => frame,
                        Err(error) => {
                            diagnostic!("Could not reduce an asynchronous material frame: {error}");
                            return;
                        }
                    }
                } else {
                    frame
                };
                self.store_capture(kind, frame, generation);
            }
        }
    }

    fn store_capture(&mut self, kind: ReplyKind, frame: CapturedFrame, generation: u64) {
        let state = &mut self.playback;
        let frame = Arc::new(frame);
        match kind {
            ReplyKind::IncomingTransitionCapture => {
                state.incoming_transition_frame = Some(frame);
                state.incoming_transition_frame_revision =
                    state.incoming_transition_frame_revision.wrapping_add(1);
                state.incoming_transition_frame_generation = Some(state.generation);
                state.incoming_transition_frame_position = state.position;
                state.incoming_transition_frame_captured_at = Some(Instant::now());
            }
            ReplyKind::MaterialCapture => {
                state.material_frame = Some(frame);
                state.material_frame_generation = Some(state.generation);
                state.material_frame_position = state.position;
                state.material_frame_captured_at = Some(Instant::now());
                state.material_frame_revision = state.material_frame_revision.wrapping_add(1);
            }
            ReplyKind::TransitionCapture => {
                state.transition_frame = Some(frame);
                state.transition_frame_generation = Some(generation);
                state.transition_frame_revision = state.transition_frame_revision.wrapping_add(1);
                state.transition_frame_captured_at = Some(Instant::now());
            }
            ReplyKind::FrameCapture => {
                state.captured_frame = Some(frame);
                state.completed_frame_captures = state.completed_frame_captures.wrapping_add(1);
            }
            ReplyKind::Plain | ReplyKind::Screenshot { .. } => {}
        }
    }
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
