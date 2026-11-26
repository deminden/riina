use super::*;
use crate::plugin_runtime::{HostEvent, PluginRuntimeSession, UiEffect};
use riina_ui::PlayerPluginPointerPhase;
use slint::{
    LogicalPosition, LogicalSize,
    platform::{PointerEventButton, WindowEvent},
};
use std::time::Duration;

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

#[test]
fn video_surface_delivers_pointer_events_to_the_plugin_and_honors_consumption() {
    i_slint_backend_testing::init_no_event_loop();
    let app = PlayerWindow::new().unwrap();
    app.set_loaded(true);
    app.window().set_size(LogicalSize::new(900.0, 700.0));
    app.show().unwrap();
    app.invoke_focus_player();
    slint::platform::update_timers_and_animations();

    let session = PluginRuntimeSession::start_entry(
        r#"
        const { input, log } = iina;
        let releases = 0;
        input.onMouseDown(input.MOUSE, ({x}) => { log(`down:${x}`); });
        input.onMouseDrag(input.MOUSE, ({x}) => { log(`drag:${x}`); });
        input.onMouseUp(input.MOUSE, ({x}) => {
            log(`up:${x}`);
            return ++releases <= 2;
        }, input.PRIORITY_HIGH);
        "#,
        "org.test.video-pointer",
        &[],
        &[],
    )
    .unwrap();
    let router = Router::default().with_test_budget(Duration::from_secs(2));
    router.add(&session.input);
    let _input = install(&app, &router);
    let ordinary_clicks = Rc::new(Cell::new(0));
    let actions = ordinary_clicks.clone();
    app.global::<PlayerControlActions>()
        .on_single_click_action(move |_| actions.set(actions.get() + 1));

    let position = |x| LogicalPosition::new(x, 145.0);
    let press = |x| {
        app.window().dispatch_event(WindowEvent::PointerPressed {
            position: position(x),
            button: PointerEventButton::Left,
        })
    };
    let motion = |x| {
        app.window().dispatch_event(WindowEvent::PointerMoved {
            position: position(x),
        })
    };
    let release = |x| {
        app.window().dispatch_event(WindowEvent::PointerReleased {
            position: position(x),
            button: PointerEventButton::Left,
        })
    };
    press(80.0);
    motion(80.0);
    release(80.0);
    assert_eq!(ordinary_clicks.get(), 0);

    press(130.0);
    motion(155.0);
    release(155.0);
    assert_eq!(ordinary_clicks.get(), 0);

    press(190.0);
    release(190.0);
    assert_eq!(ordinary_clicks.get(), 1);

    session.flush_events(Duration::from_secs(2)).unwrap();
    let messages = session
        .drain_events()
        .unwrap()
        .into_iter()
        .filter_map(|event| match event {
            HostEvent::Ui(UiEffect::Log(message)) => Some(message),
            _ => None,
        })
        .collect::<Vec<_>>();
    let expected = [
        "down:80", "up:80", "down:130", "drag:155", "up:155", "down:190", "up:190",
    ];
    assert_eq!(messages.len(), expected.len(), "{messages:?}");
    assert!(
        messages
            .iter()
            .zip(expected)
            .all(|(message, expected)| message.ends_with(expected)),
        "{messages:?}"
    );
    app.hide().unwrap();
}
