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
    let monitor = VisibilityMonitor::install(window.0.0 as isize, &alignment, &materials)?;
    let running = || (alignment.running(), materials.running());
    assert_eq!(running(), (false, false));

    let show = |action| {
        // ShowWindow returns its prior visibility, which the test checks separately.
        // SAFETY: this test retains the live window on its owner thread.
        let _ = unsafe { ShowWindow(window.0, action) };
    };
    show(SW_SHOWNOACTIVATE);
    assert_eq!(running(), (true, true));
    i_slint_backend_testing::mock_elapsed_time(interval);
    assert_eq!(ticks.get(), 1);

    show(SW_SHOWMINNOACTIVE);
    assert_eq!(running(), (false, false));
    i_slint_backend_testing::mock_elapsed_time(interval);
    assert_eq!(ticks.get(), 1);

    show(SW_SHOWNOACTIVATE);
    assert_eq!(running(), (true, true));
    i_slint_backend_testing::mock_elapsed_time(interval);
    assert_eq!(ticks.get(), 2);

    show(SW_HIDE);
    assert_eq!(running(), (false, false));
    show(SW_SHOWNOACTIVATE);
    assert_eq!(running(), (true, true));
    drop(monitor);
    show(SW_HIDE);
    show(SW_SHOWNOACTIVATE);
    assert_eq!(running(), (false, false));
    Ok(())
}
