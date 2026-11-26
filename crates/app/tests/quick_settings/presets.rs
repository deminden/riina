use super::*;

#[test]
fn video_rails_reveal_focus_and_keep_one_tab_stop_per_group() {
    let app = player(6);
    app.window().set_size(LogicalSize::new(335.0, 700.0));
    let video = app.global::<PlaybackVideoState>();
    video.set_video_aspect_known(true);
    video.set_video_crop_known(true);
    video.set_video_rotation_known(true);
    video.set_video_aspect_presets(slint::ModelRc::new(slint::VecModel::from(
        riina_core::video_controls::ASPECT_PRESETS
            .map(slint::SharedString::from)
            .to_vec(),
    )));
    video.set_video_crop_presets(slint::ModelRc::new(slint::VecModel::from(
        ["None", "4:3", "16:9", "16:10", "21:9", "5:4", "Custom"]
            .map(slint::SharedString::from)
            .to_vec(),
    )));
    video.set_video_crop_index(0);
    video.set_video_rotation_index(0);
    let actions = app.global::<PlayerSidebarActions>();
    let aspects = Rc::new(RefCell::new(Vec::new()));
    let recorded = aspects.clone();
    actions.on_set_video_aspect(move |value| recorded.borrow_mut().push(value));
    let crops = Rc::new(RefCell::new(Vec::new()));
    let recorded = crops.clone();
    actions.on_set_video_crop(move |value| recorded.borrow_mut().push(value));
    let rotations = Rc::new(RefCell::new(Vec::new()));
    let recorded = rotations.clone();
    actions.on_set_video_rotation(move |value| recorded.borrow_mut().push(value));
    let playback_keys = Rc::new(std::cell::Cell::new(0));
    let received = playback_keys.clone();
    app.global::<PlayerControlActions>()
        .on_dispatch_key_binding(move |_, _, _, _, _, _| {
            received.set(received.get() + 1);
            false
        });
    flush();

    focus(&app, &button(&app, "Video"));
    key(&app, Key::Tab);
    playback_keys.set(0);
    // Engine observations can lag a keyboard repeat without stalling traversal.
    key(&app, Key::RightArrow);
    key(&app, Key::RightArrow);
    key(&app, Key::End);
    key(&app, Key::RightArrow);
    video.set_video_aspect_index(5);
    key(&app, Key::LeftArrow);
    assert_eq!(playback_keys.get(), 0);
    assert_eq!(
        &*aspects.borrow(),
        &["4:3".into(), "16:9".into(), "5:4".into(), "21:9".into()] as &[slint::SharedString]
    );

    // The selected preset can be after the cursor while mpv is catching up.
    key(&app, Key::Tab);
    let custom = field(&app, "Custom aspect ratio");
    assert!(custom.absolute_position().x + custom.size().width <= 335.5);
    video.set_video_custom_aspect("2.35:1".into());
    flush();
    assert_eq!(custom.accessible_value().as_deref(), Some("2.35:1"));
    custom.set_accessible_value("2.39:1");
    video.set_video_custom_aspect("1.85:1".into());
    flush();
    assert_eq!(custom.accessible_value().as_deref(), Some("2.39:1"));
    key(&app, Key::Escape);
    assert_eq!(custom.accessible_value().as_deref(), Some("1.85:1"));
    assert_eq!(aspects.borrow().len(), 4, "Escape must not submit an edit");

    focus(&app, &custom);
    custom.set_accessible_value("2.39:1");
    key(&app, Key::Tab);
    assert_eq!(
        aspects.borrow().last().map(|value| value.as_str()),
        Some("2.39:1")
    );
    assert_eq!(aspects.borrow().len(), 5);
    key(&app, Key::End);
    assert!(crops.borrow().is_empty(), "Custom Crop requires activation");
    key(&app, Key::Return);
    assert_eq!(&*crops.borrow(), &[6]);
    key(&app, Key::LeftArrow);
    assert_eq!(&*crops.borrow(), &[6, 5]);

    key(&app, Key::Tab);
    key(&app, Key::End);
    key(&app, Key::RightArrow);
    key(&app, Key::Home);
    assert_eq!(&*rotations.borrow(), &[270, 0]);
}

#[test]
fn subtitle_size_menu_commits_once_and_keeps_engine_owned_values() {
    let app = player(2);
    let subtitles = app.global::<PlaybackSubtitleState>();
    subtitles.set_subtitle_text_style_available(true);
    subtitles.set_subtitle_font_size(68.5);
    subtitles.set_subtitle_font_size_known(true);
    subtitles.set_subtitle_outline_size(0.75);
    let changes = Rc::new(RefCell::new(Vec::new()));
    let recorded = changes.clone();
    app.global::<PlayerSidebarActions>()
        .on_set_subtitle_font_size(move |value| recorded.borrow_mut().push((false, value)));
    let recorded = changes.clone();
    app.global::<PlayerSidebarActions>()
        .on_set_subtitle_outline_size(move |value| recorded.borrow_mut().push((true, value)));
    let menu_keys = Rc::new(std::cell::Cell::new(0));
    let received = menu_keys.clone();
    app.global::<PlayerMenuActions>()
        .on_toggle_fullscreen(move || received.set(received.get() + 1));
    let shortcuts = Rc::new(std::cell::Cell::new(0));
    let received = shortcuts.clone();
    app.global::<PlayerControlActions>()
        .on_dispatch_key_binding(move |text, control, _, _, _, _| {
            let down: slint::SharedString = Key::DownArrow.into();
            let handled = control && text == down;
            if handled {
                received.set(received.get() + 1);
            }
            handled
        });
    flush();
    let font_size = field(&app, "Subtitle font size");
    let outline_width = field(&app, "Subtitle outline width");
    assert_eq!(font_size.accessible_value().as_deref(), Some("68.5"));
    assert_eq!(outline_width.accessible_value().as_deref(), Some("—"));
    outline_width.invoke_accessible_default_action();
    assert!(changes.borrow().is_empty());

    font_size.invoke_accessible_default_action();
    i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
    key(&app, Key::DownArrow);
    key(&app, Key::Escape);
    assert!(changes.borrow().is_empty());
    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Control.into(),
    });
    key(&app, Key::DownArrow);
    app.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Control.into(),
    });
    assert_eq!(shortcuts.get(), 1);
    key(&app, " ");
    i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
    key(&app, Key::DownArrow);
    key(&app, Key::Return);
    assert_eq!(&*changes.borrow(), &[(false, 25.0)]);
    assert_eq!(font_size.accessible_value().as_deref(), Some("68.5"));
    assert_eq!(menu_keys.get(), 0);

    subtitles.set_edit_secondary_subtitle(true);
    subtitles.set_subtitle_font_size(25.0);
    subtitles.set_subtitle_outline_size_known(true);
    flush();
    assert_eq!(font_size.accessible_value().as_deref(), Some("25"));
    assert_eq!(outline_width.accessible_value().as_deref(), Some("0.75"));
    outline_width.invoke_accessible_default_action();
    i_slint_backend_testing::testing_backend::mock_elapsed_time(1);
    key(&app, Key::DownArrow);
    key(&app, Key::DownArrow);
    key(&app, Key::Return);
    assert_eq!(&*changes.borrow(), &[(false, 25.0), (true, 0.25)]);
    assert_eq!(outline_width.accessible_value().as_deref(), Some("0.75"));

    app.window().set_size(LogicalSize::new(285.0, 310.0));
    focus(&app, &button(&app, "Subtitles"));
    key(&app, Key::Tab);
    outline_width.invoke_accessible_default_action();
    flush();
    let position = outline_width.absolute_position();
    let size = outline_width.size();
    assert!(position.y >= 0.0 && position.y + size.height <= 310.0);
    assert!(position.x >= 0.0 && position.x + size.width <= 285.5);
    key(&app, Key::Escape);
    assert_eq!(changes.borrow().len(), 2);
}
