//! Test-only command queue and playback notifications without a native player.

use super::{CommandQueue, CommandSender};
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
    ) -> Result<(), String> {
        self.notify
            .set(Box::new(notify))
            .map_err(|_| "A recording engine already has a playback observer".into())
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
        assert_eq!(sender.send(Command::TogglePause), Ok(()));
        assert_eq!(sender.send(Command::Stop), Ok(()));
        assert_eq!(
            sender.send(Command::Volume(50.0)),
            Err("Player command was not accepted: sending on a full channel".into()),
        );
        assert_eq!(
            sender.send(Command::Shutdown),
            Err("Player shutdown is reserved for its final owner".into()),
        );
        assert_eq!(engine.commands.try_recv(), Ok(Command::TogglePause));
        assert_eq!(engine.commands.try_recv(), Ok(Command::Stop));
        drop(engine);
        assert_eq!(
            sender.send(Command::Stop),
            Err("The playback engine has stopped".into()),
        );
    }
}
