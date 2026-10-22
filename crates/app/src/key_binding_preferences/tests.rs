use super::bindings::Bindings;
use super::*;
use super::{
    builtins::{BUILTIN_PROFILES, SYSTEM_MEDIA_ROWS, builtin_mappings},
    dispatch::{
        RuntimeAction, is_system_media_key, preferred_system_skip_interval, runtime_action,
    },
    editor::{display_row, matches_query, visible_source_indices},
    keys::normalize_profile_key,
    menus::{MENU_SHORTCUT_ACTIONS, menu_shortcut_rows},
};
use riina_core::key_bindings::{ActionKind, Mapping};

fn shortcut(mac: &str, windows: &str) -> String {
    if cfg!(target_os = "macos") {
        mac.to_owned()
    } else {
        windows.to_owned()
    }
}

#[test]
fn encoded_slint_keys_record_portable_profile_names() {
    use slint::platform::Key;
    for (key, name) in [
        (Key::LeftArrow, "LEFT"),
        (Key::RightArrow, "RIGHT"),
        (Key::UpArrow, "UP"),
        (Key::DownArrow, "DOWN"),
        (Key::Return, "ENTER"),
        (Key::Escape, "ESC"),
        (Key::Tab, "TAB"),
        (Key::Backspace, "BS"),
        (Key::Delete, "DEL"),
        (Key::Insert, "INS"),
        (Key::Home, "HOME"),
        (Key::End, "END"),
        (Key::PageUp, "PGUP"),
        (Key::PageDown, "PGDWN"),
        (Key::F1, "F1"),
        (Key::F12, "F12"),
        (Key::F24, "F24"),
    ] {
        let text = slint::SharedString::from(key);
        let recorded = recorded_key(text.as_str(), true, false, false, false).unwrap();
        assert_eq!(recorded, format!("Ctrl+{name}"));
        assert_eq!(normalize_profile_key(&recorded), recorded);
        let mapping = Mapping {
            key: recorded,
            action: "cycle pause".into(),
            kind: ActionKind::Mpv,
            comment: None,
        };
        let serialized =
            riina_core::key_bindings::serialize(std::slice::from_ref(&mapping)).unwrap();
        assert_eq!(
            riina_core::key_bindings::parse(&serialized).unwrap(),
            [mapping]
        );
    }
}

#[test]
fn encoded_modifier_keys_are_not_recorded_as_shortcuts() {
    use slint::platform::Key;
    for key in [
        Key::Control,
        Key::ControlR,
        Key::Shift,
        Key::ShiftR,
        Key::Alt,
        Key::AltGr,
        Key::Meta,
        Key::MetaR,
    ] {
        assert_eq!(
            recorded_key(
                slint::SharedString::from(key).as_str(),
                true,
                false,
                false,
                false
            ),
            None
        );
    }
}

#[test]
fn menus_do_not_install_accelerators_for_shadowed_profile_keys() {
    let mappings = [
        ("SPACE", "cycle pause"),
        ("space", "stop"),
        ("p", "cycle pause"),
    ]
    .map(|(key, action)| Mapping {
        key: key.into(),
        action: action.into(),
        kind: ActionKind::Mpv,
        comment: None,
    });
    let rows = menu_shortcut_rows(&Bindings::new(mappings.to_vec()));
    assert_eq!(rows[0].shortcut, shortcut_keys("p").unwrap());
    assert_eq!(rows[1].shortcut, shortcut_keys("SPACE").unwrap());
}

#[test]
fn captured_iina_defaults_begin_with_the_panel_and_track_actions() {
    let mappings = builtin_mappings("IINA Default");
    let visible: Vec<_> = mappings
        .iter()
        .take(7)
        .map(|mapping| (mapping.key.as_str(), mapping.action.as_str(), mapping.kind))
        .collect();
    assert_eq!(
        visible,
        [
            ("Shift+Meta+v", "video-panel", ActionKind::Riina),
            ("Shift+Meta+a", "audio-panel", ActionKind::Riina),
            ("Shift+Meta+s", "sub-panel", ActionKind::Riina),
            ("Ctrl+Meta+v", "cycle video", ActionKind::Mpv),
            ("Ctrl+Meta+s", "cycle sub", ActionKind::Mpv),
            ("Ctrl+Meta+a", "cycle audio", ActionKind::Mpv),
            ("SPACE", "cycle pause", ActionKind::Mpv),
        ]
    );
}

#[test]
fn iina_default_profile_includes_both_mute_shortcuts() {
    let mappings = builtin_mappings("IINA Default");
    let mute_keys: Vec<_> = mappings
        .iter()
        .filter(|mapping| mapping.action == "cycle mute")
        .map(|mapping| mapping.key.as_str())
        .collect();

    assert_eq!(mute_keys, ["Meta+/", "m"]);
    assert_eq!(
        normalize_profile_key("Meta+/"),
        if cfg!(target_os = "macos") {
            "Meta+/"
        } else {
            "Ctrl+/"
        }
    );
    let displayed = display_row(
        mappings
            .iter()
            .find(|mapping| mapping.key == "Meta+/")
            .unwrap(),
        false,
    );
    assert_eq!(displayed.key.as_str(), shortcut("⌘/", "Ctrl+/"));
}

#[test]
fn settings_shortcuts_show_platform_modifiers_and_preserve_raw_bindings() {
    for (key, mac, other) in [
        ("Shift+Meta+v", "⇧⌘V", "Ctrl+Shift+V"),
        ("Ctrl+Meta+v", "⌃⌘V", "Ctrl+Alt+V"),
        ("Meta+.", "⌘.", "Ctrl+."),
        ("Meta+D", "⇧⌘D", "Ctrl+Shift+D"),
    ] {
        let mapping = Mapping {
            key: key.into(),
            action: "cycle pause".into(),
            kind: ActionKind::Mpv,
            comment: None,
        };
        assert_eq!(
            display_row(&mapping, false).key.as_str(),
            shortcut(mac, other)
        );
        assert_eq!(display_row(&mapping, true).key.as_str(), key);
        assert_eq!(mapping.key, key);
    }
}

#[test]
#[cfg(not(target_os = "macos"))]
fn settings_shortcuts_use_the_menu_formatter_for_each_builtin_profile() {
    for profile in BUILTIN_PROFILES {
        for mapping in builtin_mappings(profile) {
            let row = display_row(&mapping, false);
            assert!(!row.key.is_empty(), "{profile}: {}", mapping.key);
            if let Some(keys) = shortcut_keys(&mapping.key) {
                assert_eq!(
                    row.key.as_str(),
                    keys.to_string(),
                    "{profile}: {}",
                    mapping.key
                );
            }
            assert_eq!(display_row(&mapping, true).key.as_str(), mapping.key);
        }
    }
}

#[test]
fn iina_default_profile_routes_online_subtitle_search() {
    let mappings = builtin_mappings("IINA Default");
    let mapping = mappings
        .iter()
        .find(|mapping| mapping.action == "find-online-subs")
        .unwrap();

    assert_eq!(mapping.key, "Meta+D");
    assert_eq!(mapping.kind, ActionKind::Riina);
    assert!(matches!(
        runtime_action(mapping),
        Some(RuntimeAction::FindOnlineSubs)
    ));
    assert_eq!(
        normalize_profile_key(&mapping.key),
        if cfg!(target_os = "macos") {
            "Shift+Meta+d"
        } else {
            "Ctrl+Shift+d"
        }
    );
    assert_eq!(
        display_row(mapping, false).key.as_str(),
        shortcut("⇧⌘D", "Ctrl+Shift+D")
    );
}

#[test]
fn iina_default_profile_covers_primary_source_mappings() {
    let mappings = builtin_mappings("IINA Default");
    let source_mappings = [
        ("Alt+RIGHT", "frame-step"),
        ("Alt+LEFT", "frame-back-step"),
        ("Shift+LEFT", "sub-seek -1"),
        ("Shift+RIGHT", "sub-seek 1"),
        ("Meta+L", "cycle-values loop \"inf\" \"no\""),
        ("Shift+_", "set audio-delay 0"),
        ("Z", "add sub-delay -0.5"),
        ("X", "add sub-delay 0.5"),
        ("Alt+Z", "add sub-delay -0.1"),
        ("Alt+X", "add sub-delay 0.1"),
        ("C", "set sub-delay 0"),
        ("ESC", "set fullscreen no"),
        ("ENTER", "set fullscreen yes"),
    ];

    for (key, action) in source_mappings {
        let mapping = mappings
            .iter()
            .find(|mapping| mapping.key == key && mapping.action == action)
            .unwrap_or_else(|| panic!("missing IINA default mapping {key} {action}"));
        assert!(
            runtime_action(mapping).is_some(),
            "IINA default action has no runtime route: {key} {action}"
        );
    }
}

#[test]
fn iina_alternative_bindings_route_supported_player_actions() {
    let mappings = builtin_mappings("IINA Default");
    let alternatives = [
        ("q", "quit"),
        ("p", "cycle pause"),
        ("o", "show-progress"),
        ("P", "show-progress"),
        ("Shift+PGUP", "seek 600"),
        ("Shift+PGDWN", "seek -600"),
        ("G", "add sub-scale +0.1"),
        ("F", "add sub-scale -0.1"),
        ("Alt+v", "cycle secondary-sub-visibility"),
        ("E", "cycle edition"),
    ];

    for (key, action) in alternatives {
        let mapping = mappings
            .iter()
            .find(|mapping| mapping.key == key && mapping.action == action)
            .unwrap_or_else(|| panic!("missing alternative mapping {key} {action}"));
        assert!(
            runtime_action(mapping).is_some(),
            "alternative mapping has no runtime route: {key} {action}"
        );
    }
    assert!(is_system_media_key("POWER"));
    assert!(is_system_media_key("CLOSE_WIN"));
}

#[test]
fn every_iina_default_and_system_key_has_a_runtime_route() {
    let mut mappings = builtin_mappings("IINA Default");
    mappings.extend(SYSTEM_MEDIA_ROWS.iter().map(|(key, action)| Mapping {
        key: (*key).into(),
        action: (*action).into(),
        kind: ActionKind::Mpv,
        comment: None,
    }));

    for mapping in mappings {
        assert!(
            runtime_action(&mapping).is_some(),
            "IINA default key has no runtime route: {} {}",
            mapping.key,
            mapping.action
        );
    }
}

#[test]
fn key_binding_search_matches_visible_and_raw_values_case_insensitively() {
    let mapping = &builtin_mappings("IINA Default")[0];
    assert!(matches_query(mapping, false, "video panel"));
    assert!(matches_query(mapping, true, "meta+v"));
    assert!(!matches_query(mapping, false, "subtitle"));
}

#[test]
fn filtered_rows_keep_their_source_mapping_identity() {
    let mappings = [
        ("Ctrl+k", "cycle pause", ActionKind::Mpv),
        ("Meta+s", "screenshot", ActionKind::Mpv),
        ("Ctrl+Meta+a", "cycle audio", ActionKind::Mpv),
        ("Shift+Meta+s", "sub-panel", ActionKind::Riina),
    ]
    .map(|(key, action, kind)| Mapping {
        key: key.into(),
        action: action.into(),
        kind,
        comment: None,
    });
    assert_eq!(visible_source_indices(&mappings, false, "cycle audio"), [2]);
    assert_eq!(visible_source_indices(&mappings, true, "META+S"), [1, 3]);
    assert_eq!(
        visible_source_indices(&mappings, false, "missing"),
        Vec::<usize>::new()
    );
}

#[test]
fn recorded_shortcuts_use_mpv_names_and_preserve_modifier_order() {
    assert_eq!(
        recorded_key(" ", false, false, false, false).as_deref(),
        Some("SPACE")
    );
    assert_eq!(
        recorded_key("ArrowLeft", true, false, true, false).as_deref(),
        Some("Ctrl+Shift+LEFT")
    );
    assert_eq!(
        recorded_key("V", false, false, true, true).as_deref(),
        Some("Shift+Meta+v")
    );
    assert_eq!(recorded_key("Control", true, false, false, false), None);
}

#[test]
fn recorded_profile_keys_match_platform_primary_shortcuts() {
    let video_panel = normalize_profile_key("Shift+Meta+v");
    if cfg!(target_os = "macos") {
        assert_eq!(video_panel, "Shift+Meta+v");
    } else {
        assert_eq!(video_panel, "Ctrl+Shift+v");
        assert_eq!(normalize_profile_key("Ctrl+Meta+v"), "Ctrl+Alt+v");
    }
    assert_eq!(normalize_profile_key("R"), "Shift+r");
    assert_eq!(normalize_profile_key("SPACE"), "SPACE");
    assert_eq!(normalize_profile_key("RIGHT"), "RIGHT");
}

#[test]
fn menu_shortcuts_follow_the_active_profile_for_both_player_surfaces() {
    let mappings = builtin_mappings("IINA Default");
    let rows = menu_shortcut_rows(&Bindings::new(mappings.to_vec()));
    assert_eq!(rows.len(), 56);
    assert_eq!(
        rows[0].shortcut.to_string(),
        slint::Keys::from_parts(["Space"]).unwrap().to_string()
    );
    let video_panel_key = if cfg!(target_os = "macos") {
        slint::Keys::from_parts(["Shift", "Meta", "v"]).unwrap()
    } else {
        slint::Keys::from_parts(["Control", "Shift", "v"]).unwrap()
    };
    assert_eq!(rows[18].shortcut, video_panel_key);
    let cycle_video_key = if cfg!(target_os = "macos") {
        slint::Keys::from_parts(["Control", "Meta", "v"]).unwrap()
    } else {
        slint::Keys::from_parts(["Control", "Alt", "v"]).unwrap()
    };
    assert_eq!(rows[19].shortcut, cycle_video_key);
    let ab_loop_key = if cfg!(target_os = "macos") {
        slint::Keys::from_parts(["Meta", "l"]).unwrap()
    } else {
        slint::Keys::from_parts(["Control", "l"]).unwrap()
    };
    assert_eq!(rows[46].shortcut, ab_loop_key);
    let mpv_rows = menu_shortcut_rows(&Bindings::new(builtin_mappings("mpv Default")));
    assert_eq!(
        mpv_rows[46].shortcut,
        slint::Keys::from_parts(["l"]).unwrap()
    );
    let custom = Mapping {
        key: "Ctrl+k".into(),
        action: "cycle pause".into(),
        kind: ActionKind::Mpv,
        comment: None,
    };
    let custom_rows = menu_shortcut_rows(&Bindings::new(vec![custom]));
    assert_eq!(
        custom_rows[0].shortcut,
        slint::Keys::from_parts(["Control", "k"]).unwrap()
    );
    assert!(custom_rows[18].shortcut.to_string().is_empty());
    assert!(custom_rows[46].shortcut.to_string().is_empty());
    let plugin_shortcut = shortcut_keys("Cmd+t").unwrap();
    let plugin_expected = if cfg!(target_os = "macos") {
        slint::Keys::from_parts(["Meta", "t"]).unwrap()
    } else {
        slint::Keys::from_parts(["Control", "t"]).unwrap()
    };
    assert_eq!(plugin_shortcut, plugin_expected);
}

#[test]
fn default_menu_hints_cover_bound_actions_and_prefer_primary_aliases() {
    let rows = menu_shortcut_rows(&Bindings::new(builtin_mappings("IINA Default")));
    for (index, (_, action)) in MENU_SHORTCUT_ACTIONS.iter().enumerate() {
        if index != 45 {
            assert!(
                !rows[index].shortcut.to_string().is_empty(),
                "Missing hint: {action}"
            );
        }
    }
    let primary = if cfg!(target_os = "macos") {
        "Meta"
    } else {
        "Control"
    };
    let secondary = if cfg!(target_os = "macos") {
        "Control"
    } else {
        "Alt"
    };
    assert_eq!(
        rows[27].shortcut,
        slint::Keys::from_parts([primary, secondary, "f"]).unwrap()
    );
    assert_eq!(
        rows[37].shortcut,
        slint::Keys::from_parts([primary, "Slash"]).unwrap()
    );
    assert_eq!(
        rows[47].shortcut,
        slint::Keys::from_parts([primary, "Shift", "l"]).unwrap()
    );
}

#[test]
fn shifted_punctuation_and_literal_plus_survive_profile_and_menu_conversion() {
    for punctuation in [
        "<", ">", "(", ")", "_", "+", "[", "]", "-", "=", "/", ",", ".", "`", "\\",
    ] {
        let key = format!("Ctrl+Shift+{punctuation}");
        assert_eq!(normalize_profile_key(&key), key);
        assert_eq!(
            shortcut_keys(&key).unwrap(),
            slint::Keys::from_parts(["Control", "Shift", punctuation]).unwrap(),
            "{key}",
        );
    }
    for key in ["+", "Ctrl++", "Ctrl+Alt++", "Ctrl+Shift++"] {
        assert_eq!(normalize_profile_key(key), key);
        assert!(shortcut_keys(key).is_some(), "{key}");
    }
    assert_eq!(
        recorded_key("+", true, false, true, false).as_deref(),
        Some("Ctrl+Shift++")
    );
    assert!(shortcut_keys("Ctrl+").is_none());
}

#[test]
fn runtime_dispatch_accepts_supported_actions_and_rejects_unbounded_commands() {
    let mapping = |action: &str, kind| Mapping {
        key: "SPACE".into(),
        action: action.into(),
        kind,
        comment: None,
    };
    assert!(matches!(
        runtime_action(&mapping("cycle pause", ActionKind::Mpv)),
        Some(RuntimeAction::TogglePause)
    ));
    assert!(matches!(
        runtime_action(&mapping("video-panel", ActionKind::Riina)),
        Some(RuntimeAction::SettingsPane(6))
    ));
    assert!(matches!(
        runtime_action(&mapping("fit-to-screen", ActionKind::Riina)),
        Some(RuntimeAction::FitToScreen)
    ));
    assert!(matches!(
        runtime_action(&mapping("find-online-subs", ActionKind::Riina)),
        Some(RuntimeAction::FindOnlineSubs)
    ));
    assert!(matches!(
        runtime_action(&mapping("bigger-window", ActionKind::Riina)),
        Some(RuntimeAction::ResizeWindow(4))
    ));
    assert!(matches!(
        runtime_action(&mapping("set window-scale 0.5", ActionKind::Mpv)),
        Some(RuntimeAction::ResizeWindow(0))
    ));
    assert!(runtime_action(&mapping("run calc.exe", ActionKind::Mpv)).is_none());
    assert!(runtime_action(&mapping("seek NaN", ActionKind::Mpv)).is_none());
}

#[test]
fn system_skip_intervals_follow_only_supported_seek_bindings() {
    let mapping = |action: &str, kind| Mapping {
        key: "FORWARD".into(),
        action: action.into(),
        kind,
        comment: None,
    };
    assert_eq!(preferred_system_skip_interval(None), Some(15.0));
    assert_eq!(
        preferred_system_skip_interval(Some(&mapping("seek -7 exact", ActionKind::Mpv))),
        Some(7.0)
    );
    assert_eq!(
        preferred_system_skip_interval(Some(&mapping("seek 7 absolute", ActionKind::Mpv))),
        None
    );
    assert_eq!(
        preferred_system_skip_interval(Some(&mapping("next", ActionKind::Riina))),
        None
    );
    assert_eq!(
        preferred_system_skip_interval(Some(&mapping("seek NaN", ActionKind::Mpv))),
        None
    );
}

#[test]
fn installed_profiles_dispatch_encoded_events_without_ui_fallbacks() {
    use slint::ComponentHandle;
    i_slint_backend_testing::init_no_event_loop();
    let app = crate::PlayerWindow::new().unwrap();
    let position = std::rc::Rc::new(std::cell::Cell::new(0.0_f32));
    let observed = position.clone();
    app.on_seek_relative(move |amount| observed.set(amount));
    let weak = app.as_weak();
    app.on_toggle_fullscreen(move || {
        if let Some(app) = weak.upgrade() {
            app.set_fullscreen(!app.get_fullscreen());
        }
    });
    install_player(
        &app,
        &riina_core::key_bindings::Settings::default(),
        std::sync::Arc::new(std::sync::Mutex::new(riina_core::Playback::default())),
    );
    for (key, expected) in [
        (slint::platform::Key::LeftArrow, -5.0),
        (slint::platform::Key::RightArrow, 5.0),
    ] {
        assert!(app.invoke_dispatch_key_binding(key.into(), false, false, false, false));
        assert_eq!(position.get(), expected);
    }
    assert!(app.invoke_dispatch_key_binding(
        slint::platform::Key::Return.into(),
        false,
        false,
        false,
        false
    ));
    assert!(app.get_fullscreen());
    assert!(app.invoke_dispatch_key_binding(
        slint::platform::Key::Escape.into(),
        false,
        false,
        false,
        false
    ));
    assert!(!app.get_fullscreen());
}

#[test]
fn duplicate_keys_keep_their_first_menu_position_and_last_action() {
    let mappings = [
        ("SPACE", "stop"),
        ("p", "cycle pause"),
        ("space", "cycle pause"),
    ]
    .map(|(key, action)| Mapping {
        key: key.into(),
        action: action.into(),
        kind: ActionKind::Mpv,
        comment: None,
    });
    let bindings = Bindings::new(mappings.to_vec());
    assert_eq!(bindings.get("SPACE").unwrap().action, "cycle pause");
    let rows = menu_shortcut_rows(&bindings);
    assert_eq!(rows[0].shortcut, shortcut_keys("SPACE").unwrap());
    assert_eq!(rows[1].shortcut, slint::Keys::default());
}

#[test]
fn malformed_modifier_sequences_do_not_override_unmodified_keys() {
    for malformed in ["Unknown+a", "Ctrl++a", "+a", "Ctrl+", "ThingMeta+a"] {
        assert_eq!(normalize_profile_key(malformed), "", "{malformed}");
        assert!(shortcut_keys(malformed).is_none(), "{malformed}");
        let mappings = [("a", "cycle pause"), (malformed, "stop")].map(|(key, action)| Mapping {
            key: key.into(),
            action: action.into(),
            kind: ActionKind::Mpv,
            comment: None,
        });
        let bindings = Bindings::new(mappings.to_vec());
        assert_eq!(bindings.get("a").unwrap().action, "cycle pause");
    }
}
