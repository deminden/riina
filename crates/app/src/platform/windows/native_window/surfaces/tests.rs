use super::*;
use crate::NativeVideoHost;

#[test]
fn absent_native_surfaces_never_report_successful_alignment() {
    i_slint_backend_testing::init_no_event_loop();
    let host = NativeVideoHost::new().unwrap();
    let app = PlayerWindow::new().unwrap();
    assert!(!align(host.window(), &app));
}

#[test]
fn unverified_alignment_discards_an_opaque_transition_cover() {
    i_slint_backend_testing::init_no_event_loop();
    let host = NativeVideoHost::new().unwrap();
    let app = PlayerWindow::new().unwrap();
    app.set_native_transition_frame(true);
    let _ = align(host.window(), &app);
    assert!(!app.get_native_transition_frame());
}
