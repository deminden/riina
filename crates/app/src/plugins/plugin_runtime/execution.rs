//! One execution budget covers a callback and all of its Promise continuations.

use rquickjs::Runtime;
use std::{
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, RecvTimeoutError},
    },
    time::{Duration, Instant},
};

const MAX_EXECUTION_TIME: Duration = Duration::from_millis(500);

pub(super) struct ExecutionControl {
    cancelled: AtomicBool,
    timing: Mutex<Timing>,
}

struct Timing {
    deadline: Instant,
    waiting_since: Option<Instant>,
    waited: Duration,
}

impl ExecutionControl {
    pub(super) fn new() -> Self {
        Self {
            cancelled: AtomicBool::new(false),
            timing: Mutex::new(Timing {
                deadline: Instant::now() + MAX_EXECUTION_TIME,
                waiting_since: None,
                waited: Duration::ZERO,
            }),
        }
    }

    pub(super) fn begin(&self) {
        // A poisoned execution budget stays failed closed in interrupted().
        if let Ok(mut timing) = self.timing.lock() {
            timing.deadline = Instant::now() + MAX_EXECUTION_TIME;
        }
    }

    pub(super) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub(super) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub(super) fn interrupted(&self) -> bool {
        self.deadline()
            .is_none_or(|deadline| Instant::now() >= deadline)
    }

    pub(super) fn deadline(&self) -> Option<Instant> {
        if self.is_cancelled() {
            return None;
        }
        let timing = self.timing.lock().ok()?;
        Some(timing.deadline + timing.current_wait())
    }

    pub(super) fn wait_for_user(&self) -> Option<UserWait<'_>> {
        let mut timing = self.timing.lock().ok()?;
        let now = Instant::now();
        if self.is_cancelled() || now >= timing.deadline || timing.waiting_since.is_some() {
            return None;
        }
        timing.waiting_since = Some(now);
        Some(UserWait(self))
    }

    pub(super) fn user_wait_time(&self) -> Duration {
        self.timing.lock().map_or(Duration::ZERO, |timing| {
            timing.waited + timing.current_wait()
        })
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

impl Timing {
    fn current_wait(&self) -> Duration {
        self.waiting_since
            .map_or(Duration::ZERO, |since| since.elapsed())
    }
}

/// Suspend only time spent waiting on a user decision. The JS turn resumes
/// with its previous budget, including any work that happened before the dialog.
pub(super) struct UserWait<'a>(&'a ExecutionControl);

impl Drop for UserWait<'_> {
    fn drop(&mut self) {
        if let Ok(mut timing) = self.0.timing.lock()
            && let Some(start) = timing.waiting_since.take()
        {
            let elapsed = start.elapsed();
            timing.deadline += elapsed;
            timing.waited += elapsed;
        }
    }
}

pub(super) fn wait_for_startup(
    execution: &ExecutionControl,
    ready: Receiver<Result<(), String>>,
    budget: Duration,
) -> Result<Result<(), String>, RecvTimeoutError> {
    let started = Instant::now();
    let earlier_wait = execution.user_wait_time();
    loop {
        let interaction = execution.user_wait_time().saturating_sub(earlier_wait);
        let elapsed = started.elapsed().saturating_sub(interaction);
        let remaining = budget.saturating_sub(elapsed);
        match ready.recv_timeout(remaining.min(Duration::from_millis(100))) {
            Err(RecvTimeoutError::Timeout) if !remaining.is_zero() => {}
            result => return result,
        }
    }
}
