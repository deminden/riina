//! Worker-owned timer deadlines and JavaScript callback handles.
use rquickjs::{Ctx, function::Func};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};
const MAX_ACTIVE_TIMERS: usize = 256;
const MAX_TIMER_DELAY: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Default)]
pub(super) struct PluginTimers {
    next_id: AtomicU32,
    deadlines: Mutex<BTreeMap<u32, Instant>>,
}

impl PluginTimers {
    fn schedule(&self, delay_ms: f64) -> Result<u32, String> {
        if !delay_ms.is_finite() || !(0.0..=MAX_TIMER_DELAY.as_millis() as f64).contains(&delay_ms)
        {
            return Err("Plugin timer delay must be between 0 and 86400000 milliseconds".into());
        }
        let delay = Duration::from_secs_f64(delay_ms / 1000.0);
        let mut deadlines = self
            .deadlines
            .lock()
            .map_err(|_| "Plugin timer queue was poisoned".to_owned())?;
        if deadlines.len() >= MAX_ACTIVE_TIMERS {
            return Err("Plugin has reached the 256 active timer limit".into());
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        deadlines.insert(id, Instant::now() + delay);
        Ok(id)
    }

    fn cancel(&self, id: u32) {
        if let Ok(mut deadlines) = self.deadlines.lock() {
            deadlines.remove(&id);
        }
    }

    pub(super) fn next_wait(&self) -> Option<Duration> {
        self.deadlines
            .lock()
            .ok()?
            .values()
            .copied()
            .min()
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
    }

    pub(super) fn take_due(&self) -> Option<u32> {
        let mut deadlines = self.deadlines.lock().ok()?;
        let now = Instant::now();
        let id = deadlines
            .iter()
            .filter(|(_, deadline)| **deadline <= now)
            .min_by_key(|(_, deadline)| **deadline)
            .map(|(id, _)| *id)?;
        deadlines.remove(&id);
        Some(id)
    }
}

pub(super) fn install(ctx: Ctx<'_>, timers: Arc<PluginTimers>) -> rquickjs::Result<()> {
    let globals = ctx.globals();
    let timer_queue = timers.clone();
    globals.set(
        "__riinaScheduleTimer",
        Func::from(move |delay_ms: f64| -> rquickjs::Result<u32> {
            timer_queue.schedule(delay_ms).map_err(timer_js_error)
        }),
    )?;
    let timer_queue = timers.clone();
    globals.set(
        "__riinaCancelTimer",
        Func::from(move |id: u32| {
            timer_queue.cancel(id);
        }),
    )?;
    ctx.eval::<(), _>(
    r#"(() => {
        const scheduleNative = __riinaScheduleTimer;
        const cancelNative = __riinaCancelTimer;
        const active = new Map();
        const callbacks = new Map();
        let nextHandle = 1;
        const schedule = handle => {
            const timer = active.get(handle);
            if (!timer) return;
            const nativeId = scheduleNative(timer.delay);
            timer.nativeId = nativeId;
            callbacks.set(nativeId, () => {
                if (!active.has(handle)) return;
                if (!timer.interval) active.delete(handle);
                try { timer.callback.call(globalThis); }
                finally { if (timer.interval && active.has(handle)) schedule(handle); }
            });
        };
        const createTimer = (callback, delay, interval) => {
            if (typeof callback !== 'function') throw new TypeError('callback must be a function');
            const milliseconds = Number(delay);
            const normalized = Number.isFinite(milliseconds) ? Math.max(interval ? 1 : 0, milliseconds) : 0;
            const handle = 'riina-timer-' + (nextHandle++).toString(36);
            active.set(handle, { callback, delay: normalized, interval, nativeId: 0 });
            try { schedule(handle); } catch (error) { active.delete(handle); throw error; }
            return handle;
        };
        const clearTimer = handle => {
            const key = String(handle);
            const timer = active.get(key);
            if (!timer) return;
            active.delete(key);
            callbacks.delete(timer.nativeId);
            cancelNative(timer.nativeId);
        };
        globalThis.setTimeout = (callback, delay) => createTimer(callback, delay, false);
        globalThis.setInterval = (callback, delay) => createTimer(callback, delay, true);
        globalThis.clearTimeout = clearTimer;
        globalThis.clearInterval = clearTimer;
        globalThis.__riinaTimers = { fire: id => {
            const callback = callbacks.get(id);
            callbacks.delete(id);
            if (callback) callback();
        }};
    })();"#,
)?;
    Ok(())
}

fn timer_js_error(message: String) -> rquickjs::Error {
    rquickjs::Error::new_from_js_message("timer delay", "finite delay up to one day", message)
}
