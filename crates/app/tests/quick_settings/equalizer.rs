use super::*;

fn profiles(app: &PlayerWindow) {
    app.set_audio_equalizer_profile_names(slint::ModelRc::new(slint::VecModel::from(vec![
        "Manual".into(),
        "Flat".into(),
        "Rock".into(),
        "映画 🎧".into(),
    ])));
    app.set_audio_equalizer_user_profile_start_index(3);
    app.set_audio_equalizer_can_save_profile(true);
    app.set_audio_equalizer_values(slint::ModelRc::new(slint::VecModel::from(vec![0.0; 10])));
    app.on_validate_audio_equalizer_profile_name(|name, _| {
        if name.is_empty() {
            "Enter a profile name".into()
        } else if name == "Rock" {
            "Already exists".into()
        } else {
            "".into()
        }
    });
    flush();
}

#[test]
fn picker_chevron_is_centered_inside_its_hit_frame_in_both_themes() {
    let app = player(1);
    profiles(&app);
    for width in [1000.0, 285.0] {
        app.window().set_size(LogicalSize::new(width, 900.0));
        for dark in [false, true] {
            app.set_dark_theme(dark);
            flush();
            let picker = field(&app, "Equalizer profile: Manual");
            let arrow = descendant(&picker, "AudioEqualizerProfilePicker::chevron");
            assert!(
                (arrow.absolute_position().y + arrow.size().height / 2.0
                    - picker.absolute_position().y
                    - picker.size().height / 2.0)
                    .abs()
                    < 0.5
            );
            // A 90-degree transform maps the image origin to its right edge.
            assert!(
                arrow.absolute_position().x - arrow.size().height >= picker.absolute_position().x
            );
            assert!(
                arrow.absolute_position().x <= picker.absolute_position().x + picker.size().width
            );
        }
    }
}

#[test]
fn equalizer_fader_passes_modified_shortcuts_and_tab_reaches_the_profile_picker() {
    let app = player(1);
    profiles(&app);
    let gains = Rc::new(RefCell::new(Vec::new()));
    let recorded = gains.clone();
    app.on_set_audio_equalizer(move |index, value| recorded.borrow_mut().push((index, value)));
    let shortcuts = Rc::new(RefCell::new(Vec::new()));
    let recorded = shortcuts.clone();
    app.on_dispatch_key_binding(move |text, control, _, _, _| {
        let right: slint::SharedString = Key::RightArrow.into();
        let handled = control && text == right;
        recorded.borrow_mut().push((text.to_string(), control));
        handled
    });
    focus(&app, &field(&app, "32 Hz equalizer gain"));
    gains.borrow_mut().clear();
    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Control.into(),
    });
    shortcuts.borrow_mut().clear();
    key(&app, Key::RightArrow);
    app.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Control.into(),
    });
    assert!(gains.borrow().is_empty());
    let arrow: slint::SharedString = Key::RightArrow.into();
    assert_eq!(&*shortcuts.borrow(), &[(arrow.to_string(), true)]);
    key(&app, Key::UpArrow);
    assert_eq!(gains.borrow().len(), 1);
    assert_eq!(gains.borrow()[0].0, 0);
    assert!((gains.borrow()[0].1 - 0.1).abs() < 0.001);
    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Shift.into(),
    });
    key(&app, Key::Tab);
    app.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Shift.into(),
    });
    shortcuts.borrow_mut().clear();
    key(&app, Key::Return);
    assert!(
        shortcuts.borrow().is_empty(),
        "Return belongs to the profile picker"
    );
    activate_save(&app);
    assert!(
        ElementHandle::find_by_accessible_label(&app, "New equalizer profile name")
            .next()
            .is_some()
    );
}

fn activate_save(app: &PlayerWindow) {
    i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
    key(app, Key::DownArrow);
    key(app, Key::Return);
    i_slint_backend_testing::testing_backend::mock_elapsed_time(32);
    flush();
}

fn naming_dialog(app: &PlayerWindow) -> ElementHandle {
    field(app, "Equalizer profile: Manual").invoke_accessible_default_action();
    flush();
    activate_save(app);
    field(app, "New equalizer profile name")
}

#[test]
fn profile_prompt_focus_validates_submits_once_and_restores_picker_focus() {
    let app = player(1);
    profiles(&app);
    let saved = Rc::new(RefCell::new(Vec::new()));
    let recorded = saved.clone();
    app.on_save_audio_equalizer_profile(move |name| {
        recorded.borrow_mut().push(name.to_string());
        true
    });
    let editor = naming_dialog(&app);
    key(&app, Key::Return);
    assert!(saved.borrow().is_empty(), "Blank names cannot submit");
    editor.set_accessible_value("Rock");
    key(&app, Key::Return);
    assert!(saved.borrow().is_empty(), "Duplicate names cannot submit");
    editor.set_accessible_value("映画 🎧");
    key(&app, Key::End);
    key(&app, "!");
    assert_eq!(editor.accessible_value().unwrap(), "映画 🎧!");
    key(&app, Key::Return);
    assert_eq!(&*saved.borrow(), &["映画 🎧!"]);
    assert!(
        ElementHandle::find_by_accessible_label(&app, "New equalizer profile name")
            .next()
            .is_none()
    );
    key(&app, Key::Return);
    activate_save(&app);
    assert!(
        ElementHandle::find_by_accessible_label(&app, "New equalizer profile name")
            .next()
            .is_some()
    );
}

#[test]
fn escape_cancels_a_profile_prompt_without_reaching_fullscreen_or_playback() {
    let app = player(1);
    profiles(&app);
    app.set_fullscreen(true);
    let commands = Rc::new(std::cell::Cell::new(0));
    let recorded = commands.clone();
    app.on_toggle_fullscreen(move || recorded.set(recorded.get() + 1));
    let recorded = commands.clone();
    app.on_dispatch_key_binding(move |_, _, _, _, _| {
        recorded.set(recorded.get() + 1);
        true
    });
    naming_dialog(&app).set_accessible_value("Cancelled");
    key(&app, Key::Escape);
    assert_eq!(commands.get(), 0);
    assert!(
        ElementHandle::find_by_accessible_label(&app, "New equalizer profile name")
            .next()
            .is_none()
    );
}
