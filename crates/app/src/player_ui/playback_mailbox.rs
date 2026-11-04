//! Drain owned observations before clipboard work, UI callbacks or native transitions.
use crate::screenshot_output::Completions;
use riina_core::Playback;
use std::sync::Mutex;

#[derive(Default)]
struct Pending {
    state: Option<Playback>,
    captures: Completions,
}

#[derive(Default)]
pub(super) struct Mailbox(Mutex<Pending>);

pub(super) struct Delivery {
    pub state: Playback,
    pub captures: Vec<String>,
}

impl Mailbox {
    pub fn publish(&self, state: Playback) {
        if let Ok(mut pending) = self.0.lock() {
            pending.captures.observe(&state);
            pending.state = Some(state);
        }
    }

    pub fn take(&self) -> Option<Delivery> {
        let mut pending = self.0.lock().ok()?;
        Some(Delivery {
            state: pending.state.take()?,
            captures: pending.captures.take(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn coalescing_preserves_each_capture_without_replaying_it() {
        let mailbox = Mailbox::default();
        for count in 1..=3 {
            mailbox.publish(Playback {
                completed_screenshots: count,
                screenshot_file: Some(format!("capture-{count}.png")),
                ..Default::default()
            });
        }
        let update = mailbox.take().unwrap();
        assert_eq!(update.state.completed_screenshots, 3);
        assert_eq!(
            update.captures,
            ["capture-1.png", "capture-2.png", "capture-3.png"]
        );
        assert!(mailbox.take().is_none());
        mailbox.publish(update.state);
        assert!(mailbox.take().unwrap().captures.is_empty());
    }

    #[test]
    fn worker_can_publish_while_ui_retains_the_previous_delivery() {
        let mailbox = Arc::new(Mailbox::default());
        mailbox.publish(Playback {
            generation: 1,
            ..Default::default()
        });
        let displayed = mailbox.take().unwrap();
        let worker = mailbox.clone();
        std::thread::spawn(move || {
            worker.publish(Playback {
                generation: 2,
                ..Default::default()
            });
        })
        .join()
        .unwrap();
        assert_eq!(displayed.state.generation, 1);
        assert_eq!(mailbox.take().unwrap().state.generation, 2);
    }
}
