//! Drain owned observations before clipboard work, UI callbacks or native transitions.
use crate::screenshot_output::Completions;
use riina_core::{Playback, sync::lock};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Pending {
    state: Option<Arc<Playback>>,
    captures: Completions,
}

#[derive(Default)]
pub(super) struct Mailbox(Mutex<Pending>);

pub(super) struct Delivery {
    pub state: Arc<Playback>,
    pub captures: Vec<String>,
}

impl Mailbox {
    pub fn publish(&self, state: Arc<Playback>) {
        let mut pending = lock(&self.0);
        pending.captures.observe(&state);
        pending.state = Some(state);
    }

    pub fn take(&self) -> Option<Delivery> {
        let mut pending = lock(&self.0);
        Some(Delivery {
            state: pending.state.take()?,
            captures: pending.captures.take(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coalescing_preserves_each_capture_without_replaying_it() {
        let mailbox = Mailbox::default();
        for count in 1..=3 {
            mailbox.publish(Arc::new(Playback {
                completed_screenshots: count,
                screenshot_file: Some(format!("capture-{count}.png")),
                ..Default::default()
            }));
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
        let published = Arc::new(Playback {
            generation: 1,
            ..Default::default()
        });
        mailbox.publish(published.clone());
        let displayed = mailbox.take().unwrap();
        assert!(Arc::ptr_eq(&displayed.state, &published));
        let worker = mailbox.clone();
        std::thread::spawn(move || {
            worker.publish(Arc::new(Playback {
                generation: 2,
                ..Default::default()
            }));
        })
        .join()
        .unwrap();
        assert_eq!(displayed.state.generation, 1);
        assert_eq!(mailbox.take().unwrap().state.generation, 2);
    }
}
