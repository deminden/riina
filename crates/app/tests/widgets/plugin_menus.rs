use super::{key, player};
use i_slint_backend_testing::ElementHandle;
use riina_ui::{
    DeveloperConsoleMember, DeveloperConsoleRow, MenuShortcutRow, PlayerMenuActions,
    PlayerPluginSidebarState, PluginDeveloperTool, PluginMenuRow,
};
use slint::{
    ComponentHandle, LogicalPosition, LogicalSize, ModelRc, VecModel,
    platform::{Key, PointerEventButton, WindowEvent},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[test]
fn focused_plugin_sidebar_keeps_space_from_the_playback_menu()
-> Result<(), Box<dyn std::error::Error>> {
    let app = player(7);
    app.set_native_video(false);
    let sidebar = app.global::<PlayerPluginSidebarState>();
    sidebar.set_composited(true);
    app.set_menu_shortcuts(ModelRc::new(VecModel::from(vec![MenuShortcutRow {
        shortcut: slint::Keys::from_parts(["Space"])?,
    }])));
    let pause = Rc::new(Cell::new(0));
    let recorded = pause.clone();
    app.global::<PlayerMenuActions>()
        .on_toggle_pause(move || recorded.set(recorded.get() + 1));
    let browser_keys = Rc::new(RefCell::new(Vec::new()));
    let recorded = browser_keys.clone();
    sidebar.on_key_input(move |text, _, _, _, _, released| {
        if !released {
            recorded.borrow_mut().push(text);
        }
    });

    slint::platform::update_timers_and_animations();
    let position = LogicalPosition::new(app.get_plugin_sidebar_view_x() + 40.0, 190.0);
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
    slint::platform::update_timers_and_animations();
    assert!(sidebar.get_keyboard_focused());
    key(&app, Key::Space);
    assert_eq!(pause.get(), 0);
    assert_eq!(
        browser_keys.borrow().as_slice(),
        &[slint::SharedString::from(Key::Space)]
    );

    app.invoke_focus_player();
    slint::platform::update_timers_and_animations();
    assert!(!sidebar.get_keyboard_focused());
    key(&app, Key::Space);
    assert_eq!(pause.get(), 1);
    app.hide()?;
    Ok(())
}

#[test]
fn nested_plugin_shortcuts_follow_replaced_menu_rows() -> Result<(), Box<dyn std::error::Error>> {
    let app = player(0);
    app.set_native_video(false);
    let calls = Rc::new(RefCell::new(Vec::new()));
    let observed = calls.clone();
    app.global::<PlayerMenuActions>()
        .on_activate_plugin_menu_item(move |token| observed.borrow_mut().push(token.to_string()));

    let rows = |token: &'static str| -> Result<_, slint::KeysParseError> {
        let mut rows = vec![PluginMenuRow {
            token: "plugin-heading".into(),
            label: "Plugin title".into(),
            section_header: true,
            ..Default::default()
        }];
        for label in ["First", "Second", "Third", "Fourth", "Fifth"] {
            rows.push(PluginMenuRow {
                token: label.into(),
                label: label.into(),
                enabled: true,
                ..Default::default()
            });
        }
        for depth in 0..7 {
            rows.push(PluginMenuRow {
                token: "submenu".into(),
                label: format!("Tools {}", depth + 1).into(),
                enabled: true,
                depth,
                submenu: true,
                shortcut: if depth == 0 {
                    slint::Keys::from_parts(["F9"])?
                } else {
                    Default::default()
                },
                ..Default::default()
            });
        }
        rows.push(PluginMenuRow {
            token: token.into(),
            label: "Run action".into(),
            enabled: true,
            depth: 7,
            shortcut: slint::Keys::from_parts(["F8"])?,
            ..Default::default()
        });
        Ok(ModelRc::new(VecModel::from(rows)))
    };

    app.set_plugin_menu_rows(rows("global-action")?);
    key(&app, Key::F9);
    key(&app, Key::F8);
    app.set_plugin_menu_rows(rows("player-action")?);
    key(&app, Key::F8);
    app.set_plugin_menu_rows(ModelRc::default());
    key(&app, Key::F8);
    assert_eq!(*calls.borrow(), ["global-action", "player-action"]);
    app.hide()?;
    Ok(())
}

#[test]
fn developer_tool_keeps_results_compact_and_recallable() -> Result<(), Box<dyn std::error::Error>> {
    i_slint_backend_testing::init_no_event_loop();
    let window = PluginDeveloperTool::new()?;
    window.window().set_size(LogicalSize::new(500.0, 400.0));
    let members = ModelRc::new(VecModel::from(vec![DeveloperConsoleMember {
        key: "total".into(),
        value: "11".into(),
    }]));
    let rows = Rc::new(VecModel::from(vec![
        DeveloperConsoleRow {
            label: "â†’".into(),
            text: "Object".into(),
            members: members.clone(),
            ..Default::default()
        },
        DeveloperConsoleRow {
            label: "â†’".into(),
            text: "Second result".into(),
            members,
            ..Default::default()
        },
    ]));
    window.set_lines(ModelRc::from(Rc::clone(&rows)));
    window.on_recall_command(|offset| match offset {
        0 => "({total: 11})".into(),
        1 => "total = 10".into(),
        _ => "".into(),
    });
    window.show()?;
    slint::platform::update_timers_and_animations();
    let find = |label| ElementHandle::find_by_accessible_label(&window, label).next();
    let run = find("Run").ok_or("Missing Run button")?;
    assert!(
        run.size().height <= 36.0,
        "Run button size: {:?}",
        run.size()
    );
    assert!(
        run.absolute_position().y >= 240.0,
        "Run button position: {:?}",
        run.absolute_position()
    );
    let object = find("Object").ok_or("Missing expandable result")?;
    let second = find("Second result").ok_or("Missing second expandable result")?;
    let y = second.absolute_position().y;
    let gap = y - object.absolute_position().y;
    assert!((21.0..=32.0).contains(&gap), "Result row spacing: {gap}");
    assert_eq!(object.accessible_expanded(), Some(false));
    object.invoke_accessible_default_action();
    assert_eq!(object.accessible_expanded(), Some(true));
    let second = find("Second result").ok_or("Second result disappeared")?;
    assert!(
        second.absolute_position().y > y,
        "Expanded rows did not grow: {y}"
    );
    rows.push(DeveloperConsoleRow {
        text: "More output".into(),
        ..Default::default()
    });
    assert_eq!(object.accessible_expanded(), Some(true));
    window.invoke_focus_editor();
    let editor = find("JavaScript").ok_or("Missing editor")?;
    for expected in ["({total: 11})", "total = 10"] {
        let text = Key::UpArrow.into();
        window
            .window()
            .dispatch_event(WindowEvent::KeyPressed { text });
        assert_eq!(editor.accessible_value().as_deref(), Some(expected));
    }
    window.hide()?;
    Ok(())
}
