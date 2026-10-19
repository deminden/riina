//! Real player widgets on Slint's headless backend; native focus is checked separately.
use i_slint_backend_testing::ElementHandle;
use slint::{
    ComponentHandle, LogicalPosition, LogicalSize,
    platform::{Key, PointerEventButton, WindowEvent},
};
use std::{cell::RefCell, rc::Rc};

slint::include_modules!();

fn flush() {
    slint::platform::update_timers_and_animations();
}

fn player(pane: i32) -> PlayerWindow {
    i_slint_backend_testing::init_no_event_loop();
    let app = PlayerWindow::new().unwrap();
    app.set_disable_animations(true);
    app.set_loaded(true);
    app.set_sidebar(pane);
    app.set_audio_delay_text("0".into());
    app.set_subtitle_delay_text("0".into());
    app.set_speed_edit_value("1".into());
    app.window().set_size(LogicalSize::new(1000.0, 900.0));
    app.show().unwrap();
    flush();
    app
}

fn field(app: &PlayerWindow, label: &str) -> ElementHandle {
    let mut found = ElementHandle::find_by_accessible_label(app, label);
    let field = found
        .next()
        .expect("Numeric field must exist in the player");
    assert!(found.next().is_none(), "Ambiguous field label");
    field
}

fn focus(app: &PlayerWindow, field: &ElementHandle) {
    let position = field.absolute_position();
    let size = field.size();
    assert!(size.width > 0.0 && size.height > 0.0);
    let position = LogicalPosition::new(
        position.x + size.width / 2.0,
        position.y + size.height / 2.0,
    );
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
    flush();
}

fn key(app: &PlayerWindow, text: impl Into<slint::SharedString>) {
    let text = text.into();
    app.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    app.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
    flush();
}

fn delay_submissions(app: &PlayerWindow) -> Rc<RefCell<Vec<(i32, String)>>> {
    let submissions = Rc::new(RefCell::new(Vec::new()));
    let recorded = submissions.clone();
    app.on_set_delay_text(move |lane, text| {
        recorded.borrow_mut().push((lane, text.to_string()));
        riina_core::timing_controls::parse_delay_seconds(&text).is_ok()
    });
    submissions
}

#[test]
fn controller_hide_keeps_delay_focus_and_return_submits_once() {
    let app = player(1);
    let submissions = delay_submissions(&app);
    let input = field(&app, "Audio delay in seconds");
    focus(&app, &input);
    input.set_accessible_value("0.7");
    key(&app, Key::End);
    app.set_controls_visible(false);
    flush();
    key(&app, "5");
    assert_eq!(input.accessible_value().unwrap(), "0.75");
    key(&app, Key::Return);
    assert_eq!(&*submissions.borrow(), &[(1, "0.75".into())]);
}

#[test]
fn draft_survives_observations_and_escape_restores_latest_without_submission() {
    let app = player(1);
    app.set_fullscreen(true);
    let fullscreen_toggles = Rc::new(std::cell::Cell::new(0));
    let toggles = fullscreen_toggles.clone();
    app.on_toggle_fullscreen(move || toggles.set(toggles.get() + 1));
    let submissions = delay_submissions(&app);
    let input = field(&app, "Audio delay in seconds");
    focus(&app, &input);
    input.set_accessible_value("-0.");
    app.set_audio_delay_text("2".into());
    flush();
    assert_eq!(input.accessible_value().unwrap(), "-0.");
    key(&app, Key::Escape);
    assert_eq!(input.accessible_value().unwrap(), "2");
    assert!(submissions.borrow().is_empty());
    assert_eq!(fullscreen_toggles.get(), 0);
    assert_eq!(app.get_sidebar(), 1);
    key(&app, Key::Escape);
    assert_eq!(
        fullscreen_toggles.get(),
        1,
        "Editing must return focus to the player"
    );
}

#[test]
fn changing_subtitle_lane_discards_its_draft_before_secondary_submission() {
    let app = player(2);
    app.set_secondary_subtitle_delay_known(true);
    app.set_secondary_subtitle_delay_text("-0.75".into());
    let submissions = delay_submissions(&app);
    let primary = field(&app, "Subtitle delay in seconds");
    focus(&app, &primary);
    primary.set_accessible_value("1.5");
    app.set_edit_secondary_subtitle(true);
    flush();
    let secondary = field(&app, "Secondary subtitle delay in seconds");
    assert_eq!(secondary.accessible_value().unwrap(), "-0.75");
    assert!(submissions.borrow().is_empty());
    focus(&app, &secondary);
    secondary.set_accessible_value("-0.25");
    key(&app, Key::Return);
    assert_eq!(&*submissions.borrow(), &[(5, "-0.25".into())]);
}

#[test]
fn empty_submit_resets_and_invalid_submit_remains_editable_until_cancelled() {
    let app = player(1);
    let submissions = delay_submissions(&app);
    let input = field(&app, "Audio delay in seconds");
    focus(&app, &input);
    input.set_accessible_value("");
    key(&app, Key::Return);
    assert_eq!(input.accessible_value().unwrap(), "0");
    focus(&app, &input);
    input.set_accessible_value("bad");
    key(&app, Key::Return);
    assert_eq!(input.accessible_value().unwrap(), "bad");
    key(&app, Key::Escape);
    assert_eq!(input.accessible_value().unwrap(), "0");
    assert_eq!(&*submissions.borrow(), &[(1, "".into()), (1, "bad".into())]);
}

#[test]
fn tab_commits_a_dirty_field_once() {
    let app = player(1);
    let submissions = delay_submissions(&app);
    let input = field(&app, "Audio delay in seconds");
    focus(&app, &input);
    input.set_accessible_value("-1.25");
    key(&app, Key::Tab);
    assert_eq!(&*submissions.borrow(), &[(1, "-1.25".into())]);
}

#[test]
fn speed_uses_the_same_draft_and_escape_lifecycle() {
    let app = player(6);
    let input = field(&app, "Custom playback speed");
    focus(&app, &input);
    input.set_accessible_value("1.");
    app.set_speed_edit_value("2".into());
    flush();
    assert_eq!(input.accessible_value().unwrap(), "1.");
    key(&app, Key::Escape);
    assert_eq!(input.accessible_value().unwrap(), "2");
    assert_eq!(app.get_sidebar(), 6);
}

#[test]
fn disabling_a_field_discards_its_draft_and_blocks_accessible_edits() {
    let app = player(1);
    let submissions = delay_submissions(&app);
    let input = field(&app, "Audio delay in seconds");
    focus(&app, &input);
    input.set_accessible_value("3");
    app.set_loaded(false);
    flush();
    input.set_accessible_value("5");
    key(&app, Key::Return);
    assert_eq!(input.accessible_value().unwrap(), "0");
    assert!(submissions.borrow().is_empty());
}

fn connect_settings_tabs(app: &PlayerWindow) {
    let weak = app.as_weak();
    app.on_select_settings_pane(move |pane| {
        let app = weak.upgrade().unwrap();
        if app.get_settings_at_leading() {
            app.set_leading_sidebar(pane);
        } else {
            app.set_sidebar(pane);
        }
    });
}

fn button(app: &PlayerWindow, label: &str) -> ElementHandle {
    ElementHandle::find_by_accessible_label(app, label)
        .find(|element| {
            element.accessible_role() == Some(i_slint_backend_testing::AccessibleRole::Button)
        })
        .unwrap_or_else(|| panic!("Missing button: {label}"))
}

#[test]
fn category_arrows_home_end_keep_focus_and_do_not_reach_playback() {
    let app = player(1);
    connect_settings_tabs(&app);
    let playback_keys = Rc::new(std::cell::Cell::new(0));
    let received = playback_keys.clone();
    app.on_dispatch_key_binding(move |_, _, _, _, _| {
        received.set(received.get() + 1);
        false
    });
    focus(&app, &button(&app, "Audio"));
    for (pressed, pane, label) in [
        (Key::LeftArrow, 6, "Video"),
        (Key::LeftArrow, 5, "Layout"),
        (Key::LeftArrow, 5, "Layout"),
        (Key::End, 2, "Subtitles"),
        (Key::RightArrow, 2, "Subtitles"),
        (Key::Home, 5, "Layout"),
        (Key::RightArrow, 6, "Video"),
        (Key::RightArrow, 1, "Audio"),
    ] {
        key(&app, pressed);
        assert_eq!(app.get_sidebar(), pane);
        assert_eq!(button(&app, label).accessible_item_selected(), Some(true));
    }
    key(&app, " ");
    assert_eq!(app.get_sidebar(), 1);
    assert_eq!(playback_keys.get(), 0);
}

#[test]
fn tab_leaves_the_category_group_for_the_page_in_one_step() {
    let app = player(1);
    connect_settings_tabs(&app);
    let browsed = Rc::new(std::cell::Cell::new(0));
    let count = browsed.clone();
    app.on_browse_files(move |pane| {
        assert_eq!(pane, 1);
        count.set(count.get() + 1);
    });
    focus(&app, &button(&app, "Audio"));
    key(&app, Key::Tab);
    key(&app, Key::Return);
    assert_eq!(
        browsed.get(),
        1,
        "Inactive categories must not add Tab stops"
    );
}

#[test]
fn narrow_category_buttons_remain_inside_both_sidebar_headers() {
    let app = player(1);
    connect_settings_tabs(&app);
    for leading in [false, true] {
        app.set_settings_at_leading(leading);
        app.set_sidebar(if leading { 0 } else { 1 });
        app.set_leading_sidebar(if leading { 1 } else { 0 });
        for compact in [false, true] {
            app.set_compact_ui(compact);
            for width in [285.0, 320.0, 440.0, 960.0] {
                app.window().set_size(LogicalSize::new(width, 600.0));
                flush();
                let close = button(&app, "Close sidebar");
                let close_x = close.absolute_position().x;
                let close_right = close_x + close.size().width;
                for label in ["Layout", "Video", "Audio", "Subtitles"] {
                    let tab = button(&app, label);
                    let pos = tab.absolute_position();
                    let size = tab.size();
                    assert!(size.width >= 28.0 && size.height >= 24.0);
                    assert!(
                        pos.x >= 0.0 && pos.x + size.width <= width + 0.5,
                        "{label} clipped at width {width}, leading={leading}, compact={compact}: {pos:?} {size:?}"
                    );
                    assert!(
                        pos.x + size.width <= close_x || pos.x >= close_right,
                        "Category overlaps close control"
                    );
                }
                focus(&app, &button(&app, "Audio"));
                key(&app, Key::End);
                assert_eq!(
                    if leading {
                        app.get_leading_sidebar()
                    } else {
                        app.get_sidebar()
                    },
                    2
                );
                key(&app, Key::Home);
                key(&app, Key::RightArrow);
                key(&app, Key::RightArrow);
            }
        }
    }
}

#[test]
fn tab_enters_the_delay_editor_from_its_slider_without_an_invisible_stop() {
    let app = player(1);
    let input = field(&app, "Audio delay in seconds");
    focus(&app, &field(&app, "Audio delay"));
    key(&app, Key::Tab);
    input.set_accessible_value("0.7");
    key(&app, Key::End);
    key(&app, "5");
    assert_eq!(input.accessible_value().unwrap(), "0.75");
}

#[test]
fn reverse_tab_leaves_the_editor_for_its_slider_and_commits_once() {
    let app = player(1);
    let submissions = delay_submissions(&app);
    let slider_changes = Rc::new(RefCell::new(Vec::new()));
    let changes = slider_changes.clone();
    app.on_set_delay(move |lane, value| changes.borrow_mut().push((lane, value)));
    let input = field(&app, "Audio delay in seconds");
    focus(&app, &input);
    input.set_accessible_value("0.75");
    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Shift.into(),
    });
    key(&app, Key::Tab);
    app.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Shift.into(),
    });
    key(&app, Key::RightArrow);
    assert_eq!(&*submissions.borrow(), &[(1, "0.75".into())]);
    let changes = slider_changes.borrow();
    assert_eq!(
        changes.len(),
        1,
        "Reverse Tab must focus the preceding slider"
    );
    assert_eq!(changes[0].0, 1);
    assert!((changes[0].1 - 0.05).abs() < 0.0001);
}

#[cfg(target_os = "windows")]
#[path = "../src/platform/windows/menu_modifiers.rs"]
mod menu_modifiers;

#[cfg(target_os = "windows")]
#[test]
fn menu_return_clears_a_consumed_alt_release_before_category_navigation() {
    let app = player(1);
    connect_settings_tabs(&app);
    let modified_keys = Rc::new(RefCell::new(Vec::new()));
    let recorded = modified_keys.clone();
    app.on_dispatch_key_binding(move |text, control, alt, shift, meta| {
        recorded
            .borrow_mut()
            .push((text.to_string(), control, alt, shift, meta));
        true
    });
    focus(&app, &button(&app, "Audio"));
    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Alt.into(),
    });
    modified_keys.borrow_mut().clear();
    key(&app, Key::LeftArrow);
    assert_eq!(
        app.get_sidebar(),
        1,
        "First reproduce the stale Alt rejection"
    );
    assert!(modified_keys.borrow().last().unwrap().2);
    modified_keys.borrow_mut().clear();
    menu_modifiers::reconcile_alt(app.window(), false);
    assert!(!app.get_menu_alternates_visible());
    key(&app, Key::LeftArrow);
    assert_eq!(app.get_sidebar(), 6);
    assert!(
        modified_keys.borrow().is_empty(),
        "Plain Left must stay in the category group"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn menu_return_preserves_a_physically_held_alt_and_other_modifiers() {
    let app = player(1);
    app.invoke_focus_player();
    let modifiers = Rc::new(RefCell::new(Vec::new()));
    let recorded = modifiers.clone();
    app.on_dispatch_key_binding(move |text, control, alt, shift, meta| {
        if text == "x" {
            recorded.borrow_mut().push((control, alt, shift, meta));
        }
        true
    });
    for modifier in [Key::Control, Key::Shift, Key::Alt] {
        app.window().dispatch_event(WindowEvent::KeyPressed {
            text: modifier.into(),
        });
    }
    menu_modifiers::reconcile_alt(app.window(), true);
    key(&app, "x");
    assert_eq!(&*modifiers.borrow(), &[(true, true, true, false)]);
    assert!(app.get_menu_alternates_visible());
    menu_modifiers::reconcile_alt(app.window(), false);
    key(&app, "x");
    assert_eq!(
        &*modifiers.borrow(),
        &[(true, true, true, false), (true, false, true, false)]
    );
    assert!(!app.get_menu_alternates_visible());
}

#[cfg(target_os = "windows")]
#[test]
fn menu_return_keeps_the_active_editor_draft_and_submission_scope() {
    let app = player(1);
    let submissions = delay_submissions(&app);
    let input = field(&app, "Audio delay in seconds");
    focus(&app, &input);
    input.set_accessible_value("0.7");
    key(&app, Key::End);
    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Alt.into(),
    });
    menu_modifiers::reconcile_alt(app.window(), false);
    key(&app, "5");
    assert_eq!(input.accessible_value().unwrap(), "0.75");
    key(&app, Key::Return);
    assert_eq!(&*submissions.borrow(), &[(1, "0.75".into())]);
}
