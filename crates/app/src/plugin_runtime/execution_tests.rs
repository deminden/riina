use super::*;
use std::time::Instant;

fn wait_for_log(session: &PluginRuntimeSession, expected: &str) {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut received = Vec::new();
    while Instant::now() < deadline {
        received.extend(session.drain_events().unwrap());
        if received.contains(&HostEvent::Log(expected.into())) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("Missing {expected:?}; received {received:?}");
}

#[test]
fn startup_promises_finish_without_an_external_event() {
    let session = PluginRuntimeSession::start_entry(
        "Promise.resolve().then(() => iina.log('startup promise'));",
        "org.example.startup-promise",
        &[],
        &[],
    )
    .unwrap();
    wait_for_log(&session, "startup promise");
}

#[test]
fn long_promise_chains_continue_after_the_job_batch_limit() {
    let session = PluginRuntimeSession::start_entry(
        "let task = Promise.resolve(); for (let i = 0; i < 600; i++) task = task.then(() => {}); task.then(() => iina.log('chain finished'));",
        "org.example.promise-chain",
        &[],
        &[],
    )
    .unwrap();
    wait_for_log(&session, "chain finished");
}

#[test]
fn timers_and_player_and_menu_callbacks_run_their_promise_continuations() {
    let session = PluginRuntimeSession::start_entry(
        r#"
        setTimeout(async () => { await Promise.resolve(); iina.log('timer promise'); }, 0);
        iina.event.on('iina.file-loaded', async () => {
            await Promise.resolve(); iina.log('event promise');
        });
        iina.menu.addItem(iina.menu.item('Async action', async () => {
            await Promise.resolve(); iina.log('menu promise');
        }));
        "#,
        "org.example.callback-promises",
        &[],
        &[],
    )
    .unwrap();
    // Retain the menu identifier while waiting for the timer's microtask.
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut item_id = None;
    let mut timer_ran = false;
    while Instant::now() < deadline && (item_id.is_none() || !timer_ran) {
        for event in session.drain_events().unwrap() {
            match event {
                HostEvent::MenuUpdate(items) => item_id = Some(items[0].id.clone()),
                HostEvent::Log(message) if message == "timer promise" => timer_ran = true,
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(timer_ran, "Timer promise never resumed");
    session
        .deliver_player_event("iina.file-loaded".into(), serde_json::Value::Null)
        .unwrap();
    session.flush_events(Duration::from_secs(3)).unwrap();
    wait_for_log(&session, "event promise");
    session
        .deliver_plugin_menu_action(item_id.unwrap())
        .unwrap();
    session.flush_events(Duration::from_secs(3)).unwrap();
    wait_for_log(&session, "menu promise");
}

#[test]
fn shutdown_interrupts_the_active_callback_and_discards_queued_callbacks() {
    let session = PluginRuntimeSession::start_entry(
        "iina.event.on('iina.file-loaded', () => { iina.log('started'); while (true) {} });",
        "org.example.shutdown-callback",
        &[],
        &[],
    )
    .unwrap();
    session
        .deliver_player_event("iina.file-loaded".into(), serde_json::Value::Null)
        .unwrap();
    wait_for_log(&session, "started");
    for _ in 0..32 {
        session
            .deliver_player_event("iina.file-loaded".into(), serde_json::Value::Null)
            .unwrap();
    }
    let events = session.events.clone();
    let started = Instant::now();
    drop(session);
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(
        !events
            .lock()
            .unwrap()
            .contains(&HostEvent::Log("started".into())),
        "Shutdown executed callbacks queued behind the interrupted callback"
    );
}

#[test]
fn unbounded_promise_chains_share_the_entry_execution_limit() {
    let started = Instant::now();
    let result = PluginRuntimeSession::start_entry(
        "function repeat() { Promise.resolve().then(repeat); } repeat();",
        "org.example.promise-loop",
        &[],
        &[],
    );
    assert!(result.is_err());
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[test]
fn a_failed_overlay_callback_still_runs_its_queued_promise_jobs() {
    let permissions = vec!["video-overlay".to_owned()];
    let session = PluginRuntimeSession::start_entry(
        r#"iina.overlay.simpleMode();
        iina.overlay.onMessage('throw', () => {
            Promise.resolve().then(() => iina.log('promise after throw'));
            throw new Error('overlay failure');
        });"#,
        "org.example.overlay-exception",
        &permissions,
        &permissions,
    )
    .unwrap();
    session.drain_events().unwrap();
    session
        .deliver_overlay_message("throw".into(), serde_json::Value::Null)
        .unwrap();
    session.flush_events(Duration::from_secs(3)).unwrap();
    let events = session.drain_events().unwrap();
    assert!(events.contains(&HostEvent::Log("promise after throw".into())));
    assert!(events.iter().any(
        |event| matches!(event, HostEvent::Log(message) if message.contains("overlay callback failed"))
    ));
}

#[test]
fn package_require_bridge_survives_api_installation() {
    let root = tests::temporary_directory();
    std::fs::write(root.join("entry.js"), "iina.log(require('./helper.js'));").unwrap();
    std::fs::write(root.join("helper.js"), "module.exports = 'module loaded';").unwrap();
    let result = execute_package_entry(&root, &tests::manifest("entry.js".into()), &[]);
    std::fs::remove_dir_all(root).unwrap();
    assert_eq!(result.unwrap(), [HostEvent::Log("module loaded".into())]);
}
