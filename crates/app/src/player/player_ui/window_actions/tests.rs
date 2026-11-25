use super::*;
use crate::automatic_music_mode;
use crate::{NativeVideoHost, player_ui::fullscreen};
use slint::ComponentHandle;
use std::{cell::RefCell, rc::Rc};

fn install_fullscreen(app: &PlayerWindow, host: Option<slint::Weak<NativeVideoHost>>) {
    fullscreen::install(
        app,
        host,
        &automatic_music_mode::Controller::install(app, true),
    );
}

fn player(fullscreen: bool) -> PlayerWindow {
    let app = PlayerWindow::new().unwrap();
    app.set_loaded(true);
    app.set_fullscreen(fullscreen);
    app.set_paused(!fullscreen);
    app.global::<riina_ui::PlayerPreferences>()
        .set_play_when_entering_fullscreen(true);
    app.global::<riina_ui::PlayerPreferences>()
        .set_pause_when_leaving_fullscreen(true);
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
        app.global::<riina_ui::PlayerControlActions>()
            .on_set_paused(move |paused| observed.borrow_mut().push(paused));
        install_fullscreen(&app, Some(host.as_weak()));
        app.global::<PlayerMenuActions>().invoke_toggle_fullscreen();
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
        app.global::<riina_ui::PlayerControlActions>()
            .on_set_paused(move |paused| {
                let app = weak.upgrade().unwrap();
                observed.borrow_mut().push((app.get_fullscreen(), paused));
            });
        install_fullscreen(&app, Some(host.as_weak()));
        app.global::<PlayerMenuActions>().invoke_toggle_fullscreen();
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
    app.global::<PlayerMenuActions>().invoke_toggle_fullscreen();
    assert!(!app.get_fullscreen());
}

#[test]
fn fullscreen_preferences_do_not_change_playback_when_disabled_or_unloaded() {
    i_slint_backend_testing::init_no_event_loop();
    for loaded in [false, true] {
        let app = player(false);
        app.set_loaded(loaded);
        app.global::<riina_ui::PlayerPreferences>()
            .set_play_when_entering_fullscreen(false);
        let host = NativeVideoHost::new().unwrap();
        host.on_set_fullscreen_window(|_| true);
        app.global::<riina_ui::PlayerControlActions>()
            .on_set_paused(|_| panic!("Playback policy should be inactive"));
        install_fullscreen(&app, Some(host.as_weak()));
        app.global::<PlayerMenuActions>().invoke_toggle_fullscreen();
        assert!(app.get_fullscreen());
    }
}

#[test]
fn native_fullscreen_notification_commits_playback_and_closes_pip_only_once() {
    i_slint_backend_testing::init_no_event_loop();
    let app = player(false);
    install_fullscreen(&app, None);
    let effects = Rc::new(RefCell::new(Vec::new()));
    let paused = effects.clone();
    app.global::<riina_ui::PlayerControlActions>()
        .on_set_paused(move |paused_value| {
            paused
                .borrow_mut()
                .push(if paused_value { "pause" } else { "play" })
        });
    app.set_picture_in_picture_active(true);
    let pip = effects.clone();
    app.global::<PlayerMenuActions>()
        .on_toggle_picture_in_picture(move || pip.borrow_mut().push("close pip"));

    app.invoke_native_fullscreen_observed(true, false);
    assert!(app.get_entering_fullscreen());
    assert!(!app.get_fullscreen());
    assert!(effects.borrow().is_empty());
    app.invoke_native_fullscreen_observed(true, true);
    app.invoke_native_fullscreen_observed(true, true);
    assert!(app.get_fullscreen());
    assert!(!app.get_entering_fullscreen());
    assert_eq!(effects.borrow().as_slice(), ["play", "close pip"]);
}

#[test]
fn confirmed_fullscreen_exit_cancels_crop_before_applying_the_pause_policy() {
    use riina_ui::{PlayerCropState, PlayerWindowMode};
    i_slint_backend_testing::init_no_event_loop();
    let app = player(true);
    install_fullscreen(&app, None);
    app.set_paused(true);
    let crop = app.global::<PlayerCropState>();
    crop.set_crop_editing(true);
    let effects = Rc::new(RefCell::new(Vec::new()));
    let canceled = effects.clone();
    let weak = app.as_weak();
    crop.on_finish_custom_crop(move |confirm| {
        assert!(!confirm);
        let app = weak.upgrade().unwrap();
        assert!(app.get_fullscreen());
        app.global::<PlayerCropState>().set_crop_editing(false);
        canceled.borrow_mut().push("cancel crop");
    });
    let published = effects.clone();
    app.on_window_mode_confirmed(move |mode, active| {
        assert_eq!(mode, PlayerWindowMode::Fullscreen);
        assert!(!active);
        published.borrow_mut().push("windowed");
    });
    let paused = effects.clone();
    app.global::<riina_ui::PlayerControlActions>()
        .on_set_paused(move |pause| {
            assert!(pause);
            paused.borrow_mut().push("pause");
        });
    app.invoke_native_fullscreen_observed(false, false);
    assert!(effects.borrow().is_empty());
    app.invoke_native_fullscreen_observed(false, true);
    app.invoke_native_fullscreen_observed(false, true);
    assert_eq!(
        effects.borrow().as_slice(),
        ["cancel crop", "windowed", "pause"]
    );
}

#[test]
fn queued_music_mode_clears_after_cancellation_and_waits_for_a_real_exit() {
    use riina_ui::PlayerMusicModeState;
    i_slint_backend_testing::init_no_event_loop();
    let app = player(true);
    let host = NativeVideoHost::new().unwrap();
    host.on_set_fullscreen_window(|_| false);
    let state = automatic_music_mode::Controller::install(&app, true);
    fullscreen::install(&app, Some(host.as_weak()), &state);
    let menu = app.global::<PlayerMenuActions>();
    menu.invoke_toggle_music_mode();
    assert!(app.get_fullscreen());
    assert!(!app.global::<PlayerMusicModeState>().get_music_mode());

    app.invoke_native_fullscreen_observed(false, false);
    menu.invoke_toggle_music_mode();
    menu.invoke_toggle_music_mode();
    assert!(!app.global::<PlayerMusicModeState>().get_music_mode());
    app.invoke_native_fullscreen_observed(false, true);
    assert!(!app.global::<PlayerMusicModeState>().get_music_mode());

    app.invoke_native_fullscreen_observed(true, true);
    app.invoke_native_fullscreen_observed(false, false);
    menu.invoke_toggle_music_mode();
    assert!(app.get_fullscreen());
    assert!(!app.global::<PlayerMusicModeState>().get_music_mode());
    app.invoke_native_fullscreen_observed(false, true);
    assert!(app.global::<PlayerMusicModeState>().get_music_mode());
    menu.invoke_request_fullscreen(false);
    menu.invoke_request_fullscreen(true);
    assert!(app.global::<PlayerMusicModeState>().get_music_mode());
    assert!(!app.get_fullscreen());
}
