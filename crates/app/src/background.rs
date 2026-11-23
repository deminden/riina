//! Named application work with explicit thread and UI delivery lifetimes.
use riina_core::sync::lock;
use std::{
    collections::BTreeMap,
    io,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
};

static TASKS: Mutex<BTreeMap<u64, String>> = Mutex::new(BTreeMap::new());
static NEXT_TASK: AtomicU64 = AtomicU64::new(0);

pub(crate) type TaskHandle<T> = thread::JoinHandle<T>;

pub(crate) fn spawn_named<T: Send + 'static>(
    name: impl Into<String>,
    work: impl FnOnce() -> T + Send + 'static,
) -> io::Result<TaskHandle<T>> {
    let name = name.into();
    let task = RegisteredTask(NEXT_TASK.fetch_add(1, Ordering::Relaxed));
    lock(&TASKS).insert(task.0, name.clone());
    thread::Builder::new().name(name.clone()).spawn(move || {
        let _task = task;
        match catch_unwind(AssertUnwindSafe(work)) {
            Ok(value) => value,
            Err(panic) => {
                crate::diagnostics::error!("Background task {name} panicked");
                resume_unwind(panic)
            }
        }
    })
}

struct RegisteredTask(u64);

impl Drop for RegisteredTask {
    fn drop(&mut self) {
        lock(&TASKS).remove(&self.0);
    }
}

pub(crate) fn report_running() {
    let running = lock(&TASKS).values().cloned().collect::<Vec<_>>();
    for name in running {
        crate::diagnostics::warn!("Background task {name} is still running at application exit");
    }
}

pub(crate) fn to_ui<T: slint::ComponentHandle + 'static>(
    weak: slint::Weak<T>,
    deliver: impl FnOnce(T) + Send + 'static,
) -> Result<(), slint::EventLoopError> {
    weak.upgrade_in_event_loop(deliver)
}

/// At most one queued callback. The callback reads the latest state when run.
#[derive(Clone, Default)]
pub(crate) struct CoalescedUiCallback(Arc<AtomicBool>);

impl CoalescedUiCallback {
    pub(crate) fn post<F: FnOnce() + Send + 'static>(&self, prepare: impl FnOnce() -> F) {
        if let Some(callback) = self.wrap(prepare) {
            // Once the loop closes, delivery can fail or the queued callback
            // can be dropped. Either path releases the reservation.
            let _ = slint::invoke_from_event_loop(callback);
        }
    }

    fn wrap<F: FnOnce() + Send + 'static>(
        &self,
        prepare: impl FnOnce() -> F,
    ) -> Option<impl FnOnce() + Send + 'static> {
        if self.0.swap(true, Ordering::AcqRel) {
            return None;
        }
        let reservation = UiCallbackReservation(self.0.clone());
        let callback = prepare();
        Some(move || {
            // New work can schedule another delivery while this callback runs.
            drop(reservation);
            callback();
        })
    }
}

struct UiCallbackReservation(Arc<AtomicBool>);

impl Drop for UiCallbackReservation {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, time::Duration};

    #[test]
    fn a_discarded_ui_callback_allows_the_next_delivery() {
        let callback = CoalescedUiCallback::default();
        let discarded = callback.wrap(|| || {}).unwrap();
        assert!(callback.wrap(|| || {}).is_none());
        drop(discarded);

        let next = callback.clone();
        callback
            .wrap(|| move || assert!(next.wrap(|| || {}).is_some()))
            .unwrap()();
    }

    #[test]
    fn the_worker_is_registered_until_it_returns() {
        const NAME: &str = "test-background-lifecycle";
        let (ready, started) = mpsc::sync_channel(1);
        let (release, released) = mpsc::sync_channel(1);
        let worker = spawn_named(NAME, move || {
            ready
                .send(thread::current().name().map(str::to_owned))
                .unwrap();
            released.recv_timeout(Duration::from_secs(2)).unwrap();
        })
        .unwrap();
        assert_eq!(
            started
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .as_deref(),
            Some(NAME)
        );
        assert!(lock(&TASKS).values().any(|name| name == NAME));
        release.send(()).unwrap();
        worker.join().unwrap();
        assert!(!lock(&TASKS).values().any(|name| name == NAME));
    }
}
