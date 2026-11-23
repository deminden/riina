use super::*;
use crate::plugin_runtime::{HostEvent, PluginRuntimeSession, UiEffect};

fn start(id: &str, code: &str) -> PluginRuntimeSession {
    PluginRuntimeSession::start_entry(code, id, &[], &[]).unwrap()
}

fn event(router: &Router, kind: Kind, code: &str, arguments: Arguments) -> Dispatch {
    let mut event = router.begin(kind, code.to_owned(), arguments).unwrap();
    // Parallel workspace tests may deschedule a worker for more than one frame.
    event.deadline = Instant::now() + Duration::from_secs(2);
    event
}

fn logs(session: &PluginRuntimeSession) -> Vec<String> {
    session.flush_events(Duration::from_secs(2)).unwrap();
    session
        .drain_events()
        .unwrap()
        .into_iter()
        .map(|event| match event {
            HostEvent::Ui(UiEffect::Log(message)) => message,
            other => panic!("Unexpected plugin event {other:?}"),
        })
        .collect()
}

#[test]
fn prioritized_key_listeners_can_replace_remove_and_consume_bindings() {
    let first = start(
        "org.test.input.low",
        r#"
        const { input, log } = iina;
        input.onKeyDown('Shift+k', () => log('replaced'));
        input.onKeyDown('K', ({x, y, isRepeat}) => {
            log(`low:${x},${y},${isRepeat}`);
            input.onKeyDown('K', null);
            return true;
        });
        "#,
    );
    let second = start(
        "org.test.input.high",
        r#"
        const { input, log } = iina;
        input.onKeyDown('K', () => { log('high'); }, input.PRIORITY_HIGH);
        input.onKeyUp('K', () => true, input.PRIORITY_HIGH);
        "#,
    );
    let router = Router::default();
    router.add(&first.input);
    router.add(&second.input);
    let position = Arguments::Key {
        x: 22.0,
        y: 68.0,
        is_repeat: true,
    };
    let press = event(&router, Kind::KeyDown, "K", position);
    assert!(!press.handled(true));
    assert!(logs(&first).is_empty());
    assert!(logs(&second).iter().any(|line| line.ends_with("high")));
    assert!(press.handled(false));
    assert!(
        logs(&first)
            .iter()
            .any(|line| line.ends_with("low:22,68,true"))
    );
    assert!(!event(&router, Kind::KeyDown, "K", position).handled(false));
    assert!(event(&router, Kind::KeyUp, "K", position).handled(true));
    assert!(
        Router::default()
            .begin(Kind::KeyDown, "K".into(), position)
            .is_none()
    );
    drop(press);
    drop(second);
    assert!(router.begin(Kind::KeyDown, "K".into(), position).is_none());
}

#[test]
fn mouse_listeners_receive_clicks_and_read_the_current_profile_snapshot() {
    let primary = start(
        "org.test.input.mouse",
        r#"
        const { input, log } = iina;
        input.onMouseUp(input.RIGHT_MOUSE, ({x, y, clickCount, pressure}) => {
            const binding = input.getAllKeyBindings().SPACE;
            log(`${x},${y},${clickCount},${pressure};${binding.action};${binding.isIINACommand}`);
            return true;
        }, 999);
        "#,
    );
    let secondary = start(
        "org.test.input.secondary",
        "iina.input.onMouseUp(iina.input.RIGHT_MOUSE, () => iina.log('unexpected'), 200);",
    );
    let router = Router::default();
    router.add(&secondary.input);
    router.add(&primary.input);
    let position = Arguments::Mouse {
        x: 100.0,
        y: 150.0,
        click_count: 2,
        pressure: 0.0,
    };
    for action in ["loadfile", "cycle pause"] {
        primary.input.bindings.replace(
            serde_json::json!({
                "SPACE": { "key": "SPACE", "action": action, "isIINACommand": true }
            })
            .to_string(),
        );
        assert!(event(&router, Kind::MouseUp, "*rightMouse", position).handled(true));
        let suffix = format!("100,150,2,0;{action};true");
        assert!(logs(&primary).iter().any(|line| line.ends_with(&suffix)));
    }
    assert!(logs(&secondary).is_empty());
}
