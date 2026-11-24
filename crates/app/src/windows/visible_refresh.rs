use slint::{ComponentHandle, Timer, TimerMode};
use std::{rc::Rc, time::Duration};

pub(crate) fn run<T: ComponentHandle + 'static>(
    window: &T,
    timer: &Rc<Timer>,
    interval: Duration,
    mut refresh: impl FnMut(&T) + 'static,
) {
    let timer_handle = Rc::downgrade(timer);
    let window = window.as_weak();
    timer.start(TimerMode::Repeated, interval, move || {
        if let Some(window) = window
            .upgrade()
            .filter(|window| window.window().is_visible())
        {
            refresh(&window);
        } else if let Some(timer) = timer_handle.upgrade() {
            // Programmatic hides do not invoke the native close handler.
            timer.stop();
        }
    });
}
