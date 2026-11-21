//! Named application work with explicit thread and UI delivery lifetimes.
use riina_core::sync::lock;
use std::{
    collections::BTreeMap,
    io,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, time::Duration};

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
