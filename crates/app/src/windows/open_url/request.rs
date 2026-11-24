use riina_core::{MediaLink, sync::lock};
use riina_mpv::{CommandSender, EngineError, StreamAttempt, StreamSource, StreamStatus};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub(crate) struct RunningStream {
    attempt: StreamAttempt,
    sender: Arc<Mutex<Option<CommandSender>>>,
}

impl RunningStream {
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
    Browser(MediaLink),
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
        if let Err(error) = self.dispatch(app, sender) {
            crate::diagnostics::report(app, error);
        }
    }

    pub(super) fn dispatch(
        mut self,
        app: &crate::PlayerWindow,
        sender: &CommandSender,
    ) -> Result<bool, EngineError> {
        let Some(source) = self.source.take() else {
            return Ok(false);
        };
        let mut attached = lock(&self.running.sender);
        if self.running.status() != StreamStatus::Loading {
            return Ok(false);
        }
        let options = match &self.mode {
            StreamMode::Manual => None,
            StreamMode::Startup(start) => Some(MediaLink {
                start: *start,
                ..MediaLink::default()
            }),
            StreamMode::Browser(options) => Some(options.clone()),
        };
        if let Err(error) =
            sender.open_stream_with_options(source, options, self.running.attempt.clone())
        {
            self.running.attempt.mark_failed();
            return Err(error);
        }
        *attached = Some(sender.clone());
        drop(attached);
        crate::media_open::invalidate(app);
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
