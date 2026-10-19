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
