use crate::plugin_runtime::{HostEvent, PluginRuntimeSession, PluginUi, UiEffect};
use riina_core::plugin_window::Document;
use riina_test_support::TempDir;
use std::{fs, time::Duration};

#[test]
fn player_and_global_windows_have_independent_documents_and_retirement() {
    let package = TempDir::new("standalone-windows").unwrap();
    let mut manifest = super::super::tests::manifest("main.js".into());
    manifest.global_entry = Some("global.js".into());
    fs::write(
        package.path().join("global.js"),
        r"
        const view = iina.standaloneWindow;
        view.simpleMode();
        view.setContent('Global content');
        view.open();
        view.onMessage('echo', data => iina.log(data.text));
        ",
    )
    .unwrap();
    fs::write(
        package.path().join("main.js"),
        r"
        const view = iina.standaloneWindow;
        try { view.loadFile('../outside.html'); } catch (_) { iina.log('confined'); }
        view.loadFile('page.html');
        view.setProperty({ title: 'Child window', resizable: false });
        view.setFrame(640, null, 20, 35);
        view.postMessage('ready', { value: 7 });
        view.open();
        iina.log(String(view.isOpen()));
        iina.event.on('iina.file-loaded', () => {
            view.simpleMode();
            view.setStyle('body { color: green; }');
            view.setContent('Child content');
            view.onMessage('echo', data => iina.log(data.text));
        });
        iina.event.on('iina.window-resized', () => iina.log(String(view.isOpen())));
        iina.event.on('iina.menu-update', () => view.open());
        ",
    )
    .unwrap();
    let ui = PluginUi {
        windows: super::Hub::new(true),
        ..PluginUi::default()
    };
    let global = PluginRuntimeSession::start_package_global_entry(
        package.path(),
        &manifest,
        &[],
        ui.clone(),
    )
    .unwrap();
    let player = PluginRuntimeSession::start_package_entry_with_player_context(
        package.path(),
        &manifest,
        &[],
        Default::default(),
        None,
        ui.clone(),
    )
    .unwrap();
    let entries = ui.windows.live_entries();
    let [controller, child] = entries.as_slice() else {
        panic!("expected two independent windows")
    };
    assert_ne!(controller.origin, child.origin);
    let standalone = child.snapshot(None).unwrap();
    assert!(standalone.visible);
    assert_eq!(standalone.frame.width, Some(640.0));
    assert_eq!(standalone.frame.height, None);
    assert_eq!(standalone.properties.resizable, Some(false));
    assert_eq!(standalone.document, Document::Package("page.html".into()));
    assert_eq!(
        child.take_messages(standalone.document_id),
        [("ready".into(), serde_json::json!({"value": 7}))]
    );
    assert!(matches!(controller.snapshot(None).unwrap().document,
        Document::Simple { content: Some(ref content), .. } if content.as_ref() == "Global content"
    ));
    assert_eq!(logs(&player), ["confined", "true"]);

    player
        .deliver_player_event("iina.file-loaded".into(), serde_json::Value::Null)
        .unwrap();
    player.flush_events(Duration::from_secs(2)).unwrap();
    let current = child.snapshot(Some(standalone.revision)).unwrap();
    child.receive(
        standalone.document_id,
        "echo".into(),
        serde_json::json!({"text": "Stale"}),
    );
    child.receive(
        current.document_id,
        "echo".into(),
        serde_json::json!({"text": "Child"}),
    );
    controller.receive(1, "echo".into(), serde_json::json!({"text": "Global"}));
    assert_eq!(logs(&player), ["Child"]);
    assert_eq!(logs(&global), ["Global"]);
    assert!(ui.windows.has_open_window());
    controller.user_closed();
    child.user_closed();
    assert!(!ui.windows.has_open_window());
    player
        .deliver_player_event("iina.window-resized".into(), serde_json::Value::Null)
        .unwrap();
    assert_eq!(logs(&player), ["false"]);
    player
        .deliver_player_event("iina.menu-update".into(), serde_json::Value::Null)
        .unwrap();
    player.flush_events(Duration::from_secs(2)).unwrap();
    let reopened = child.snapshot(Some(current.revision)).unwrap();
    assert!(reopened.visible && ui.windows.has_open_window());
    assert_ne!(reopened.open_revision, current.open_revision);
    assert_eq!(reopened.document_id, current.document_id);
    child.presentation_failed(current.revision);
    assert!(ui.windows.has_open_window());
    let revision = ui.windows.revision();
    child.presentation_failed(reopened.revision);
    assert_ne!(ui.windows.revision(), revision);
    assert!(!ui.windows.has_open_window());
    drop(player);
    assert_eq!(ui.windows.live_entries().len(), 1);
    assert!(!child.is_live());
    assert!(controller.is_live());
}

fn logs(session: &PluginRuntimeSession) -> Vec<String> {
    session.flush_events(Duration::from_secs(2)).unwrap();
    session
        .drain_events()
        .unwrap()
        .into_iter()
        .map(|event| match event {
            HostEvent::Ui(UiEffect::Log(text)) => text,
            event => panic!("unexpected plugin event {event:?}"),
        })
        .collect()
}
