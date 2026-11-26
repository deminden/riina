use super::*;
use riina_ui::PlayerPluginPointerPhase;
use slint::{
    LogicalPosition, LogicalSize,
    platform::{PointerEventButton, WindowEvent},
};

#[test]
fn player_routes_keys_and_video_clicks_without_stealing_protected_input() {
    i_slint_backend_testing::init_no_event_loop();
    let app = PlayerWindow::new().unwrap();
    let bindings = Rc::new(RefCell::new(None));
    let published = bindings.clone();
    app.on_plugin_input_bindings_changed(move |json| {
        published.replace(Some(json));
    });
    crate::key_binding_preferences::install_player(
        &app,
        &riina_core::key_bindings::Settings::default(),
        crate::PlaybackStore::default(),
    );
    let bindings: serde_json::Value =
        serde_json::from_str(bindings.take().unwrap().as_str()).unwrap();
    assert_eq!(bindings["SPACE"]["key"], "SPACE");

    app.set_loaded(true);
    app.window().set_size(LogicalSize::new(900.0, 700.0));
    app.show().unwrap();
    app.invoke_focus_player();
    slint::platform::update_timers_and_animations();
    let controls = app.global::<PlayerControlActions>();
    let events = Rc::new(RefCell::new(Vec::new()));
    let consume = Rc::new(Cell::new(false));
    let key_events = events.clone();
    let consume_key = consume.clone();
    controls.on_begin_plugin_key(move |text, _, _, _, _, _, released| {
        assert_eq!(text.as_str(), "k");
        key_events
            .borrow_mut()
            .push(if released { "released" } else { "before" });
        consume_key.get()
    });
    let binding_events = events.clone();
    controls.on_dispatch_key_binding(move |_, _, _, _, _, _| {
        binding_events.borrow_mut().push("binding");
        true
    });
    let finish_events = events.clone();
    controls.on_finish_plugin_key(move || {
        finish_events.borrow_mut().push("after");
        false
    });
    let key = || {
        app.window()
            .dispatch_event(WindowEvent::KeyPressed { text: "k".into() });
        app.window()
            .dispatch_event(WindowEvent::KeyReleased { text: "k".into() });
        events.take()
    };
    assert_eq!(key(), ["before", "binding", "after", "released"]);
    consume.set(true);
    assert_eq!(key(), ["before", "released"]);
    app.global::<PlayerDialogState>().set_dialog_open(true);
    assert_eq!(key(), ["binding"]);
    app.global::<PlayerDialogState>().set_dialog_open(false);

    let pointer_events = events.clone();
    let consume_pointer = consume.clone();
    controls.on_plugin_pointer(move |phase, code, _, _, clicks| {
        if matches!(
            phase,
            PlayerPluginPointerPhase::Move | PlayerPluginPointerPhase::Cancel
        ) {
            return false;
        }
        assert_eq!(code.as_str(), "*mouse");
        match phase {
            PlayerPluginPointerPhase::Down => pointer_events.borrow_mut().push("mouse down"),
            PlayerPluginPointerPhase::Up => {
                assert!(clicks > 0);
                pointer_events.borrow_mut().push("mouse up");
            }
            _ => {}
        }
        consume_pointer.get()
    });
    let click_events = events.clone();
    controls.on_single_click_action(move |_| click_events.borrow_mut().push("single click"));
    let click = |position: LogicalPosition| {
        app.window().dispatch_event(WindowEvent::PointerPressed {
            position,
            button: PointerEventButton::Left,
        });
        app.window().dispatch_event(WindowEvent::PointerReleased {
            position,
            button: PointerEventButton::Left,
        });
        events.take()
    };
    assert_eq!(
        click(LogicalPosition::new(80.0, 145.0)),
        ["mouse down", "mouse up"]
    );
    consume.set(false);
    assert_eq!(
        click(LogicalPosition::new(110.0, 145.0)),
        ["mouse down", "mouse up", "single click"]
    );
    app.hide().unwrap();
}
