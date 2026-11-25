//! Poll an active native service until it has no remaining work.
use slint::{Timer, TimerMode};
use std::{rc::Rc, time::Duration};

#[derive(Default)]
pub(super) struct ActivePoll(Rc<Timer>);

impl ActivePoll {
    pub(super) fn start_if_idle(
        &self,
        interval: Duration,
        mut poll: impl FnMut() -> Option<Duration> + 'static,
    ) {
        if self.0.running() {
            return;
        }
        let weak = Rc::downgrade(&self.0);
        self.0.start(TimerMode::Repeated, interval, move || {
            let Some(timer) = weak.upgrade() else { return };
            if let Some(interval) = poll() {
                if timer.interval() != interval {
                    timer.set_interval(interval);
                }
            } else {
                timer.stop();
            }
        });
    }

    pub(super) fn stop(&self) {
        self.0.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn polling_changes_cadence_only_while_active_and_can_wake_again() {
        i_slint_backend_testing::init_no_event_loop();
        const ONSCREEN: Duration = Duration::from_millis(100);
        const OFFSCREEN: Duration = Duration::from_millis(33);
        let poll = ActivePoll::default();
        let count = Rc::new(Cell::new(0));
        let ticks = count.clone();
        poll.start_if_idle(ONSCREEN, move || {
            let completed = ticks.get() + 1;
            ticks.set(completed);
            (completed == 1).then_some(OFFSCREEN)
        });
        poll.start_if_idle(ONSCREEN, || {
            panic!("The current polling job owns the timer")
        });

        i_slint_backend_testing::mock_elapsed_time(ONSCREEN);
        assert_eq!(count.get(), 1);
        i_slint_backend_testing::mock_elapsed_time(OFFSCREEN);
        assert_eq!(count.get(), 2);
        i_slint_backend_testing::mock_elapsed_time(ONSCREEN);
        assert_eq!(count.get(), 2);

        let ticks = count.clone();
        poll.start_if_idle(ONSCREEN, move || {
            ticks.set(ticks.get() + 1);
            Some(ONSCREEN)
        });
        i_slint_backend_testing::mock_elapsed_time(ONSCREEN);
        assert_eq!(count.get(), 3);
        poll.stop();
        i_slint_backend_testing::mock_elapsed_time(ONSCREEN);
        assert_eq!(count.get(), 3);
    }
}
