use super::*;
use std::{cell::Cell, time::Duration};
use windows::{
    Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, SW_HIDE, SW_SHOWMINNOACTIVE, SW_SHOWNOACTIVATE, ShowWindow,
        WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
    },
    core::w,
};

struct TestWindow(HWND);

impl Drop for TestWindow {
    fn drop(&mut self) {
        // SAFETY: this test owns the HWND on its creating thread.
        let _ = unsafe { DestroyWindow(self.0) };
    }
}

#[test]
fn native_visibility_pauses_and_restores_presentation_timers()
-> Result<(), Box<dyn std::error::Error>> {
    i_slint_backend_testing::init_no_event_loop();
    // SAFETY: the system class and title are static. This hidden offscreen window never activates.
    let window = TestWindow(unsafe {
        CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
            w!("STATIC"),
            w!("Riina visibility regression"),
            WS_POPUP,
            -30_000,
            -30_000,
            100,
            100,
            None,
            None,
            None,
            None,
        )?
    });
    let alignment = Rc::new(Timer::default());
    let materials = Rc::new(Timer::default());
    let hdr_display = Rc::new(Timer::default());
    let ticks = Rc::new(Cell::new(0));
    let reported_ticks = ticks.clone();
    let interval = Duration::from_millis(16);
    alignment.start(slint::TimerMode::Repeated, interval, move || {
        reported_ticks.set(reported_ticks.get() + 1);
    });
    materials.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(500),
        || {},
    );
    let hdr_samples = Rc::new(Cell::new(0));
    let recorded_samples = hdr_samples.clone();
    let hdr_interval = Duration::from_secs(2);
    hdr_display.start(slint::TimerMode::Repeated, hdr_interval, move || {
        recorded_samples.set(recorded_samples.get() + 1);
    });
    let timers = [&alignment, &materials, &hdr_display];
    let monitor = VisibilityMonitor::install(window.0.0 as isize, timers)?;
    let minimized_changes = Rc::new(RefCell::new(Vec::new()));
    let observed = minimized_changes.clone();
    monitor.on_minimized(move |minimized| observed.borrow_mut().push(minimized));
    let running = || timers.map(|timer| timer.running());
    assert_eq!(running(), [false; 3]);

    let show = |action| {
        // ShowWindow returns its prior visibility, which the test checks separately.
        // SAFETY: this test retains the live window on its owner thread.
        let _ = unsafe { ShowWindow(window.0, action) };
    };
    show(SW_SHOWNOACTIVATE);
    assert_eq!(running(), [true; 3]);
    i_slint_backend_testing::mock_elapsed_time(interval);
    assert_eq!(ticks.get(), 1);

    show(SW_SHOWMINNOACTIVE);
    assert_eq!(running(), [false; 3]);
    i_slint_backend_testing::mock_elapsed_time(hdr_interval);
    assert_eq!(ticks.get(), 1);
    assert_eq!(hdr_samples.get(), 0);

    show(SW_SHOWNOACTIVATE);
    assert_eq!(running(), [true; 3]);
    i_slint_backend_testing::mock_elapsed_time(interval);
    assert_eq!(ticks.get(), 2);
    i_slint_backend_testing::mock_elapsed_time(hdr_interval - interval);
    assert_eq!(hdr_samples.get(), 1);

    show(SW_HIDE);
    assert_eq!(running(), [false; 3]);
    show(SW_SHOWNOACTIVATE);
    assert_eq!(running(), [true; 3]);
    drop(monitor);
    show(SW_HIDE);
    show(SW_SHOWNOACTIVATE);
    assert_eq!(running(), [false; 3]);
    i_slint_backend_testing::mock_elapsed_time(hdr_interval);
    assert_eq!(hdr_samples.get(), 1);
    assert_eq!(*minimized_changes.borrow(), [true, false]);
    Ok(())
}
