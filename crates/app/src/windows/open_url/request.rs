use riina_core::{MediaLink, sync::lock};
use riina_mpv::{CommandSender, EngineError, StreamAttempt, StreamSource, StreamStatus};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub(crate) struct RunningStream {
    attempt: StreamAttempt,
    sender: Arc<Mutex<Option<CommandSender>>>,
}

impl RunningStream {
    pub(super) fn id(&self) -> u64 {
        self.attempt.id()
    }

    pub(super) fn status(&self) -> StreamStatus {
        self.attempt.status()
    }

    pub(super) fn cancel(&self) {
        self.attempt.cancel();
        if let Some(sender) = lock(&self.sender).as_ref() {
            sender.cancel_stream(&self.attempt);
        }
    }
}

#[derive(Clone)]
pub(super) enum StreamMode {
    Manual,
    Startup(Option<f64>),
    Playlist {
        following: Arc<[String]>,
        explicit: bool,
    },
    Browser(MediaLink),
}

#[derive(Clone, Copy)]
pub(super) enum StreamRequest {
    New,
    Prepared(i32),
}

pub(crate) struct PendingStream {
    source: Option<StreamSource>,
    running: RunningStream,
    mode: StreamMode,
}

impl PendingStream {
    pub(super) fn new(source: StreamSource, mode: StreamMode) -> (Self, RunningStream) {
        let running = RunningStream::default();
        (
            Self {
                source: Some(source),
                running: running.clone(),
                mode,
            },
            running,
        )
    }

    pub(crate) fn start(self, app: &crate::PlayerWindow, sender: &CommandSender) {
        if let Err(error) = self.dispatch(app, sender, StreamRequest::New) {
            crate::diagnostics::report(app, error);
        }
    }

    pub(super) fn dispatch(
        mut self,
        app: &crate::PlayerWindow,
        sender: &CommandSender,
        request: StreamRequest,
    ) -> Result<bool, EngineError> {
        if let StreamRequest::Prepared(generation) = request
            && app.get_media_request_generation() != generation
        {
            return Ok(false);
        }
        let Some(source) = self.source.take() else {
            return Ok(false);
        };
        let mut attached = lock(&self.running.sender);
        if self.running.status() != StreamStatus::Loading {
            return Ok(false);
        }
        if matches!(request, StreamRequest::New) {
            crate::media_open::invalidate(app);
        }
        let opened = match &self.mode {
            StreamMode::Manual => sender.open_stream(source, self.running.attempt.clone()),
            StreamMode::Startup(start) => sender.open_stream_with_options(
                source,
                Some(MediaLink {
                    start: *start,
                    ..MediaLink::default()
                }),
                self.running.attempt.clone(),
            ),
            StreamMode::Playlist {
                following,
                explicit,
            } => sender.open_stream_playlist(
                source,
                following,
                *explicit,
                self.running.attempt.clone(),
            ),
            StreamMode::Browser(options) => sender.open_stream_with_options(
                source,
                Some(options.clone()),
                self.running.attempt.clone(),
            ),
        };
        if let Err(error) = opened {
            self.running.attempt.mark_failed();
            return Err(error);
        }
        *attached = Some(sender.clone());
        drop(attached);
        app.set_status("Opening…".into());
        if !matches!(self.mode, StreamMode::Browser(_))
            && let Err(error) = crate::media_open::apply_manual_open_repeat(app, sender)
        {
            app.set_status(error.into());
        }
        Ok(true)
    }
}

impl Drop for PendingStream {
    fn drop(&mut self) {
        if self.source.is_some() {
            self.running.attempt.mark_failed();
        }
    }
}
