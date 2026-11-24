//! Native commands and events are serialized on the event thread.
mod duration_probe;
mod folder_discovery;
mod handle;
mod hooks;
mod playlist_resume;
mod stream;
mod version_probe;
pub use duration_probe::read_local_duration;
pub use version_probe::{EngineVersions, read_engine_versions};

use self::handle::{EventRef, Handle};
use self::playlist_resume::PlaylistResume;
use super::{Message, WakeupContext, diag, record_notice, trace_shutdown};
use crate::{
    EngineError,
    auto_seek::AutoSeek,
    client_request::{Operation, Pending, Request},
    ffi::{Api, end_reason, event_id, format},
    pending::{PendingCommand, ReplyKind},
    properties::OBSERVED,
    seek_feedback::{self, SeekFeedback},
};
use riina_core::{CapturedFrame, Command, CommandPlan, NoticeSeverity, Phase, Playback};
use std::{
    collections::{BTreeMap, HashMap},
    ffi::{CString, c_int},
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
    pub(super) ready: SyncSender<Result<usize, EngineError>>,
}

impl<N: Fn(Arc<Playback>)> Start<N> {
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
            sender: callback_sender.clone(),
            queued: AtomicBool::new(false),
        }));
        // The render API may borrow the registered address. The worker owns the core.
        let _ = ready.send(Ok(handle.address()));
        let mut state = WorkerState {
            playback: Playback {
                volume: 100.0,
                speed: 1.0,
                ..Default::default()
            },
            next_observer: 1,
            ..Default::default()
        };

        if let Some(message) = error {
            record_notice(&mut state.playback, NoticeSeverity::Warning, message);
        }

        while let Some(message) = state.hooks.next_message(&receiver) {
            if accept_shutdown(&message) {
                break;
            }
            let handled = catch_unwind(AssertUnwindSafe(|| {
                match message {
                    Message::Wake => handle.acknowledge_wakeup(),
                    Message::Intent(command @ Command::ObserveMpvProperty(_)) => {
                        state.observe_dynamic(&handle, &command, &notify);
                    }
                    Message::Intent(command) => state.handle_intent(&handle, command),
                    Message::MediaIntent {
                        generation,
                        command,
                    } => {
                        state.drain_events(&mut handle, &callback_sender);
                        if state.accepts_media_intent(generation) {
                            state.handle_intent(&handle, command);
                        }
                    }
                    Message::OpenStream(command, source, attempt) => {
                        state.open_stream(&handle, command, source, attempt);
                    }
                    Message::Client(request) => {
                        if matches!(request.operation, Operation::RegisterHook(_)) {
                            state.drain_events(&mut handle, &callback_sender);
                        }
                        state.request_client(&handle, request);
                    }
                    Message::Shutdown => return,
                }
                state.drain_events(&mut handle, &callback_sender);
                state.stop_cancelled_stream(&handle);
                state.hooks.continue_ready(&handle);
                state.publish(&notify);
            }));
            if handled.is_err() {
                stopped.store(true, Ordering::Release);
                diag("The native player event worker stopped unexpectedly");
                state.playback.phase = Phase::Failed;
                state.stream = None;
                record_notice(
                    &mut state.playback,
                    NoticeSeverity::Error,
                    "The playback engine stopped unexpectedly; reopen the file".into(),
                );
                // The shared OpenGL surface can still use the core until its owner releases it.
                state.hooks.continue_all(&handle);
                state.continue_stream_auth(&handle);
                handle.stop_after_panic();
                let _ = catch_unwind(AssertUnwindSafe(|| {
                    notify(Arc::new(state.playback.clone()))
                }));
                for queued in &receiver {
                    if accept_shutdown(&queued) {
                        break;
                    }
                }
                break;
            }
        }
        state.hooks.continue_all(&handle);
        state.continue_stream_auth(&handle);
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
) -> Result<Vec<String>, EngineError> {
    for (name, value) in [
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
        let key = CString::new(name).unwrap();
        let value = CString::new(value).unwrap();
        handle
            .set_option(&key, &value)
            .map_err(|source| EngineError::RequiredStartupOption {
                name,
                source: Box::new(source),
            })?;
    }
    let mut ignored = Vec::new();
    for (key, value) in options {
        let key = CString::new(key).map_err(EngineError::StartupOption)?;
        let value = CString::new(value).map_err(EngineError::StartupOption)?;
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
    client_requests: HashMap<u64, Pending>,
    hooks: hooks::Hooks,
    volume_boost: bool,
    pending_drop_subtitles: Vec<String>,
    folder_discovery: folder_discovery::FolderDiscovery,
    playlist_resume: PlaylistResume,
    seek_pending: bool,
    event_names: HashMap<c_int, Option<Arc<str>>>,
    revision: u64,
    published_revision: Option<u64>,
    seek_feedback: SeekFeedback,
    auto_seek: AutoSeek,
    stream: Option<stream::StreamLoad>,
    active_entry: Option<i64>,
    stream_auth: stream::AuthenticationHook,
}

impl WorkerState {
    fn mark_changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    fn publish(&mut self, notify: &impl Fn(Arc<Playback>)) {
        if self.published_revision == Some(self.revision) {
            return;
        }
        self.playback.reconcile();
        notify(Arc::new(self.playback.clone()));
        self.published_revision = Some(self.revision);
    }

    fn observe_dynamic(
        &mut self,
        handle: &Handle,
        command: &Command,
        notify: &impl Fn(Arc<Playback>),
    ) {
        let Command::ObserveMpvProperty(property) = command else {
            return;
        };
        if self.observers.values().any(|observed| observed == property) {
            return;
        }
        if let Err(error) = self.add_observer(handle, command, property) {
            record_notice(&mut self.playback, NoticeSeverity::Error, error);
        }
        if self.playback.error().is_some() {
            self.mark_changed();
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
        command
            .plan(&self.playback)
            .map_err(|error| error.to_string())?;
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
        self.handle_intent_with_stream(handle, command, None);
    }

    fn accepts_media_intent(&self, generation: u64) -> bool {
        self.playback.generation == generation
            && !matches!(
                self.playback.phase,
                Phase::Loading | Phase::Stopped | Phase::Failed
            )
            && !self
                .pending
                .values()
                .any(|pending| pending.generation == generation && pending.changes_media)
    }

    fn handle_intent_with_stream(
        &mut self,
        handle: &Handle,
        command: Command,
        stream: Option<stream::StreamLoad>,
    ) {
        if command.replaces_media() {
            self.pending_drop_subtitles.clear();
        }
        let plan = match command.plan(&self.playback) {
            Ok(plan) => plan,
            Err(error) => {
                if matches!(
                    command,
                    Command::CaptureFrame | Command::CaptureThumbnailFrame
                ) {
                    self.finish_frame_request();
                }
                record_notice(&mut self.playback, NoticeSeverity::Error, error.to_string());
                self.mark_changed();
                return;
            }
        };
        if let Command::SetPauseWhenOpen(pause) = &command
            && self.playback.pause_when_open != *pause
        {
            // Changing the policy must leave the current playback state alone.
            self.playback.pause_when_open = *pause;
            self.mark_changed();
        }
        if let Command::VideoAspect(aspect) = &command {
            // The requested aspect remains visible across failure and source replacement.
            self.playback.video_aspect.clone_from(aspect);
            self.playback.aspect_requests = self.playback.aspect_requests.wrapping_add(1);
            self.mark_changed();
        }
        if matches!(command, Command::TogglePause | Command::SetPaused(_)) {
            self.seek_feedback.playback_transition();
        }
        let resume_after_navigation = plan.resume_on_load;
        if command.replaces_media() || resume_after_navigation {
            self.stream = None;
        }
        if resume_after_navigation
            || command.replaces_media()
            || matches!(command, Command::TogglePause | Command::SetPaused(_))
        {
            self.playlist_resume = PlaylistResume::Idle;
        }
        if let Err(error) = self.issue_plan(handle, &command, plan, stream) {
            if matches!(command, Command::CaptureMaterialFrame) {
                diagnostic!("Could not request a Windows video material frame: {error}");
            } else {
                record_notice(&mut self.playback, NoticeSeverity::Error, error);
                self.mark_changed();
            }
            if matches!(
                command,
                Command::CaptureFrame | Command::CaptureThumbnailFrame
            ) {
                self.finish_frame_request();
            }
        } else if let Command::OpenFilesWithSubtitles { subtitles, .. } = command {
            self.pending_drop_subtitles = subtitles;
        } else if resume_after_navigation {
            self.playlist_resume = PlaylistResume::Submitted(self.next_id);
        }
    }

    fn issue_plan(
        &mut self,
        handle: &Handle,
        command: &Command,
        plan: CommandPlan,
        mut stream: Option<stream::StreamLoad>,
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
            if let (Command::Volume(volume), [_, _, value]) = (command, args.as_mut_slice()) {
                *value = volume.min(self.volume_limit()).to_string();
            }
            if let (Command::OpenMediaLink(link), [action, property, value]) =
                (command, args.as_mut_slice())
                && action == "set"
                && property == "volume"
                && let Some(volume) = link.volume
            {
                *value = volume.min(self.volume_limit()).to_string();
            }
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
            if command.replaces_media()
                && args.first().is_some_and(|name| name == "loadfile")
                && args.get(2).is_some_and(|mode| mode == "replace")
            {
                if let Some(stream) = stream.take() {
                    self.stream = Some(stream.for_reply(self.next_id));
                }
                self.folder_discovery.submitted(self.next_id, command);
                self.playback.folder_discovery_allowed = false;
                let explicit = matches!(command, Command::OpenPlaylist(_));
                if self.playback.explicit_playlist != explicit {
                    self.playback.explicit_playlist = explicit;
                    self.mark_changed();
                }
            }
            if args.first().is_some_and(|name| name == "stop") {
                self.folder_discovery = folder_discovery::FolderDiscovery::default();
                self.playback.folder_discovery_allowed = false;
            }
            if matches!(command, Command::OpenShuffledPlaylist(_))
                && args.first().is_some_and(|name| name == "playlist-shuffle")
            {
                self.playback.explicit_playlist = true;
                self.playback.playlist_shuffled = true;
                self.mark_changed();
            }
            if rewriting_seek {
                self.auto_seek.submitted(self.next_id);
            }
            if matches!(command, Command::TogglePlaylistShuffle) {
                self.playback.playlist_shuffled = !self.playback.playlist_shuffled;
                self.mark_changed();
            } else if let Command::PluginMpv(native) = command
                && matches!(native.name(), "playlist-shuffle" | "playlist-unshuffle")
            {
                self.playback.playlist_shuffled = native.name() == "playlist-shuffle";
                self.mark_changed();
            } else if command.resets_playlist_shuffle() && self.playback.playlist_shuffled {
                self.playback.playlist_shuffled = false;
                self.mark_changed();
            }
            if seek_feedback::navigation(&args, &self.playback) {
                self.seek_feedback.submitted(self.next_id);
            }
        }
        Ok(())
    }

    fn issue(&mut self, handle: &Handle, args: &[String], kind: ReplyKind) -> Result<(), String> {
        let changes_media = changes_media(args);
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
        if let [action, property, value] = args.as_slice()
            && action.as_c_str() == c"set"
            && property.as_c_str() == c"volume-max"
        {
            // Match the volume policy sent through this FIFO before a later plugin write.
            self.volume_boost = value.as_c_str() == c"200";
        }
        self.pending.insert(
            self.next_id,
            PendingCommand {
                generation: self.playback.generation,
                label,
                kind,
                changes_media,
            },
        );
        Ok(())
    }

    fn volume_limit(&self) -> f64 {
        if self.volume_boost { 200.0 } else { 100.0 }
    }

    fn request_client(&mut self, handle: &Handle, mut request: Request) {
        const MAX_PENDING_REQUESTS: usize = 32;
        if let Operation::Read { name, .. } = &mut request.operation
            && name
                .to_str()
                .is_ok_and(|name| self.stream_auth.protects(name))
        {
            *name = c"path".to_owned();
        }
        let now = Instant::now();
        self.client_requests
            .retain(|_, pending| pending.deadline > now);
        let sent = if request.pending.deadline <= now {
            Err(EngineError::ClientTimeout)
        } else if self.client_requests.len() >= MAX_PENDING_REQUESTS {
            Err(EngineError::QueueFull)
        } else {
            request
                .apply_volume_limit(self.volume_limit())
                .and_then(|()| self.submit_client(handle, request.operation))
        };
        match sent {
            Ok(Some(id)) => {
                self.client_requests.insert(id, request.pending);
            }
            result => {
                let _ = request
                    .pending
                    .reply
                    .send(result.map(|_| serde_json::Value::Null));
            }
        }
    }

    fn submit_client(
        &mut self,
        handle: &Handle,
        operation: Operation,
    ) -> Result<Option<u64>, EngineError> {
        if let Operation::RegisterHook(registration) = operation {
            self.hooks.register(handle, registration)?;
            return Ok(None);
        }
        if let Operation::Intent(command) = operation {
            let before = self.next_id;
            self.handle_intent(handle, command);
            // The final submitted step of a typed intent acknowledges the command.
            // Every step still goes through the regular command-reply handling.
            return Ok(
                (self.next_id != before && self.pending.contains_key(&self.next_id))
                    .then_some(self.next_id),
            );
        }
        self.next_id += 1;
        handle.request_property(self.next_id, &operation)?;
        Ok(Some(self.next_id))
    }

    fn drain_events(&mut self, handle: &mut Handle, wake: &SyncSender<Message>) {
        while let Some(event) = handle.wait() {
            if let Some((hook, entry, failed)) = self.stream_auth.take_reply(&event) {
                self.finish_stream_auth(handle, hook, entry, failed);
                continue;
            }
            if matches!(
                event.id(),
                event_id::GET_PROPERTY_REPLY
                    | event_id::SET_PROPERTY_REPLY
                    | event_id::COMMAND_REPLY
            ) {
                if let Some(pending) = self.client_requests.remove(&event.userdata())
                    && pending.deadline > Instant::now()
                {
                    let _ = pending.reply.send(event.client_reply());
                }
                if event.id() != event_id::COMMAND_REPLY {
                    continue;
                }
            }
            // Native events update playback, retained plugin history, or both.
            self.mark_changed();
            let name = self.event_names.entry(event.id()).or_insert_with(|| {
                // Own the cached names since snapshots can outlive the loaded library.
                event
                    .name()
                    .map(|name| Arc::from(name.to_string_lossy().as_ref()))
            });
            if let Some(name) = name {
                self.playback.record_mpv_event(name.clone());
            }
            if let Some(id) = event.hook_id() {
                let slot = event.userdata();
                if slot == stream::AUTH_HOOK_SLOT {
                    self.authenticate_stream(handle, id);
                } else {
                    self.hooks.receive(handle, id, slot, wake);
                }
            } else if event.id() == event_id::FILE_LOADED {
                self.on_file_loaded(handle);
            } else if event.id() == event_id::PLAYBACK_RESTART {
                self.on_playback_restarted(handle);
            } else if event.id() == event_id::QUEUE_OVERFLOW {
                self.on_event(&event);
                self.continue_stream_auth(handle);
            } else {
                self.on_event(&event);
            }
        }
    }

    fn on_event(&mut self, event: &EventRef<'_>) {
        match event.id() {
            event_id::COMMAND_REPLY => {
                self.on_stream_reply(event);
                self.on_command_reply(event);
            }
            event_id::START_FILE => {
                self.active_entry = event.started_entry();
                self.stream_auth.public_address = None;
                if let Some(load) = &self.stream
                    && let Some(entry) = self.active_entry
                    && load.superseded_by(entry)
                {
                    self.stream = None;
                }
                self.seek_pending = false;
                self.seek_feedback.clear();
                self.auto_seek = AutoSeek::default();
                self.playback.begin_load();
                self.playback.folder_discovery_allowed =
                    self.folder_discovery.take_for(self.active_entry);
                self.playlist_resume.file_started(self.playback.generation);
            }
            event_id::END_FILE => self.on_end_file(event),
            event_id::SEEK => {
                self.seek_pending = true;
                self.seek_feedback.started();
                self.auto_seek.seek_started(Instant::now());
            }
            event_id::PROPERTY_CHANGE => {
                let observer = self.observers.get(&event.userdata()).cloned();
                if let Some(name) = observer.as_ref()
                    && self.stream_auth.protects(name)
                    && let Some(address) = &self.stream_auth.public_address
                {
                    Arc::make_mut(&mut self.playback.observed_mpv_properties)
                        .insert(name.clone(), serde_json::Value::String(address.clone()));
                } else {
                    event.apply_property(&mut self.playback, observer);
                }
            }
            event_id::QUEUE_OVERFLOW => {
                self.stream = None;
                self.folder_discovery = folder_discovery::FolderDiscovery::default();
                self.playback.folder_discovery_allowed = false;
                self.client_requests.clear();
                self.playlist_resume = PlaylistResume::Idle;
                record_notice(
                    &mut self.playback,
                    NoticeSeverity::Error,
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
        if self.stream.as_mut().is_some_and(|load| load.end(end)) {
            self.stream = None;
        }
        if self.active_entry == Some(end.playlist_entry_id) {
            self.active_entry = None;
            self.playback.folder_discovery_allowed = false;
        }
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
        if end.error < 0
            && let Some(message) = event.end_error(end)
        {
            record_notice(&mut self.playback, NoticeSeverity::Error, message);
        }
        self.playlist_resume
            .file_ended(self.playback.generation, end.reason == end_reason::REDIRECT);
    }

    fn on_file_loaded(&mut self, handle: &Handle) {
        self.complete_open_stream();
        self.playback.phase = Phase::Playing;
        self.playlist_resume.file_loaded(self.playback.generation);
        for path in std::mem::take(&mut self.pending_drop_subtitles) {
            let args = ["sub-add".into(), path, "auto".into()];
            if let Err(error) = self.issue(handle, &args, ReplyKind::Plain) {
                record_notice(&mut self.playback, NoticeSeverity::Error, error);
                break;
            }
        }
    }

    fn on_playback_restarted(&mut self, handle: &Handle) {
        // Only a preceding native SEEK distinguishes this from initial playback.
        if std::mem::take(&mut self.seek_pending) {
            self.playback.completed_seeks = self.playback.completed_seeks.wrapping_add(1);
            self.auto_seek.playback_restarted(Instant::now());
            if self.seek_feedback.completed() {
                let completed = &mut self.playback.completed_navigation_seeks;
                *completed = completed.wrapping_add(1);
            }
        }
        // With normal latency settings, mpv waits for the initial video frame
        // at its output before emitting this event. Audio-only media also emits it.
        if self
            .playlist_resume
            .playback_restarted(self.playback.generation)
            && !self.playback.pause_when_open
        {
            let args = ["set".into(), "pause".into(), "no".into()];
            if let Err(error) = self.issue(handle, &args, ReplyKind::Plain) {
                record_notice(&mut self.playback, NoticeSeverity::Error, error);
            }
        }
    }

    fn on_command_reply(&mut self, event: &EventRef<'_>) {
        let failed = event.failed();
        if self.folder_discovery.awaits(event.userdata()) {
            self.folder_discovery
                .replied((!failed).then(|| event.loaded_entry()).flatten());
            if self.folder_discovery.take_for(self.active_entry) {
                self.playback.folder_discovery_allowed = true;
            }
        }
        self.playlist_resume
            .command_replied(event.userdata(), failed);
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
                    record_notice(
                        &mut self.playback,
                        NoticeSeverity::Error,
                        "Screenshot completed without a saved filename".into(),
                    );
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
                if let Some(error) = event.error() {
                    let message = format!("mpv {}: {error}", pending.label);
                    record_notice(&mut self.playback, NoticeSeverity::Error, message);
                }
            }
        }
    }

    fn finish_frame_request(&mut self) {
        let finished = &mut self.playback.finished_frame_capture_requests;
        *finished = finished.wrapping_add(1);
        self.mark_changed();
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
        match kind {
            ReplyKind::IncomingTransitionCapture => state.incoming_transition_frame.store(
                frame,
                Some(state.generation),
                state.position,
                Some(Instant::now()),
            ),
            ReplyKind::MaterialCapture => state.material_frame.store(
                frame,
                Some(state.generation),
                state.position,
                Some(Instant::now()),
            ),
            ReplyKind::TransitionCapture => {
                state
                    .transition_frame
                    .store(frame, Some(generation), None, Some(Instant::now()))
            }
            ReplyKind::FrameCapture => state.captured_frame.store(frame, None, None, None),
            ReplyKind::Plain | ReplyKind::Screenshot { .. } => {}
        }
    }
}

// Keep diagnostics actionable without exposing media paths or arbitrary option values.
fn changes_media(args: &[String]) -> bool {
    let mut names = args.iter().map(String::as_str);
    match names.next() {
        Some("loadfile") => names.nth(1).is_none_or(|mode| mode != "append"),
        Some(
            "stop"
            | "playlist-next"
            | "playlist-prev"
            | "playlist-play-index"
            | "playlist-next-playlist"
            | "playlist-prev-playlist",
        ) => true,
        Some("set") => names.next() == Some("playlist-pos"),
        _ => false,
    }
}

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
mod tests {
    use super::*;

    #[test]
    fn scoped_transport_ignores_an_old_or_replacing_media_generation() {
        let mut worker = WorkerState {
            playback: Playback {
                generation: 4,
                phase: Phase::Playing,
                ..Playback::default()
            },
            ..WorkerState::default()
        };
        assert!(!worker.accepts_media_intent(3));
        assert!(worker.accepts_media_intent(4));
        for (mode, accepts) in [("append", true), ("replace", false)] {
            let args = [
                "loadfile".to_owned(),
                "media.mkv".to_owned(),
                mode.to_owned(),
            ];
            worker.pending.insert(
                1,
                PendingCommand {
                    generation: 4,
                    label: "loadfile".into(),
                    kind: ReplyKind::Plain,
                    changes_media: changes_media(&args),
                },
            );
            assert_eq!(worker.accepts_media_intent(4), accepts);
        }
        worker.pending.clear();
        worker.playback.phase = Phase::Loading;
        assert!(!worker.accepts_media_intent(4));
    }

    #[test]
    fn an_empty_drain_keeps_the_current_publication() {
        let mut worker = WorkerState::default();
        let deliveries = std::cell::RefCell::new(Vec::new());
        let notify = |state| deliveries.borrow_mut().push(state);
        worker.publish(&notify);
        worker.publish(&notify);
        worker.playback.position = Some(0.1);
        worker.mark_changed();
        worker.publish(&notify);
        worker.publish(&notify);
        let states = deliveries.into_inner();
        assert_eq!(states.len(), 2);
        assert_eq!(states[0].position, None);
        assert_eq!(states[1].position, Some(0.1));
    }
}
