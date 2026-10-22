use super::*;
use std::time::Instant;

#[test]
fn enabled_plugin_packages_follow_the_persisted_iina_order() {
    let root = std::env::temp_dir().join(format!(
        "riina-plugin-order-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let plugins = root.join("Plugins");
    for (identifier, name) in [("z.last", "Alpha"), ("a.first", "Zulu")] {
        let directory = plugins.join(format!("{identifier}.iinaplugin"));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("Info.json"),
            format!(
                r#"{{"name":"{name}","identifier":"{identifier}","version":"1.0.0","author":{{"name":"Test"}},"entry":"main.js","permissions":[]}}"#
            ),
        )
        .unwrap();
        std::fs::write(directory.join("main.js"), "").unwrap();
    }
    let enabled_path = root.join("plugin-enabled.json");
    std::fs::write(&enabled_path, r#"["a.first","z.last"]"#).unwrap();
    std::fs::write(root.join("plugin-order.json"), r#"["z.last","a.first"]"#).unwrap();

    let plugins = enabled_packages(&plugins, &enabled_path);
    assert_eq!(
        plugins
            .iter()
            .map(|plugin| plugin.manifest.identifier.as_str())
            .collect::<Vec<_>>(),
        ["z.last", "a.first"]
    );

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn plugins_enabled_after_window_and_mpv_initialization_receive_events_once() {
    let root = std::env::temp_dir().join(format!(
        "riina-late-plugin-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let directory = root.join("Plugins/org.example.late-load.iinaplugin");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("Info.json"),
        r#"{"name":"Late Load","identifier":"org.example.late-load","version":"1.0.0","author":{"name":"Test"},"entry":"main.js","permissions":[],"sidebarTab":{"name":"Late Sidebar"}}"#,
    )
    .unwrap();
    std::fs::write(
        directory.join("main.js"),
        "iina.event.on('iina.window-loaded', () => iina.log('late-window-loaded')); iina.event.on('iina.mpv-initialized', () => iina.log('late-mpv-initialized')); iina.event.on('mpv.start-file', () => iina.log('mpv-start-file')); iina.event.on('iina.plugin-overlay-loaded', () => iina.log('plugin-overlay-loaded')); iina.event.on('iina.thumbnails-ready', () => iina.log('thumbnails-ready')); iina.event.on('iina.window-will-close', () => iina.log('window-will-close'));",
    )
    .unwrap();

    let preferences_path = root.join("preferences.json");
    let host = PluginHost::start(&preferences_path, &[], &[]).unwrap();
    let handle = host.handle();
    handle
        .update_player_file_context(
            PluginPlayerFileContext::default()
                .with_core_version(serde_json::json!({"mpv": "mpv 0.41.0-git"})),
        )
        .unwrap();
    handle.notify_window_loaded().unwrap();
    handle.notify_window_loaded().unwrap();
    handle
        .enable(
            Manifest::load_from_directory(&directory).unwrap(),
            directory.clone(),
        )
        .unwrap();
    handle
        .update_player_file_context(
            PluginPlayerFileContext::default()
                .with_core_version(serde_json::json!({"mpv": "mpv 0.41.0-git"}))
                .with_mpv_events(vec![riina_core::MpvEventRecord {
                    sequence: 1,
                    name: "start-file".into(),
                }]),
        )
        .unwrap();
    handle.notify_plugin_overlay_loaded().unwrap();
    handle.notify_thumbnails_ready().unwrap();

    let deadline = Instant::now() + Duration::from_secs(3);
    let mut messages = Vec::new();
    let received = |messages: &[PluginHostMessage], needle: &str| {
        messages
            .iter()
            .any(|message| matches!(message, PluginHostMessage::Log(text) if text.contains(needle)))
    };
    while Instant::now() < deadline
        && (!received(&messages, "late-window-loaded")
            || !received(&messages, "late-mpv-initialized")
            || !received(&messages, "mpv-start-file"))
    {
        messages.extend(handle.drain_messages().unwrap());
        std::thread::sleep(Duration::from_millis(10));
    }
    handle.notify_window_will_close().unwrap();
    drop(host);
    messages.extend(handle.drain_messages().unwrap());
    assert!(received(&messages, "late-window-loaded"));
    assert!(received(&messages, "late-mpv-initialized"));
    assert!(received(&messages, "mpv-start-file"));
    assert!(received(&messages, "plugin-overlay-loaded"));
    assert!(received(&messages, "thumbnails-ready"));
    assert!(messages.iter().any(|message| {
        matches!(message, PluginHostMessage::Log(text) if text.contains("window-will-close"))
    }));
    assert_eq!(
        messages
            .iter()
            .filter(|message| matches!(message, PluginHostMessage::Log(text) if text.contains("late-window-loaded")))
            .count(),
        1
    );
    assert_eq!(
        messages
            .iter()
            .filter(|message| matches!(message, PluginHostMessage::Log(text) if text.contains("late-mpv-initialized")))
            .count(),
        1
    );
    assert_eq!(
        messages
            .iter()
            .filter(|message| matches!(message, PluginHostMessage::Log(text) if text.contains("mpv-start-file")))
            .count(),
        1
    );
    assert_eq!(
        messages
            .iter()
            .filter(|message| matches!(message, PluginHostMessage::SidebarTabAdded { plugin_identifier, .. } if plugin_identifier == "org.example.late-load"))
            .count(),
        1
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn reload_all_restarts_only_enabled_plugin_entries() {
    let root = std::env::temp_dir().join(format!(
        "riina-plugin-reload-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let directory = root.join("Plugins/org.example.reload.iinaplugin");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("Info.json"),
        r#"{"name":"Reload Test","identifier":"org.example.reload","version":"1.0.0","author":{"name":"Test"},"entry":"main.js","permissions":[]}"#,
    )
    .unwrap();
    std::fs::write(directory.join("main.js"), "iina.log('entry-started');").unwrap();
    let disabled_directory = root.join("Plugins/org.example.disabled.iinaplugin");
    std::fs::create_dir_all(&disabled_directory).unwrap();
    std::fs::write(
        disabled_directory.join("Info.json"),
        r#"{"name":"Disabled Test","identifier":"org.example.disabled","version":"1.0.0","author":{"name":"Test"},"entry":"main.js","permissions":[]}"#,
    )
    .unwrap();
    std::fs::write(
        disabled_directory.join("main.js"),
        "iina.log('disabled-entry-started');",
    )
    .unwrap();
    std::fs::write(
        root.join("plugin-enabled.json"),
        r#"["org.example.reload"]"#,
    )
    .unwrap();

    let preferences_path = root.join("preferences.json");
    let host = PluginHost::start(&preferences_path, &[], &[]).unwrap();
    let handle = host.handle();
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut messages = Vec::new();
    while Instant::now() < deadline
        && messages
            .iter()
            .filter(|message| {
                matches!(message, PluginHostMessage::Log(text) if text.contains("entry-started"))
            })
            .count()
            < 1
    {
        messages.extend(handle.drain_messages().unwrap());
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(messages.iter().any(
        |message| matches!(message, PluginHostMessage::Log(text) if text.contains("entry-started"))
    ));

    handle.reload_all().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline
        && (messages
            .iter()
            .filter(|message| {
                matches!(message, PluginHostMessage::Log(text) if text.contains("entry-started"))
            })
            .count()
            < 2
            || !messages.iter().any(|message| {
                matches!(message, PluginHostMessage::Status(text) if text.contains("Reloaded 1 enabled plugin"))
            }))
    {
        messages.extend(handle.drain_messages().unwrap());
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(host);
    messages.extend(handle.drain_messages().unwrap());
    assert_eq!(
        messages
            .iter()
            .filter(|message| {
                matches!(message, PluginHostMessage::Log(text) if text.contains("entry-started"))
            })
            .count(),
        2
    );
    assert!(messages.iter().any(|message| {
        matches!(message, PluginHostMessage::Status(text) if text.contains("Reloaded 1 enabled plugin"))
    }));
    assert!(!messages.iter().any(|message| {
        matches!(message, PluginHostMessage::Log(text) if text.contains("disabled-entry-started"))
    }));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn plugin_menu_actions_from_both_runtime_scopes_reach_their_owner() {
    let root = std::env::temp_dir().join(format!(
        "riina-plugin-menu-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let directory = root.join("Plugins/org.example.menu.iinaplugin");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("Info.json"),
        r#"{"name":"Menu Test","identifier":"org.example.menu","version":"1.0.0","author":{"name":"Test"},"entry":"main.js","globalEntry":"global.js","permissions":[]}"#,
    )
    .unwrap();
    std::fs::write(
        directory.join("main.js"),
        "iina.event.on('iina.menu-update', () => iina.log('plugin-menu-opened')); iina.menu.addItem(iina.menu.item('Run action', () => iina.log('menu-action-ran')));",
    )
    .unwrap();
    std::fs::write(
        directory.join("global.js"),
        "iina.menu.addItem(iina.menu.item('Global action', () => iina.log('global-menu-action-ran')));",
    )
    .unwrap();
    std::fs::write(root.join("plugin-enabled.json"), r#"["org.example.menu"]"#).unwrap();

    let preferences_path = root.join("preferences.json");
    let host = PluginHost::start(&preferences_path, &[], &[]).unwrap();
    let handle = host.handle();
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut messages = Vec::new();
    while Instant::now() < deadline
        && !(messages.iter().any(|message| {
            matches!(message,
            PluginHostMessage::PluginMenuUpdated { plugin_identifier, global: false, items, .. }
                if plugin_identifier == "org.example.menu"
                    && items.iter().any(|item| item.title == "Run action"))
        }) && messages.iter().any(|message| {
            matches!(message,
                PluginHostMessage::PluginMenuUpdated { plugin_identifier, global: true, items, .. }
                    if plugin_identifier == "org.example.menu"
                        && items.iter().any(|item| item.title == "Global action"))
        }))
    {
        messages.extend(handle.drain_messages().unwrap());
        std::thread::sleep(Duration::from_millis(10));
    }
    let player_menu_id = messages.iter().find_map(|message| match message {
        PluginHostMessage::PluginMenuUpdated {
            plugin_identifier,
            global: false,
            items,
            ..
        } if plugin_identifier == "org.example.menu" => items
            .iter()
            .find(|item| item.title == "Run action")
            .map(|item| item.id.clone()),
        _ => None,
    });
    let global_menu_id = messages.iter().find_map(|message| match message {
        PluginHostMessage::PluginMenuUpdated {
            plugin_identifier,
            global: true,
            items,
            ..
        } if plugin_identifier == "org.example.menu" => items
            .iter()
            .find(|item| item.title == "Global action")
            .map(|item| item.id.clone()),
        _ => None,
    });
    let (Some(player_menu_id), Some(global_menu_id)) = (player_menu_id, global_menu_id) else {
        panic!(
            "Both player and global plugin menu snapshots should be available; messages: {messages:#?}"
        );
    };
    handle
        .activate_plugin_menu_item("org.example.menu".into(), false, player_menu_id)
        .unwrap();
    handle
        .activate_plugin_menu_item("org.example.menu".into(), true, global_menu_id)
        .unwrap();
    handle.notify_menu_update().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline
        && !(messages.iter().any(|message| {
            matches!(message,
            PluginHostMessage::Log(text) if text.contains("menu-action-ran"))
        }) && messages.iter().any(|message| {
            matches!(message,
                PluginHostMessage::Log(text) if text.contains("plugin-menu-opened"))
        }) && messages.iter().any(|message| {
            matches!(message,
                PluginHostMessage::Log(text) if text.contains("global-menu-action-ran"))
        }))
    {
        messages.extend(handle.drain_messages().unwrap());
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(host);
    messages.extend(handle.drain_messages().unwrap());
    assert!(messages.iter().any(|message| matches!(message,
        PluginHostMessage::Log(text) if text.contains("menu-action-ran")
    )));
    assert!(messages.iter().any(|message| matches!(message,
        PluginHostMessage::Log(text) if text.contains("plugin-menu-opened")
    )));
    assert!(messages.iter().any(|message| matches!(message,
        PluginHostMessage::Log(text) if text.contains("global-menu-action-ran")
    )));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn core_ui_visibility_messages_reach_the_player_ui() {
    assert_eq!(
        host_message("plugin", HostEvent::SetUiVisibility(true)),
        PluginHostMessage::SetUiVisibility(true)
    );
}
