use super::*;
use std::{cell::RefCell, rc::Rc};

fn player(fullscreen: bool) -> PlayerWindow {
    let app = PlayerWindow::new().unwrap();
    app.set_loaded(true);
    app.set_fullscreen(fullscreen);
    app.set_paused(!fullscreen);
    app.set_play_when_entering_fullscreen(true);
    app.set_pause_when_leaving_fullscreen(true);
    app
}

#[test]
fn rejected_fullscreen_transition_preserves_playback_in_both_directions() {
    i_slint_backend_testing::init_no_event_loop();
    for fullscreen in [false, true] {
        let app = player(fullscreen);
        let host = NativeVideoHost::new().unwrap();
        host.on_set_fullscreen_window(|_| false);
        let pauses = Rc::new(RefCell::new(Vec::new()));
        let observed = pauses.clone();
        app.on_set_paused(move |paused| observed.borrow_mut().push(paused));
        install_fullscreen(&app, Some(host.as_weak()));
        app.invoke_toggle_fullscreen();
        assert_eq!(app.get_fullscreen(), fullscreen);
        assert!(
            pauses.borrow().is_empty(),
            "Rejected transition changed playback"
        );
    }
}

#[test]
fn accepted_fullscreen_transition_commits_window_state_before_playback_policy() {
    i_slint_backend_testing::init_no_event_loop();
    for fullscreen in [false, true] {
        let app = player(fullscreen);
        let host = NativeVideoHost::new().unwrap();
        host.on_set_fullscreen_window(move |target| {
            assert_ne!(target, fullscreen);
            true
        });
        let pauses = Rc::new(RefCell::new(Vec::new()));
        let observed = pauses.clone();
        let weak = app.as_weak();
        app.on_set_paused(move |paused| {
            let app = weak.upgrade().unwrap();
            observed.borrow_mut().push((app.get_fullscreen(), paused));
        });
        install_fullscreen(&app, Some(host.as_weak()));
        app.invoke_toggle_fullscreen();
        assert_eq!(app.get_fullscreen(), !fullscreen);
        assert_eq!(*pauses.borrow(), vec![(!fullscreen, fullscreen)]);
    }
}

#[test]
fn missing_native_owner_cannot_fullscreen_the_controls_popup() {
    i_slint_backend_testing::init_no_event_loop();
    let app = player(false);
    let host = NativeVideoHost::new().unwrap();
    let weak = host.as_weak();
    drop(host);
    install_fullscreen(&app, Some(weak));
    app.invoke_toggle_fullscreen();
    assert!(!app.get_fullscreen());
}

#[test]
fn fullscreen_preferences_do_not_change_playback_when_disabled_or_unloaded() {
    i_slint_backend_testing::init_no_event_loop();
    for loaded in [false, true] {
        let app = player(false);
        app.set_loaded(loaded);
        app.set_play_when_entering_fullscreen(false);
        let host = NativeVideoHost::new().unwrap();
        host.on_set_fullscreen_window(|_| true);
        app.on_set_paused(|_| panic!("Playback policy should be inactive"));
        install_fullscreen(&app, Some(host.as_weak()));
        app.invoke_toggle_fullscreen();
        assert!(app.get_fullscreen());
    }
}
