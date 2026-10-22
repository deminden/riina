//! One execution budget covers a callback and all of its Promise continuations.

use rquickjs::Runtime;
use std::{
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

const MAX_EXECUTION_TIME: Duration = Duration::from_millis(500);

pub(super) struct ExecutionControl {
    cancelled: AtomicBool,
    deadline: Mutex<Instant>,
}

impl ExecutionControl {
    pub(super) fn new() -> Self {
        Self {
            cancelled: AtomicBool::new(false),
            deadline: Mutex::new(Instant::now() + MAX_EXECUTION_TIME),
        }
    }

    pub(super) fn begin(&self) {
        if let Ok(mut deadline) = self.deadline.lock() {
            *deadline = Instant::now() + MAX_EXECUTION_TIME;
        }
    }

    pub(super) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub(super) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub(super) fn interrupted(&self) -> bool {
        self.is_cancelled()
            || self
                .deadline
                .lock()
                .map_or(true, |deadline| Instant::now() >= *deadline)
    }

    pub(super) fn drain_jobs(&self, runtime: &Runtime) -> Result<(), String> {
        let mut failure = None;
        while runtime.is_job_pending() {
            // A chain of tiny jobs can avoid QuickJS's bytecode interrupt check.
            // Check between jobs as well, without renewing the callback budget.
            if self.interrupted() {
                return Err("Plugin execution was cancelled or exceeded its time limit".into());
            }
            if let Err(error) = runtime.execute_pending_job() {
                // JobException leaves an exception on the originating context.
                error.0.with(|ctx| {
                    ctx.catch();
                });
                failure = Some("Plugin Promise callback failed".to_owned());
            }
        }
        failure.map_or(Ok(()), Err)
    }
}
