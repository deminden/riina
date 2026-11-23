//! Publish the need for a fast presenter tick without coupling the host to Slint.
use riina_core::sync::lock;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

pub(super) struct UiActivity {
    active: AtomicBool,
    wake: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}

impl UiActivity {
    pub(super) fn new() -> Self {
        // Let the initial presenter ticks run while enabled sessions load.
        Self {
            active: AtomicBool::new(true),
            wake: Mutex::new(None),
        }
    }

    pub(super) fn has_active_sessions(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    pub(super) fn set_waker(&self, wake: impl Fn() + Send + Sync + 'static) {
        *lock(&self.wake) = Some(Arc::new(wake));
    }

    pub(super) fn publish(&self, active: bool, messages_pending: impl FnOnce() -> bool) {
        let was_active = self.active.swap(active, Ordering::AcqRel);
        if !was_active && (active || messages_pending()) {
            // Call out without the lock so UI delivery can overlap registration.
            let wake = lock(&self.wake).clone();
            if let Some(wake) = wake {
                wake();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn waking_from_idle_covers_messages_and_a_session_without_output() {
        let activity = UiActivity::new();
        let wake_count = Arc::new(AtomicUsize::new(0));
        let called = wake_count.clone();
        activity.set_waker(move || {
            called.fetch_add(1, Ordering::Relaxed);
        });

        activity.publish(false, || false);
        assert!(!activity.has_active_sessions());
        activity.publish(false, || false);
        assert_eq!(wake_count.load(Ordering::Relaxed), 0);

        activity.publish(false, || true);
        assert_eq!(wake_count.load(Ordering::Relaxed), 1);
        activity.publish(true, || false);
        assert!(activity.has_active_sessions());
        assert_eq!(wake_count.load(Ordering::Relaxed), 2);

        activity.publish(true, || true);
        activity.publish(false, || true);
        assert!(!activity.has_active_sessions());
        assert_eq!(wake_count.load(Ordering::Relaxed), 2);
    }
}
