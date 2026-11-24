use riina_core::sync::lock;
use riina_mpv::{CommandSender, StreamAttempt, StreamSource, StreamStatus};
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

pub(crate) struct PendingStream {
    source: Option<StreamSource>,
    running: RunningStream,
}

impl PendingStream {
    pub(super) fn new(source: StreamSource) -> (Self, RunningStream) {
        let running = RunningStream::default();
        (
            Self {
                source: Some(source),
                running: running.clone(),
            },
            running,
        )
    }

    pub(crate) fn start(mut self, app: &crate::PlayerWindow, sender: &CommandSender) {
        let Some(source) = self.source.take() else {
            return;
        };
        let mut attached = lock(&self.running.sender);
        if self.running.status() != StreamStatus::Loading {
            return;
        }
        if let Err(error) = sender.open_stream(source, self.running.attempt.clone()) {
            self.running.attempt.mark_failed();
            crate::diagnostics::report(app, error);
        } else {
            *attached = Some(sender.clone());
            drop(attached);
            crate::media_open::invalidate(app);
            app.set_status("Opening…".into());
            if let Err(error) = crate::media_open::apply_manual_open_repeat(app, sender) {
                app.set_status(error.into());
            }
        }
    }
}

impl Drop for PendingStream {
    fn drop(&mut self) {
        if self.source.is_some() {
            self.running.attempt.mark_failed();
        }
    }
}
