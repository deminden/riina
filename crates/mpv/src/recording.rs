//! Test-only command queue and playback notifications without a native player.

use super::{CommandQueue, CommandSender, EngineError};
use riina_core::{Command, Playback};
use std::{
    cell::OnceCell,
    sync::{
        Arc,
        atomic::AtomicBool,
        mpsc::{self, Receiver},
    },
};

type Notify = Box<dyn Fn(Arc<Playback>) + Send>;

pub struct RecordingEngine {
    pub commands: Receiver<Command>,
    notify: OnceCell<Notify>,
}

impl CommandSender {
    /// Uses the same nonblocking, bounded FIFO as the native command sender.
    pub fn recording(capacity: usize) -> (Self, RecordingEngine) {
        let (sender, commands) = mpsc::sync_channel(capacity);
        (
            Self {
                sender: CommandQueue::Recording(sender),
                stopped: Arc::new(AtomicBool::new(false)),
            },
            RecordingEngine {
                commands,
                notify: OnceCell::new(),
            },
        )
    }
}

impl RecordingEngine {
    pub fn set_observer(
        &self,
        notify: impl Fn(Arc<Playback>) + Send + 'static,
    ) -> Result<(), EngineError> {
        self.notify
            .set(Box::new(notify))
            .map_err(|_| EngineError::ObserverAlreadyInstalled)
    }

    pub fn publish(&self, state: Playback) {
        if let Some(notify) = self.notify.get() {
            notify(Arc::new(state));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_preserves_fifo_full_and_closed_results() {
        let (sender, engine) = CommandSender::recording(2);
        assert!(sender.send(Command::TogglePause).is_ok());
        assert!(sender.send(Command::Stop).is_ok());
        let full = sender.send(Command::Volume(50.0));
        assert!(matches!(full, Err(EngineError::QueueFull)));
        assert_eq!(
            full.unwrap_err().to_string(),
            "Player command was not accepted: sending on a full channel",
        );
        assert!(matches!(
            sender.send(Command::Shutdown),
            Err(EngineError::ReservedShutdown),
        ));
        assert_eq!(engine.commands.try_recv(), Ok(Command::TogglePause));
        assert_eq!(engine.commands.try_recv(), Ok(Command::Stop));
        drop(engine);
        assert!(matches!(
            sender.send(Command::Stop),
            Err(EngineError::Stopped)
        ));
    }
}
