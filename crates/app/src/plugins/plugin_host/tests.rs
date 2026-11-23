use super::*;
use riina_test_support::TempDir;
use std::time::Instant;

#[test]
fn playback_updates_keep_the_plugin_window_context() {
    let scratch = TempDir::new("plugin-window-context").unwrap();
    let directory = scratch
        .path()
        .join("Plugins/org.example.window-context.iinaplugin");
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        directory.join("Info.json"),
        r#"{"name":"Window Context","identifier":"org.example.window-context","version":"1.0.0","author":{"name":"Test"},"entry":"main.js","permissions":[]}"#,
    )
    .unwrap();
    fs::write(
        directory.join("main.js"),
        r"const report = label => iina.log(label + ': ' + iina.core.window.loaded + ','
            + iina.core.window.frame?.width);
        report('activation');
        iina.event.on('iina.window-will-close', () => report('closing'));",
    )
    .unwrap();
    let host = PluginHost::start(&scratch.path().join("preferences.json"), &[], &[]).unwrap();
    let handle = host.handle();
    handle
        .update_window_properties(BTreeMap::from([
            ("loaded".into(), serde_json::json!(true)),
            (
                "frame".into(),
                serde_json::json!({"width": 1040, "height": 585}),
            ),
        ]))
        .unwrap();
    handle
        .update_player_file_context(PluginPlayerFileContext::default().with_media_generation(1))
        .unwrap();
    handle
        .enable(
            Manifest::load_from_directory(&directory).unwrap(),
            directory,
        )
        .unwrap();
    handle
        .update_player_file_context(PluginPlayerFileContext::default().with_media_generation(2))
        .unwrap();
    handle.notify_window_will_close().unwrap();
    drop(host);
    let messages = handle.drain_messages().unwrap();
    for expected in ["activation: true,1040", "closing: true,1040"] {
        assert!(
            messages.iter().any(|message| {
                matches!(message, PluginHostMessage::Log(text) if text.ends_with(expected))
            }),
            "Missing {expected:?} in {messages:?}"
        );
    }
}

#[test]
fn enabled_plugin_entries_run_in_persisted_order_before_initial_media() {
    let directory = TempDir::new("plugin-order").unwrap();
    let root = directory.path();
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
        std::fs::write(
            directory.join("main.js"),
            format!("iina.log('{identifier}');"),
        )
        .unwrap();
    }
    let enabled_path = root.join("plugin-enabled.json");
    std::fs::write(&enabled_path, r#"["a.first","z.last"]"#).unwrap();
    std::fs::write(root.join("plugin-order.json"), r#"["z.last","a.first"]"#).unwrap();

    let mut host = PluginHost::prepare(&root.join("preferences.json"), &[], &[]).unwrap();
    let handle = host.handle();
    let (send, receive) = mpsc::sync_channel(1);
    host.handle().after_initial_plugins(move || {
        _ = send.send(handle.drain_messages());
    });
    host.start_player(None);
    let messages = receive
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap();
    let activated = messages
        .iter()
        .filter_map(|message| match message {
            PluginHostMessage::Log(text) => {
                text.rsplit_once(": ").map(|(_, identifier)| identifier)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(activated, ["z.last", "a.first"]);
}

#[test]
fn plugins_enabled_after_window_and_mpv_initialization_receive_events_once() {
    let directory = TempDir::new("late-plugin").unwrap();
    let root = directory.path();
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
    let idle_deadline = Instant::now() + Duration::from_secs(2);
    while handle.has_active_sessions() {
        assert!(
            Instant::now() < idle_deadline,
            "Empty host did not become idle"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let (wake, wakes) = mpsc::channel();
    handle.set_ui_waker(move || {
        // The test can finish waiting before the last callback is delivered.
        let _ = wake.send(());
    });
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
    wakes.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(handle.has_active_sessions());
    handle
        .update_player_file_context(
            PluginPlayerFileContext::default()
                .with_core_version(serde_json::json!({"mpv": "mpv 0.41.0-git"}))
                .with_mpv_events(
                    std::collections::VecDeque::from([riina_core::MpvEventRecord {
                        sequence: 1,
                        name: "start-file".into(),
                    }])
                    .into(),
                ),
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
            .filter(|message| {
                matches!(
                    message,
                    PluginHostMessage::SidebarTabAdded { plugin_identifier, .. }
                        if plugin_identifier == "org.example.late-load"
                )
            })
            .count(),
        1
    );
}

#[test]
fn reload_all_restarts_only_enabled_plugin_entries() {
    let directory = TempDir::new("plugin-reload").unwrap();
    let root = directory.path();
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
}

#[test]
fn plugin_menu_actions_from_both_runtime_scopes_reach_their_owner() {
    let directory = TempDir::new("plugin-menu").unwrap();
    let root = directory.path();
    let directory = root.join("Plugins/org.example.menu.iinaplugin");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("Info.json"),
        r#"{"name":"Menu Test","identifier":"org.example.menu","version":"1.0.0","author":{"name":"Test"},"entry":"main.js","globalEntry":"global.js","permissions":[]}"#,
    )
    .unwrap();
    std::fs::write(
        directory.join("main.js"),
        r"
        iina.event.on('iina.menu-update', () => iina.log('plugin-menu-opened'));
        iina.event.on('iina.window-will-close', () => {});
        if (iina.mpv.getFlag('missing') !== false || iina.mpv.getNumber('missing') !== 0 ||
            iina.mpv.getString('missing') !== null || iina.mpv.getNative('missing') !== null) {
            throw Error('unexpected mpv getter fallback');
        }
        iina.menu.addItem(iina.menu.item('Run action', () => iina.log('menu-action-ran')));
        iina.playlist.registerMenuItemBuilder(rows => {
            const submenu = iina.menu.item('Tools', null);
            submenu.addSubMenuItem(iina.menu.item(rows.join(','), () => {
                iina.log('playlist-action-ran:' + rows.join(','));
            }, {selected: true}));
            const blocked = iina.menu.item('Blocked', () => iina.log('blocked-ran'), {
                enabled: false
            });
            return [submenu, iina.menu.separator(), blocked];
        });
        ",
    )
    .unwrap();
    std::fs::write(
        directory.join("global.js"),
        r"
        if (iina.playlist !== undefined || iina.mpv !== undefined) {
            throw Error('unexpected global player API');
        }
        iina.menu.addItem(iina.menu.item('Global action', () => iina.log('global-menu-action-ran')));
        ",
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
    verify_playlist_menu_actions(&handle);
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
    let playlist_actions = messages
        .iter()
        .filter_map(|message| match message {
            PluginHostMessage::Log(text)
                if text.contains("playlist-action-ran:") || text.ends_with(": blocked-ran") =>
            {
                Some(text.as_str())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        playlist_actions,
        ["org.example.menu: playlist-action-ran:0,2"]
    );
}

fn verify_playlist_menu_actions(handle: &PluginHostHandle) {
    let build = |request, indices| {
        let (ready, result) = mpsc::sync_channel(1);
        handle
            .build_playlist_menu(request, indices, move |items| {
                let _ = ready.send(items);
            })
            .unwrap();
        let mut contributions = result.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(contributions.len(), 1);
        let menu = contributions.pop().unwrap();
        assert_eq!(menu.identifier, "org.example.menu");
        menu.items
    };
    let blank = build(10, Vec::new());
    assert!(blank[0].items[0].title.is_empty());
    let items = build(11, vec![0, 2]);
    assert!(items[1].separator);
    assert!(!items[2].enabled);
    let action = &items[0].items[0];
    assert!(action.selected);
    assert_eq!(action.title, "0,2");
    for (request, id) in [(10, &action.id), (11, &action.id), (11, &action.id)] {
        handle
            .activate_playlist_menu_item(request, "org.example.menu".into(), id.clone())
            .unwrap();
    }
    handle.notify_window_will_close().unwrap();
}

#[test]
fn core_ui_visibility_messages_reach_the_player_ui() {
    assert_eq!(
        host_message("plugin", UiEffect::SetUiVisibility(true)),
        PluginHostMessage::SetUiVisibility(true)
    );
}
