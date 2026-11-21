use slint::{CloseRequestResponse, ComponentHandle, Timer, TimerMode};
use std::{rc::Rc, time::Duration};

pub(super) fn start<T: ComponentHandle + 'static>(
    window: &T,
    timer: &Rc<Timer>,
    interval: Duration,
    mut refresh: impl FnMut(&T) + 'static,
) {
    if timer.running() {
        return;
    }
    let timer_handle = Rc::downgrade(timer);
    let close_timer = timer_handle.clone();
    window.window().on_close_requested(move || {
        if let Some(timer) = close_timer.upgrade() {
            timer.stop();
        }
        CloseRequestResponse::HideWindow
    });
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
