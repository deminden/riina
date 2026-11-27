//! Native commands and events are serialized on the event thread.
mod audio_route;
mod duration_probe;
pub(crate) mod filter_edits;
mod folder_discovery;
mod handle;
mod hooks;
mod load_resume;
mod local_probe;
mod playlist_end_pause;
mod playlist_shuffle;
mod screenshot_decode;
mod script_input;
mod stream;
mod subtitle_reload;
mod timeline_queue;
mod version_probe;
pub use duration_probe::read_local_duration;
pub use screenshot_decode::decode_screenshot;
pub use version_probe::{EngineVersions, read_engine_versions};

use self::handle::{EventRef, Handle};
use self::load_resume::Resume;
use super::{Message, WakeupContext, diag, record_notice, trace_shutdown};
use crate::{
    EngineError, MpvPropertyFormat, StartupOptions,
    auto_seek::AutoSeek,
    client_request::{Operation, Pending, Request},
    ffi::{Api, end_reason, event_id, format},
    pending::{PendingCommand, ReplyKind},
    playlist_feedback::{Edit, PendingEdits, Reply as EditReply},
    properties::OBSERVED,
    seek_feedback::{self, SeekFeedback},
};
use riina_core::{
    CapturedFrame, Command, CommandPlan, NoticeSeverity, Phase, Playback, RememberedVolume,
    key_bindings::{MpvInputOwner, mpv_command_prefix_len, mpv_command_suppresses_osd},
};
use std::{
    collections::{BTreeMap, HashMap},
    ffi::{CString, c_int},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{Receiver, SyncSender},
    },
    time::Instant,
};

pub(super) struct Start<N> {
    pub(super) api: Api,
    pub(super) headless: bool,
    pub(super) options: StartupOptions,
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
        let mut enable_audio_subtitle_surface = !headless
            && !options
                .mpv
                .iter()
                .any(|(name, _)| matches!(name.as_str(), "force-window" | "wid"));
        let error = match configure(&mut handle, headless, options) {
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

        let mut deferred = None;
        while let Some(message) = deferred
            .take()
            .or_else(|| state.hooks.next_message(&receiver, state.next_deadline()))
        {
            let (message, next) = timeline_queue::latest(message, &receiver);
            deferred = next;
            if accept_shutdown(&message) {
                break;
            }
            let handled = catch_unwind(AssertUnwindSafe(|| {
                state.handle_message(
                    message,
                    &mut handle,
                    &callback_sender,
                    &notify,
                    &mut enable_audio_subtitle_surface,
                );
                if state.drain_events(&mut handle, &callback_sender) {
                    state.script_input.request_due(&handle);
                    state.repair_audio_output(&handle);
                    state.stop_cancelled_stream(&handle);
                    state.hooks.continue_ready(&handle);
                    state.publish(&notify);
                }
            }));
            if state.playback.engine_shutdown || handled.is_err() {
                stopped.store(true, Ordering::Release);
                if state.playback.engine_shutdown {
                    trace_shutdown("native shutdown observed; waiting for render owners");
                } else {
                    state.retire_after_panic(&handle);
                }
                let terminal = Arc::new(std::mem::take(&mut state.playback));
                drop(state);
                let _ = catch_unwind(AssertUnwindSafe(|| notify(terminal)));
                // The final owner releases all render contexts before acknowledging shutdown.
                for queued in deferred.into_iter().chain(&receiver) {
                    if accept_shutdown(&queued) {
                        break;
                    }
                }
                return;
            }
        }
        state.finish_shutdown(&handle, &notify);
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
    handle: &mut Handle,
    headless: bool,
    options: StartupOptions,
) -> Result<Vec<String>, EngineError> {
    for (name, value) in [
        ("config", "no"),
        ("load-scripts", "no"),
        // The app owns the selected profile. Older engines also check their
        // standalone shortcuts before the console's text fallback.
        ("input-default-bindings", "no"),
        // Forwarded text can contain up to 64 characters in one UI event.
        ("input-key-fifo-size", "128"),
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
        // mpv can otherwise select RGB or 4:2:2 from the rendered frame,
        // including RGB from a GPU-rendered SDR screenshot. SVT needs 4:2:0.
        ("screenshot-avif-pixfmt", "yuv420p10le"),
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
    let explicit_device = options.mpv.iter().any(|(key, _)| key == "audio-device");
    for (key, value) in options.mpv {
        let key = CString::new(key).map_err(EngineError::StartupOption)?;
        let value = CString::new(value).map_err(EngineError::StartupOption)?;
        if let Err(error) = handle.set_option(&key, &value) {
            // IINA reports invalid optional preferences while keeping media playable.
            ignored.push(format!("{}: {error}", key.to_string_lossy()));
        }
    }
    handle.initialize()?;
    if !explicit_device
        && let Some(device) = options.preferred_audio_device
        && let Err(error) = audio_route::apply_preferred_device(handle, &device)
    {
        ignored.push(format!("audio-device: {error}"));
    }
    for property in OBSERVED {
        handle.observe(0, property.name, property.format)?;
    }
    Ok(ignored)
}

#[derive(Default)]
struct WorkerState {
    playback: Playback,
    audio_route: audio_route::Watch,
    pending_audio_change: Option<(u64, String)>,
    next_id: u64,
    observers: BTreeMap<u64, String>,
    next_observer: u64,
    pending: HashMap<u64, PendingCommand>,
    pending_stop: Option<(u64, u32)>,
    playlist_edits: PendingEdits,
    filter_edits: filter_edits::Queue,
    subtitle_reload: Option<subtitle_reload::Reload>,
    client_requests: HashMap<u64, Pending>,
    hooks: hooks::Hooks,
    script_input: script_input::Watch,
    volume_boost: bool,
    speed_from_arrow: bool,
    adjusted_volume_read: Option<(u64, u64)>,
    folder_discovery: folder_discovery::FolderDiscovery,
    playlist_end_pause: playlist_end_pause::EndPause,
    load_resume: Resume,
    playlist_shuffle: playlist_shuffle::PlaylistShuffle,
    playlist_insertion: Option<PlaylistInsertion>,
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

enum PlaylistInsertion {
    Submitted(u64),
    Confirmed(i64),
}

impl WorkerState {
    fn handle_message(
        &mut self,
        message: Message,
        handle: &mut Handle,
        wake: &SyncSender<Message>,
        notify: &impl Fn(Arc<Playback>),
        enable_audio_subtitle_surface: &mut bool,
    ) {
        match message {
            Message::Wake => handle.acknowledge_wakeup(),
            Message::Intent(command @ Command::ObserveMpvProperty(_)) => {
                self.observe_dynamic(handle, &command, notify);
            }
            Message::Intent(command) => self.handle_intent(handle, command),
            Message::PlaylistEdit(command) => {
                let edit = Edit::for_command(&command);
                self.handle_intent_with_stream(handle, command, None, edit);
            }
            Message::MediaIntent {
                generation,
                command,
            } => {
                if self.drain_events(handle, wake) && self.accepts_media_intent(generation) {
                    self.handle_intent(handle, command);
                }
            }
            Message::FinalizeIdleMedia {
                generation,
                request,
            } => {
                if self.drain_events(handle, wake) {
                    self.finalize_idle_media(handle, generation, request);
                }
            }
            Message::FilterEdit(request) => self.enqueue_filter_edit(request),
            Message::OpenStream(command, source, attempt) => {
                self.open_stream(handle, command, source, attempt);
            }
            Message::Client(request) => {
                if !matches!(request.operation, Operation::RegisterHook(_))
                    || self.drain_events(handle, wake)
                {
                    self.request_client(handle, request);
                }
            }
            Message::RenderingReady => {
                self.prepare_subtitle_surface(handle, enable_audio_subtitle_surface);
            }
            Message::Shutdown => {}
        }
    }

    fn retire_after_panic(&mut self, handle: &Handle) {
        diag("The native player event worker stopped unexpectedly");
        self.playback.phase = Phase::Failed;
        record_notice(
            &mut self.playback,
            NoticeSeverity::Error,
            "The playback engine stopped unexpectedly; reopen the file".into(),
        );
        self.hooks.continue_all(handle);
        self.continue_stream_auth(handle);
        self.release_end_pause_hook(handle);
        handle.stop_after_panic();
    }

    fn finalize_idle_media(&mut self, handle: &Handle, generation: u64, request: u32) {
        if self.pending_stop.is_some()
            || !self.media_generation_is_current(generation)
            || !self.playback.idle_for_finalization()
        {
            return;
        }
        if self.playback.stopped_idle_by_engine() {
            // A native Stop already ended the file. Respect its keep-playlist flag.
            self.playback.stopped_by = Some(request);
            self.mark_changed();
        } else {
            self.handle_intent(handle, Command::Stop(Some(request)));
        }
    }

    fn prepare_subtitle_surface(&mut self, handle: &Handle, enabled: &mut bool) {
        if std::mem::take(enabled)
            && let Err(error) = handle.set_option(c"force-window", c"immediate")
        {
            self.mark_changed();
            record_notice(
                &mut self.playback,
                NoticeSeverity::Warning,
                format!("Could not prepare video output for audio-only subtitles: {error}"),
            );
        }
    }

    fn finish_shutdown(&mut self, handle: &Handle, notify: &impl Fn(Arc<Playback>)) {
        self.hooks.continue_all(handle);
        self.continue_stream_auth(handle);
        self.release_end_pause_hook(handle);
        if self.playback.save_position_on_quit != Some(true)
            || !matches!(
                self.playback.phase,
                Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Seeking
            )
        {
            return;
        }
        // Destroying a libmpv client skips the native quit command's save step.
        if let Err(error) = handle.write_watch_later() {
            diag(&format!(
                "Could not save playback position on close: {error}"
            ));
            return;
        }
        self.watch_later_changed();
        let _ = catch_unwind(AssertUnwindSafe(|| self.publish(notify)));
    }

    fn watch_later_changed(&mut self) {
        self.playback.watch_later_revision = self.playback.watch_later_revision.wrapping_add(1);
        self.mark_changed();
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.script_input
            .deadline()
            .into_iter()
            .chain(self.filter_edits.deadline())
            .chain(
                self.subtitle_reload
                    .as_ref()
                    .and_then(subtitle_reload::Reload::deadline),
            )
            .min()
    }

    fn mark_changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    fn repair_audio_output(&mut self, handle: &Handle) {
        if self
            .pending_audio_change
            .as_ref()
            .is_some_and(|(_, device)| {
                self.playback.audio_device.as_deref() == Some(device.as_str())
            })
        {
            self.pending_audio_change = None;
        }
        if let Some(device) = self
            .audio_route
            .observe(&self.playback, self.pending_audio_change.is_some())
        {
            self.handle_intent(handle, Command::AudioDevice(device));
        }
    }

    fn publish(&mut self, notify: &impl Fn(Arc<Playback>)) {
        if self.published_revision == Some(self.revision) {
            return;
        }
        self.playback.reconcile();
        self.publish_open_feedback();
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
        if matches!(command, Command::SubtitleEncoding(_)) {
            if let Err(error) = self.change_subtitle_encoding(handle, &command) {
                self.report_subtitle_reload_error(error);
            }
            return;
        }
        if self.retain_subtitle_selection(&command) {
            return;
        }
        if let Command::MpvScriptKeys { owner, keys } = &command {
            // Escape may close a script between UI submission and worker delivery.
            let active = match owner {
                MpvInputOwner::Console => {
                    self.playback.mpv_console_open && self.playback.mpv_console_keys_active
                }
                MpvInputOwner::Statistics => {
                    !self.playback.mpv_console_open
                        && keys
                            .iter()
                            .all(|key| self.playback.mpv_statistics_keys.contains(key))
                }
            };
            if !active {
                return;
            }
            if *owner == MpvInputOwner::Statistics {
                self.script_input.schedule();
            }
        }
        self.handle_intent_with_stream(handle, command, None, None);
    }

    fn accepts_media_intent(&self, generation: u64) -> bool {
        self.media_generation_is_current(generation)
            && !matches!(
                self.playback.phase,
                Phase::Loading | Phase::Stopped | Phase::Failed
            )
    }

    fn media_generation_is_current(&self, generation: u64) -> bool {
        self.playback.generation == generation
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
        playlist_edit: Option<Edit>,
    ) {
        let shuffled = self
            .playlist_shuffle
            .expected(self.playback.playlist_shuffled);
        let mut plan = match command.plan_with_playlist_shuffle(&self.playback, shuffled) {
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
        if self.retain_subtitle_cycles(&mut plan.commands) {
            return;
        }
        if matches!(command, Command::TogglePause)
            && self.media_generation_is_current(self.playback.generation)
            && self.expects_arrow_speed()
        {
            plan.commands
                .insert(0, ["set", "speed", "1"].map(str::to_owned).into());
        }
        if let Command::SetPauseWhenOpen(pause) = &command
            && self.playback.pause_when_open != *pause
        {
            // Changing the policy must leave the current playback state alone.
            self.playback.pause_when_open = *pause;
            if *pause {
                self.install_end_pause_hook(handle);
            }
            self.mark_changed();
        }
        if let Command::VideoAspect(aspect) = &command {
            // The requested aspect remains visible across failure and source replacement.
            self.playback.video_aspect.clone_from(aspect);
            self.playback.aspect_requests = self.playback.aspect_requests.wrapping_add(1);
            self.mark_changed();
        }
        let changes_pause = changes_pause(&command);
        if changes_pause {
            self.seek_feedback.playback_transition();
        }
        let starts_media = command.replaces_media() || plan.resume_on_load.is_some();
        if starts_media {
            self.stream = None;
        }
        if starts_media || changes_pause {
            self.load_resume.cancel();
        }
        let playlist_edit = playlist_edit.or_else(|| Edit::for_open(&command));
        if let Err(error) = self.issue_plan(handle, &command, plan, stream, playlist_edit) {
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
        }
    }

    fn issue_plan(
        &mut self,
        handle: &Handle,
        command: &Command,
        plan: CommandPlan,
        mut stream: Option<stream::StreamLoad>,
        playlist_edit: Option<Edit>,
    ) -> Result<(), String> {
        let playlist_batch = self.next_id + 1;
        let changes_pause = changes_pause(command);
        let is_auto_seek = matches!(
            command,
            Command::SeekWithType {
                seek_type: riina_core::control_preferences::SeekType::Auto,
                ..
            }
        );
        // The complete intent is planned before native submission begins.
        for mut args in plan.commands {
            let prefix_len = mpv_command_prefix_len(&args);
            let mut reply =
                ReplyKind::for_command(command, args.get(prefix_len).map(String::as_str));
            if let (Command::Volume(volume) | Command::RestoreVolume(volume), [_, _, value]) =
                (command, args.as_mut_slice())
            {
                let capped = volume.min(self.volume_limit());
                *value = capped.to_string();
                if matches!(command, Command::Volume(_)) {
                    reply = ReplyKind::RememberVolume(capped);
                }
            }
            if let (Command::OpenMediaLink(link), [action, property, value]) =
                (command, args.as_mut_slice())
                && action == "set"
                && property == "volume"
                && let Some(volume) = link.volume
            {
                *value = volume.min(self.volume_limit()).to_string();
            }
            let rewriting_seek =
                is_auto_seek && args.get(prefix_len).is_some_and(|arg| arg == "seek");
            let navigation = seek_feedback::navigation(&args[prefix_len..], &self.playback);
            if rewriting_seek && args.len() >= prefix_len + 3 {
                args[prefix_len + 2] = self.auto_seek.flags().into();
            } else if navigation {
                self.auto_seek.interrupt_measurement();
            }
            if command.restarts_decoder() {
                self.seek_feedback.clear();
                self.auto_seek.interrupt_measurement();
            }
            let (prefixes, action) = args.split_at(prefix_len);
            if writes_property(action, "speed") {
                reply = ReplyKind::SpeedWrite {
                    from_arrow: matches!(command, Command::ControllerSpeed { speed, .. } if *speed != 1.0),
                };
            }
            let reporting_edit =
                playlist_edit.filter(|edit| action.first().is_some_and(|verb| edit.tracks(verb)));
            let replaces_file = is_replacement(action);
            if let Some(edit) = reporting_edit {
                let lead = replaces_file && matches!(edit, Edit::OpenGroup);
                reply = ReplyKind::PlaylistEdit {
                    batch: playlist_batch,
                    lead,
                };
            }
            self.issue(handle, &args, reply)?;
            if let Command::Stop(Some(request)) = command
                && matches!(
                    action.first().map(String::as_str),
                    Some("stop" | "playlist-clear")
                )
            {
                self.pending_stop = Some((self.next_id, *request));
            }
            if action.first().is_some_and(|verb| verb == "loadfile")
                && matches!(
                    command,
                    Command::InsertPlaylistTrack {
                        make_current: true,
                        ..
                    }
                )
            {
                self.playlist_insertion = Some(PlaylistInsertion::Submitted(self.next_id));
            }
            if script_input::changes_statistics(action) {
                self.script_input.schedule();
            }
            if let Command::AudioDevice(device) = command {
                // A reply can arrive before the new audio-device observation.
                self.pending_audio_change = Some((self.next_id, device.clone()));
            }
            let changes_media = changes_media(action);
            if changes_media {
                if let Some(policy) = plan.resume_on_load.filter(|_| {
                    replaces_file
                        || matches!(
                            action[0].as_str(),
                            "playlist-play-index" | "playlist-next" | "playlist-prev"
                        )
                }) {
                    self.load_resume.submit(self.next_id, policy);
                } else {
                    self.load_resume.cancel();
                }
            } else if writes_property(action, "pause") {
                self.load_resume.cancel();
            }
            if changes_media || action.first().is_some_and(|verb| verb == "playlist-clear") {
                self.playlist_edits.retire_open();
                self.playlist_insertion = None;
            }
            if let Some(edit) = reporting_edit {
                self.playlist_edits
                    .track(playlist_batch, edit, self.playback.generation);
            }
            if changes_media || changes_pause {
                self.playlist_end_pause.disarm();
            }
            if command.replaces_media() && replaces_file {
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
            if matches!(
                action.first().map(String::as_str),
                Some("stop" | "playlist-clear")
            ) {
                // Clearing the queue also retires any sibling scan still in flight.
                self.folder_discovery = folder_discovery::FolderDiscovery::default();
                self.playback.folder_discovery_allowed = false;
            }
            if matches!(command, Command::OpenShuffledPlaylist(_))
                && action
                    .first()
                    .is_some_and(|name| name == "playlist-shuffle")
            {
                self.playback.explicit_playlist = true;
                self.mark_changed();
            }
            if rewriting_seek {
                self.auto_seek.submitted(self.next_id);
            }
            if navigation {
                if mpv_command_suppresses_osd(prefixes) {
                    self.seek_feedback.clear();
                } else {
                    self.seek_feedback.submitted(self.next_id);
                }
            }
        }
        Ok(())
    }

    fn issue(&mut self, handle: &Handle, args: &[String], kind: ReplyKind) -> Result<(), String> {
        let prefix_len = mpv_command_prefix_len(args);
        let command = &args[prefix_len..];
        let changes_media = changes_media(command);
        let changes_watch_later = match command.first().map(String::as_str) {
            Some("write-watch-later-config" | "delete-watch-later-config") => true,
            // Native replacement saves the outgoing file according to mpv's policy.
            Some("loadfile") => {
                self.active_entry.is_some()
                    && self.playback.save_position_on_quit != Some(false)
                    && command.get(2).is_none_or(|mode| mode == "replace")
            }
            _ => false,
        };
        let args: Vec<_> = args
            .iter()
            .map(|arg| CString::new(arg.as_str()))
            .collect::<Result<_, _>>()
            .map_err(|error| error.to_string())?;
        self.next_id += 1;
        let label = command_label(&args[prefix_len..]);
        handle
            .command(self.next_id, &args)
            .map_err(|error| format!("mpv {label}: {error}"))?;
        self.playlist_shuffle.submitted(self.next_id, command);
        if changes_media
            || command
                .first()
                .is_some_and(|verb| matches!(verb.as_str(), "loadfile" | "loadlist"))
        {
            self.retire_stop();
        }
        if let [action, property, value] = &args[prefix_len..]
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
                changes_watch_later,
            },
        );
        Ok(())
    }

    fn volume_limit(&self) -> f64 {
        if self.volume_boost { 200.0 } else { 100.0 }
    }

    fn expects_arrow_speed(&self) -> bool {
        // A second control can arrive before libmpv replies to the previous speed write.
        self.pending
            .iter()
            .filter_map(|(&id, pending)| {
                if let ReplyKind::SpeedWrite { from_arrow } = pending.kind
                    && pending.generation == self.playback.generation
                {
                    Some((id, from_arrow))
                } else {
                    None
                }
            })
            .max_by_key(|&(id, _)| id)
            .map_or(self.speed_from_arrow, |(_, from_arrow)| from_arrow)
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
        if matches!(&operation, Operation::Write { name, .. } if name.as_c_str() == c"pause") {
            self.load_resume.cancel();
        }
        Ok(Some(self.next_id))
    }

    fn drain_events(&mut self, handle: &mut Handle, wake: &SyncSender<Message>) -> bool {
        if self.playback.engine_shutdown {
            return false;
        }
        while let Some(event) = handle.wait() {
            if event.id() == event_id::SHUTDOWN {
                self.playback.engine_shutdown = true;
                self.playback.phase = Phase::Stopped;
                if let Some(name) = event.name() {
                    self.playback
                        .record_mpv_event(Arc::from(name.to_string_lossy().as_ref()));
                }
                return false;
            }
            if script_input::Watch::is_reply(&event) {
                if self.script_input.apply_reply(&event, &mut self.playback) {
                    self.mark_changed();
                }
                continue;
            }
            if let Some((hook, entry, failed)) = self.stream_auth.take_reply(&event) {
                self.finish_stream_auth(handle, hook, entry, failed);
                continue;
            }
            if let Some(result) = self.take_filter_read(&event) {
                self.finish_filter_read(handle, result);
                continue;
            }
            if event.id() == event_id::GET_PROPERTY_REPLY
                && self
                    .subtitle_reload
                    .as_ref()
                    .is_some_and(|batch| batch.reads_catalog(event.userdata()))
            {
                let result = event.client_reply();
                self.finish_subtitle_reload(handle, result);
                continue;
            }
            if event.id() == event_id::GET_PROPERTY_REPLY
                && let Some((id, revision)) = self.adjusted_volume_read
                && id == event.userdata()
            {
                self.adjusted_volume_read = None;
                if let Ok(value) = event.client_reply()
                    && let Some(level) = value.as_f64().filter(|value| value.is_finite())
                {
                    self.remember_volume(revision, level.clamp(0.0, self.volume_limit()));
                }
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
            let name = name.clone();
            if let Some(id) = event.hook_id() {
                let slot = event.userdata();
                if slot == stream::AUTH_HOOK_SLOT {
                    self.authenticate_stream(handle, id);
                } else if slot == playlist_end_pause::HOOK_SLOT {
                    self.pause_after_end(handle, id);
                } else {
                    self.hooks.receive(handle, id, slot, wake);
                }
            } else if event.id() == event_id::FILE_LOADED {
                self.on_file_loaded();
            } else if event.id() == event_id::PLAYBACK_RESTART {
                self.on_playback_restarted(handle);
            } else if event.id() == event_id::QUEUE_OVERFLOW {
                let _ = self.on_event(&event);
                self.continue_stream_auth(handle);
                self.release_end_pause_hook(handle);
            } else {
                let command_reply =
                    (event.id() == event_id::COMMAND_REPLY).then(|| event.userdata());
                let failed = event.failed();
                if failed
                    && self
                        .pending_audio_change
                        .as_ref()
                        .is_some_and(|(id, _)| Some(*id) == command_reply)
                {
                    self.pending_audio_change = None;
                }
                let adjusted_volume = self.on_event(&event);
                if let Some(revision) = adjusted_volume {
                    self.read_adjusted_volume(handle, revision);
                }
                if let Some(id) = command_reply {
                    self.filter_edits.edit_replied(id);
                    if let Some(reload) = &mut self.subtitle_reload {
                        reload.replied(id, failed);
                    }
                    self.finish_end_pause(handle, id);
                }
            }
            if let Some(name) = name {
                self.playback.record_mpv_event(name);
            }
        }
        if let Some(PlaylistInsertion::Confirmed(id)) = self.playlist_insertion
            && self.playback.playlist.iter().any(|entry| entry.id == id)
        {
            self.playlist_insertion = None;
            self.handle_intent(handle, Command::SelectPlaylistId(id));
        }
        self.advance_filter_edits(handle);
        self.advance_subtitle_reload(handle);
        true
    }

    fn on_event(&mut self, event: &EventRef<'_>) -> Option<u64> {
        match event.id() {
            event_id::COMMAND_REPLY => {
                if matches!(self.playlist_insertion, Some(PlaylistInsertion::Submitted(id)) if id == event.userdata())
                {
                    // Resolve the returned ID against the observed queue after draining events.
                    self.playlist_insertion = if event.failed() {
                        None
                    } else {
                        event.loaded_entry().map(PlaylistInsertion::Confirmed)
                    };
                }
                self.on_stream_reply(event);
                return self.on_command_reply(event);
            }
            event_id::START_FILE => {
                self.subtitle_reload = None;
                self.speed_from_arrow = false;
                self.active_entry = event.started_entry();
                self.playlist_edits.file_started(self.active_entry);
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
                self.playback.active_playlist_entry = self.active_entry;
                self.playlist_insertion = None;
                self.playback.folder_discovery_allowed =
                    self.folder_discovery.take_for(self.active_entry);
                self.load_resume
                    .file_started(self.playback.generation, self.active_entry);
            }
            event_id::END_FILE => self.on_end_file(event),
            event_id::SEEK => {
                self.seek_pending = true;
                self.seek_feedback.started();
                self.auto_seek.seek_started(Instant::now());
            }
            event_id::PROPERTY_CHANGE => {
                self.apply_property_event(event);
            }
            event_id::QUEUE_OVERFLOW => {
                self.retire_stop();
                self.script_input.recover();
                self.stream = None;
                self.folder_discovery = folder_discovery::FolderDiscovery::default();
                self.playback.folder_discovery_allowed = false;
                self.client_requests.clear();
                self.playlist_edits.clear();
                self.retire_filter_edits();
                self.subtitle_reload = None;
                self.pending
                    .retain(|_, reply| !matches!(reply.kind, ReplyKind::PlaylistEdit { .. }));
                self.adjusted_volume_read = None;
                self.pending_audio_change = None;
                self.speed_from_arrow = false;
                self.load_resume.cancel();
                self.playlist_shuffle = playlist_shuffle::PlaylistShuffle::default();
                self.playback.playlist_shuffled = false;
                self.playlist_insertion = None;
                record_notice(
                    &mut self.playback,
                    NoticeSeverity::Error,
                    "The libmpv event queue overflowed; playback state may be incomplete".into(),
                );
            }
            _ => {}
        }
        None
    }

    fn apply_property_event(&mut self, event: &EventRef<'_>) {
        if let Some(name) = self.observers.get(&event.userdata()) {
            // The transport may have credentials that never belong in a
            // player snapshot or in the retained plugin event history.
            let value = self
                .stream_auth
                .public_address
                .as_ref()
                .filter(|_| self.stream_auth.protects(name))
                .map_or_else(
                    || event.plugin_property(),
                    |address| {
                        let value = serde_json::Value::String(address.clone());
                        let size = value.to_string().len();
                        Some((value, size))
                    },
                );
            if let Some((value, bytes)) = value {
                self.playback
                    .record_mpv_property(name.clone(), value, bytes);
            }
            return;
        }

        let console_open = self.playback.mpv_console_open;
        event.apply_property(&mut self.playback);
        if self.playback.idle_active && self.playback.phase == Phase::Failed {
            self.load_resume.became_idle();
        }
        if console_open != self.playback.mpv_console_open {
            self.script_input.schedule();
        }
    }

    fn read_adjusted_volume(&mut self, handle: &Handle, revision: u64) {
        // Read the native result of the relative adjustment, including its cap.
        self.next_id += 1;
        let read = Operation::Read {
            name: c"volume".to_owned(),
            format: MpvPropertyFormat::Number,
        };
        if handle.request_property(self.next_id, &read).is_ok() {
            self.adjusted_volume_read = Some((self.next_id, revision));
        }
    }

    fn remember_volume(&mut self, revision: u64, level: f64) {
        if self
            .playback
            .remembered_volume
            .is_none_or(|old| old.revision < revision)
        {
            self.playback.remembered_volume = Some(RememberedVolume { revision, level });
            self.mark_changed();
        }
    }

    fn on_end_file(&mut self, event: &EventRef<'_>) {
        let Some(end) = event.end_file() else {
            return;
        };
        self.playlist_edits.file_ended(end);
        let stream = self.stream.take_if(|load| load.end(end));
        if self.active_entry != Some(end.playlist_entry_id) {
            return;
        }
        let cancelled = stream.as_ref().filter(|load| load.cancelled());
        if end.reason == end_reason::ERROR
            && self.playback.phase == Phase::Loading
            && cancelled.is_none()
        {
            self.playback.file_feedback = riina_core::FileFeedback::OpenFailed {
                stream_attempt: stream.as_ref().map(stream::StreamLoad::attempt_id),
            };
        } else if end.reason == end_reason::STOP
            && self.playback.phase.has_loaded_file()
            && self.pending_stop.is_none()
            && self.playback.stopped_by.is_none()
        {
            // Native navigation can also end a file with STOP. The idle
            // observation and generation check decide whether it ended playback.
            self.playback.file_feedback = riina_core::FileFeedback::StoppedByEngine;
        }
        self.subtitle_reload = None;
        self.arm_end_pause(end.reason);
        self.active_entry = None;
        self.playback.folder_discovery_allowed = false;
        self.seek_pending = false;
        self.seek_feedback.clear();
        self.auto_seek.interrupt_measurement();
        if let Some(load) = cancelled {
            self.record_cancelled_open(load.attempt_id());
        } else {
            self.playback.phase = match end.reason {
                end_reason::EOF => Phase::Ended,
                end_reason::ERROR => Phase::Failed,
                _ => Phase::Stopped,
            };
        }
        if matches!(self.playback.phase, Phase::Stopped | Phase::Failed) {
            self.clear_active_file();
        }
        if cancelled.is_none()
            && end.error < 0
            && let Some(message) = event.end_error(end)
        {
            record_notice(&mut self.playback, NoticeSeverity::Error, message);
        }
        // A manual open may skip an unreadable entry or redirect to a playlist.
        // Its temporary pause still belongs to the first playable successor.
        let continues =
            cancelled.is_none() && matches!(end.reason, end_reason::REDIRECT | end_reason::ERROR);
        self.load_resume
            .file_ended(self.playback.generation, continues);
    }

    fn clear_active_file(&mut self) {
        self.playback.active_playlist_entry = None;
        self.playback.ab_loop_a = None;
        self.playback.ab_loop_b = None;
    }

    fn on_file_loaded(&mut self) {
        self.complete_open_stream();
        self.playback.phase = Phase::Playing;
        self.load_resume.file_loaded(self.playback.generation);
    }

    fn on_playback_restarted(&mut self, handle: &Handle) {
        if self.playback.file_feedback == riina_core::FileFeedback::Loading {
            self.playback.file_feedback = riina_core::FileFeedback::Restarted(Instant::now());
            // mpv may remove a consumed resume file after FILE_LOADED.
            self.watch_later_changed();
        }
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
            .load_resume
            .playback_restarted(self.playback.generation, self.playback.pause_when_open)
        {
            let args = ["set".into(), "pause".into(), "no".into()];
            if let Err(error) = self.issue(handle, &args, ReplyKind::Plain) {
                record_notice(&mut self.playback, NoticeSeverity::Error, error);
            }
        }
    }

    fn on_command_reply(&mut self, event: &EventRef<'_>) -> Option<u64> {
        static NEXT_VOLUME_REVISION: AtomicU64 = AtomicU64::new(1);
        let failed = event.failed();
        if let Some((id, request)) = self.pending_stop
            && id == event.userdata()
        {
            self.pending_stop = None;
            if !failed {
                // An idle core has no file left to emit END_FILE after the clear.
                if self.playback.idle_for_finalization() {
                    self.playback.phase = Phase::Stopped;
                    self.clear_active_file();
                }
                if self.playback.file_feedback == riina_core::FileFeedback::StoppedByEngine {
                    self.playback.file_feedback = riina_core::FileFeedback::Idle;
                }
                self.playback.stopped_by = Some(request);
                self.mark_changed();
            }
        }
        if self.folder_discovery.awaits(event.userdata()) {
            self.folder_discovery
                .replied((!failed).then(|| event.loaded_entry()).flatten());
            if self.folder_discovery.take_for(self.active_entry) {
                self.playback.folder_discovery_allowed = true;
            }
        }
        if self.load_resume.awaits_reply(event.userdata()) {
            self.load_resume.replied(if failed {
                Err(())
            } else {
                Ok(event.loaded_entry())
            });
        }
        self.seek_feedback.replied(event.userdata(), failed);
        if failed {
            self.auto_seek.failed(event.userdata());
        }
        let pending = self.pending.remove(&event.userdata())?;
        // Playlist edits survive changes to the active file's generation.
        if let Some(enabled) = self.playlist_shuffle.replied(event.userdata(), failed) {
            self.playback.playlist_shuffled = enabled;
        }
        if pending.changes_watch_later && !failed {
            // The file effect survives a media change. A reply confirms the
            // attempt completed, so readers still need to check the disk.
            self.watch_later_changed();
        }
        if let ReplyKind::PlaylistEdit { batch, lead } = pending.kind {
            let reply = if lead {
                EditReply::Lead((!failed).then(|| event.loaded_entry()).flatten())
            } else {
                EditReply::Entry(!failed)
            };
            if let Some(feedback) = self.playlist_edits.reply(batch, reply) {
                self.publish_playlist_feedback(feedback);
            }
        }
        if !pending.belongs_to(&self.playback) {
            return None;
        }
        if pending.kind == ReplyKind::FrameCapture {
            self.finish_frame_request();
        }
        if failed {
            self.on_failed_reply(event, pending);
            return None;
        }
        match pending.kind {
            ReplyKind::Plain
            | ReplyKind::PlaylistEdit { .. }
            | ReplyKind::TrackAttachment(_)
            | ReplyKind::MatchedSubtitle => {}
            ReplyKind::SpeedWrite { from_arrow } => self.speed_from_arrow = from_arrow,
            ReplyKind::ChapterSelection { index, start } => {
                self.publish_chapter_feedback(event.userdata(), index, start);
            }
            ReplyKind::FilterEdit(change) => {
                if self.accepts_media_intent(pending.generation)
                    && self
                        .playback
                        .filter_feedback
                        .as_ref()
                        .is_none_or(|old| old.sequence < event.userdata())
                {
                    self.playback.filter_feedback = Some(riina_core::FilterFeedback {
                        sequence: event.userdata(),
                        change,
                    });
                    self.mark_changed();
                }
            }
            ReplyKind::RememberVolume(level) => {
                self.remember_volume(NEXT_VOLUME_REVISION.fetch_add(1, Ordering::Relaxed), level);
            }
            ReplyKind::RememberAdjustedVolume => {
                return Some(NEXT_VOLUME_REVISION.fetch_add(1, Ordering::Relaxed));
            }
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
        None
    }

    fn retire_stop(&mut self) {
        self.pending_stop = None;
        if self.playback.stopped_by.take().is_some() {
            self.mark_changed();
        }
    }

    fn publish_open_feedback(&mut self) {
        if matches!(
            self.playback.phase,
            Phase::Playing | Phase::Paused | Phase::Buffering
        ) && matches!(
            self.playback.file_feedback,
            riina_core::FileFeedback::Restarted(_)
        ) && let Some(feedback) = self
            .playlist_edits
            .ready_open(self.active_entry, self.playback.generation)
        {
            self.publish_playlist_feedback(feedback);
        }
    }

    fn publish_chapter_feedback(&mut self, sequence: u64, index: usize, start: f64) {
        if let Some(chapter) = self.playback.chapters.get(index)
            && chapter.time.to_bits() == start.to_bits()
            && self
                .playback
                .chapter_feedback
                .as_ref()
                .is_none_or(|old| old.sequence < sequence)
        {
            self.playback.chapter_feedback = Some(riina_core::ChapterFeedback {
                sequence,
                title: chapter.title.as_deref().map_or_else(
                    || Arc::from(chapter.display_title(index).as_ref()),
                    Arc::from,
                ),
            });
            self.mark_changed();
        }
    }

    fn publish_playlist_feedback(&mut self, feedback: riina_core::PlaylistFeedback) {
        if feedback.generation == self.playback.generation
            && self
                .playback
                .playlist_feedback
                .is_none_or(|old| old.sequence < feedback.sequence)
        {
            self.playback.playlist_feedback = Some(feedback);
        }
    }

    fn on_failed_reply(&mut self, event: &EventRef<'_>, pending: PendingCommand) {
        match pending.kind {
            ReplyKind::TrackAttachment(_) | ReplyKind::MatchedSubtitle => {
                if let ReplyKind::TrackAttachment(kind) = pending.kind
                    && self.accepts_media_intent(pending.generation)
                    && !self.playback.idle_active
                {
                    self.playback.track_attachment_failure =
                        Some(riina_core::TrackAttachmentFailure {
                            sequence: event.userdata(),
                            kind,
                        });
                }
                // The alert owns visible manual feedback. Automatic matching is diagnostic only.
                if let Some(error) = event.error() {
                    diagnostic!("mpv {}: {error}", pending.label);
                }
            }
            ReplyKind::MaterialCapture => {
                let reason = event
                    .error()
                    .unwrap_or_else(|| format!("mpv error {}", event.error_code()));
                diagnostic!("Could not capture the Windows video material frame: {reason}");
            }
            ReplyKind::FrameCapture
            | ReplyKind::TransitionCapture
            | ReplyKind::IncomingTransitionCapture => {}
            ReplyKind::Plain
            | ReplyKind::RememberVolume(_)
            | ReplyKind::RememberAdjustedVolume
            | ReplyKind::SpeedWrite { .. }
            | ReplyKind::PlaylistEdit { .. }
            | ReplyKind::ChapterSelection { .. }
            | ReplyKind::FilterEdit(_)
            | ReplyKind::Screenshot { .. } => {
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
            ReplyKind::Plain
            | ReplyKind::RememberVolume(_)
            | ReplyKind::RememberAdjustedVolume
            | ReplyKind::SpeedWrite { .. }
            | ReplyKind::PlaylistEdit { .. }
            | ReplyKind::ChapterSelection { .. }
            | ReplyKind::FilterEdit(_)
            | ReplyKind::TrackAttachment(_)
            | ReplyKind::MatchedSubtitle
            | ReplyKind::Screenshot { .. } => {}
        }
    }
}

// Keep diagnostics actionable without exposing media paths or arbitrary option values.
fn is_replacement(args: &[String]) -> bool {
    args.first().is_some_and(|verb| verb == "loadfile")
        && args.get(2).is_some_and(|mode| mode == "replace")
}

fn changes_media(args: &[String]) -> bool {
    let mut names = args.iter().map(String::as_str);
    match names.next() {
        Some("loadfile") => !matches!(names.nth(1), Some("append" | "insert-at" | "insert-next")),
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

fn changes_pause(command: &Command) -> bool {
    matches!(
        command,
        Command::TogglePause
            | Command::SetPaused(_)
            | Command::ControllerSpeed { resume: true, .. }
    )
}

fn writes_property(args: &[String], name: &str) -> bool {
    matches!(
        args,
        [verb, property, ..]
            if property == name
                && matches!(verb.as_str(), "set" | "add" | "multiply" | "cycle" | "cycle-values")
    )
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
        for (mode, accepts) in [("append", true), ("insert-at", true), ("replace", false)] {
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
                    changes_watch_later: false,
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
