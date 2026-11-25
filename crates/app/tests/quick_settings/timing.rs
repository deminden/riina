use super::*;
use std::cell::Cell;

fn slider_point(app: &PlayerWindow, name: &str, fraction: f32) -> LogicalPosition {
    let slider = field(app, name);
    let position = slider.absolute_position();
    let size = slider.size();
    LogicalPosition::new(
        position.x + size.width * fraction,
        position.y + size.height / 2.0,
    )
}

fn drag_press(app: &PlayerWindow, position: LogicalPosition) {
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    flush();
}

fn drag_release(app: &PlayerWindow, position: LogicalPosition) {
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
    flush();
}

#[test]
fn audio_and_subtitle_delay_drags_preview_then_commit_once_with_lane_steps() {
    let app = player(1);
    let actions = app.global::<PlayerSidebarActions>();
    actions.on_format_delay_preview(|value| format!("{value:.2}").into());
    let subtitles = app.global::<PlaybackSubtitleState>();
    subtitles.set_secondary_subtitle_delay_known(true);
    for (pane, lane, name, preview) in [
        (1, DelayLane::Audio, "Audio delay", "1.20"),
        (2, DelayLane::PrimarySubtitle, "Subtitle delay", "1.00"),
        (
            2,
            DelayLane::SecondarySubtitle,
            "Secondary subtitle delay",
            "1.00",
        ),
    ] {
        app.global::<PlayerSidebarState>().set_sidebar(pane);
        subtitles.set_edit_secondary_subtitle(lane == DelayLane::SecondarySubtitle);
        let changes = Rc::new(RefCell::new(Vec::new()));
        let recorded = changes.clone();
        actions.on_set_delay(move |lane, value| recorded.borrow_mut().push((lane, value)));
        flush();
        drag_press(&app, slider_point(&app, name, 0.43));
        let position = slider_point(&app, name, 0.6165);
        app.window()
            .dispatch_event(WindowEvent::PointerMoved { position });
        flush();
        let input = field(&app, &format!("{name} in seconds"));
        assert_eq!(input.accessible_value().as_deref(), Some(preview));
        assert!(changes.borrow().is_empty());
        drag_release(&app, position);
        let changes = changes.borrow();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].0, lane);
        let expected: f32 = preview.parse().unwrap();
        assert!((changes[0].1 - expected).abs() < 0.001);
    }
}

#[test]
fn changing_subtitle_lane_retires_keyboard_and_pointer_drafts() {
    let app = player(2);
    let subtitles = app.global::<PlaybackSubtitleState>();
    subtitles.set_secondary_subtitle_delay_known(true);
    subtitles.set_secondary_subtitle_delay_text("-1".into());
    let submissions = delay_submissions(&app);
    let input = field(&app, "Subtitle delay in seconds");
    focus(&app, &input);
    input.set_accessible_value("3.25");
    let secondary = button(&app, "Edit secondary subtitle timing and position");
    secondary.mock_single_click(PointerEventButton::Left);
    flush();
    assert!(subtitles.get_edit_secondary_subtitle());
    assert!(submissions.borrow().is_empty());
    assert_eq!(
        field(&app, "Secondary subtitle delay in seconds")
            .accessible_value()
            .as_deref(),
        Some("-1")
    );
    key(&app, Key::LeftArrow);
    assert!(!subtitles.get_edit_secondary_subtitle());
    key(&app, Key::RightArrow);
    assert!(subtitles.get_edit_secondary_subtitle());

    let slider_commits = Rc::new(Cell::new(0));
    let observed = slider_commits.clone();
    app.global::<PlayerSidebarActions>()
        .on_set_delay(move |_, _| observed.set(observed.get() + 1));
    let position = slider_point(&app, "Secondary subtitle delay", 0.25);
    drag_press(&app, position);
    button(&app, "Edit primary subtitle timing and position").invoke_accessible_default_action();
    flush();
    drag_release(&app, position);
    assert_eq!(slider_commits.get(), 0);
    let position = slider_point(&app, "Subtitle delay", 0.25);
    drag_press(&app, position);
    app.window().dispatch_event(WindowEvent::PointerExited);
    drag_release(&app, position);
    assert_eq!(slider_commits.get(), 0);
}

#[test]
fn timing_fields_handle_accessibility_and_keep_playback_keys_out_of_text_edits() {
    let app = player(6);
    let actions = app.global::<PlayerSidebarActions>();
    let speed = Rc::new(RefCell::new(Vec::new()));
    let recorded = speed.clone();
    actions.on_set_speed_text(move |value| {
        recorded.borrow_mut().push(value.to_string());
        riina_core::speed_controls::from_text(&value).is_ok()
    });
    let input = field(&app, "Custom playback speed");
    input.set_accessible_value("2.25");
    assert_eq!(&*speed.borrow(), &["2.25"]);
    app.global::<PlaybackTimelineState>()
        .set_speed_edit_value("2.25".into());
    flush();
    input.set_accessible_value("bad");
    assert_eq!(input.accessible_value().as_deref(), Some("2.25"));
    input.set_accessible_value("");
    assert_eq!(input.accessible_value().as_deref(), Some("1"));
    assert_eq!(&*speed.borrow(), &["2.25", "bad", ""]);

    let shortcuts = Rc::new(Cell::new(0));
    let recorded = shortcuts.clone();
    app.global::<PlayerControlActions>()
        .on_dispatch_key_binding(move |text, _, alt, _, _| {
            if text == "v" && alt {
                recorded.set(recorded.get() + 1);
            }
            false
        });
    focus(&app, &input);
    for editing in [true, false] {
        app.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Alt.into(),
        });
        key(&app, "v");
        app.window().dispatch_event(WindowEvent::KeyReleased {
            text: Key::Alt.into(),
        });
        assert_eq!(shortcuts.get(), u32::from(!editing));
        if editing {
            key(&app, Key::Escape);
        }
    }
}

#[test]
fn numeric_editing_keeps_its_draft_while_a_modified_player_shortcut_runs() {
    let app = player(6);
    let screenshots = Rc::new(Cell::new(0));
    let observed = screenshots.clone();
    app.global::<PlayerMenuActions>()
        .on_take_screenshot(move || observed.set(observed.get() + 1));
    let edits = Rc::new(Cell::new(0));
    let observed = edits.clone();
    app.global::<PlayerSidebarActions>()
        .on_set_speed_text(move |_| {
            observed.set(observed.get() + 1);
            true
        });
    let input = field(&app, "Custom playback speed");
    focus(&app, &input);
    input.set_accessible_value("2.25");
    for modifier in [Key::Control, Key::Shift] {
        app.window().dispatch_event(WindowEvent::KeyPressed {
            text: modifier.into(),
        });
    }
    key(&app, "S");
    for modifier in [Key::Shift, Key::Control] {
        app.window().dispatch_event(WindowEvent::KeyReleased {
            text: modifier.into(),
        });
    }
    assert_eq!(screenshots.get(), 1);
    assert_eq!(edits.get(), 0);
    assert_eq!(input.accessible_value().as_deref(), Some("2.25"));
    key(&app, Key::Escape);
    assert_eq!(input.accessible_value().as_deref(), Some("1"));
    assert_eq!(edits.get(), 0);
}
