use slint::{CloseRequestResponse, ComponentHandle, Timer, TimerMode};
use std::{rc::Rc, time::Duration};

pub(super) fn start<T: ComponentHandle + 'static>(
    window: &T,
    timer: &Rc<Timer>,
    interval: Duration,
    refresh: impl FnMut(&T) + 'static,
) {
    if timer.running() {
        return;
    }
    let close_timer = Rc::downgrade(timer);
    window.window().on_close_requested(move || {
        if let Some(timer) = close_timer.upgrade() {
            timer.stop();
        }
        CloseRequestResponse::HideWindow
    });
    run(window, timer, interval, refresh);
}

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
