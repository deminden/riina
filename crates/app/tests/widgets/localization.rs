use i_slint_backend_testing::ElementHandle;
use riina_ui::{
    AboutWindow, LogViewerWindow, OpenUrlWindow, PlaybackHistoryWindow, RecentMediaRow,
    VideoFiltersWindow, WelcomeWindow,
};
use slint::{ComponentHandle, LogicalSize, Model, ModelRc, VecModel, select_bundled_translation};
use std::{cell::Cell, rc::Rc};

#[test]
fn bundled_languages_render_labels_and_preserve_actions_in_narrow_dialogs()
-> Result<(), Box<dyn std::error::Error>> {
    i_slint_backend_testing::init_no_event_loop();
    let welcome = WelcomeWindow::new()?;
    welcome.set_recent_media(ModelRc::new(VecModel::from(vec![RecentMediaRow {
        token: "localized.mkv".into(),
        title: "مقطع اختبار".into(),
        position_label: "01:23".into(),
    }])));
    let activated = Rc::new(Cell::new(0));
    let recorded = activated.clone();
    welcome.on_open_file(move || recorded.set(recorded.get() + 1));
    welcome.show()?;

    let stream = OpenUrlWindow::new()?;
    stream.set_authentication_available(true);
    stream.window().set_size(LogicalSize::new(380., 250.));
    stream.show()?;
    let history = PlaybackHistoryWindow::new()?;
    let filters = VideoFiltersWindow::new()?;
    let about = AboutWindow::new()?;
    let log = LogViewerWindow::new()?;
    log.set_logs_available(true);
    log.set_visible_entry_count(4);
    log.set_total_entry_count(10);

    select_bundled_translation("en")?;
    assert_eq!(
        stream.get_invalid_address_message(),
        "Enter a valid stream URL"
    );
    assert_eq!(filters.get_preset_prompt(), "Choose Preset…");
    log.set_selected_subsystem_count(1);
    assert_eq!(log.get_selected_subsystems_label(), "1 Subsystem");
    assert_eq!(log.get_status_text(), "4 of 10 log entries");

    select_bundled_translation("ru")?;
    super::flush();
    let file = ElementHandle::find_by_accessible_label(&welcome, "Открыть медиафайл")
        .next()
        .ok_or("Missing Russian file action")?;
    file.mock_single_click(slint::platform::PointerEventButton::Left);
    assert_eq!(activated.get(), 1);
    assert_eq!(about.get_engine_label(), "Проверяется версия libmpv…");
    assert_eq!(
        history.get_recent_searches().row_data(0).as_deref(),
        Some("Недавние поиски")
    );
    let caption = "Запомнить имя пользователя и пароль для этого сервера";
    assert!(
        ElementHandle::find_by_accessible_label(&stream, caption)
            .next()
            .is_some()
    );
    let remember = ElementHandle::find_by_element_id(&stream, "OpenUrlWindow::remember-caption")
        .next()
        .ok_or("Missing Russian authentication action")?;
    let position = remember.absolute_position();
    let size = remember.size();
    assert!(size.width > 0. && size.height > 0.);
    assert!(position.x >= 0. && position.x + size.width <= 380.);
    assert!(position.y >= 0. && position.y + size.height <= 250.);
    remember.mock_single_click(slint::platform::PointerEventButton::Left);
    assert!(stream.get_remember());
    for (count, expected) in [
        (2, "2 подсистемы"),
        (5, "5 подсистем"),
        (21, "21 подсистема"),
    ] {
        log.set_selected_subsystem_count(count);
        assert_eq!(log.get_selected_subsystems_label(), expected);
    }

    select_bundled_translation("ar")?;
    super::flush();
    assert_eq!(stream.get_invalid_address_message(), "أدخل رابط بث صالحًا");
    assert_eq!(log.get_status_text(), "إدخالات السجل: 4 من 10");
    log.set_selected_subsystem_count(2);
    assert_eq!(log.get_selected_subsystems_label(), "نظامان فرعيان");
    assert!(
        ElementHandle::find_by_accessible_label(&welcome, "استئناف مقطع اختبار، 01:23")
            .next()
            .is_some()
    );

    select_bundled_translation("en")?;
    exercise_welcome_documents()?;
    stream.hide()?;
    welcome.hide()?;
    Ok(())
}

fn exercise_welcome_documents() -> Result<(), Box<dyn std::error::Error>> {
    use slint::platform::{Key, PointerEventButton, WindowEvent};
    use std::cell::RefCell;

    let welcome = WelcomeWindow::new()?;
    let rows = (1..=10)
        .map(|index| RecentMediaRow {
            token: format!("directory/recording {index}.mkv").into(),
            title: format!("Recording {index}").into(),
            ..RecentMediaRow::default()
        })
        .collect::<Vec<_>>();
    welcome.set_recent_documents(ModelRc::new(VecModel::from(rows.clone())));
    welcome.set_selection_index(0);
    let received = Rc::new(RefCell::new(Vec::new()));
    let events = received.clone();
    welcome.on_open_recent(move |token, resume| events.borrow_mut().push((token, resume)));
    welcome.show()?;
    super::flush();
    let press = |key: Key| {
        let text: slint::SharedString = key.into();
        welcome
            .window()
            .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
        welcome
            .window()
            .dispatch_event(WindowEvent::KeyReleased { text });
    };
    for _ in 1..rows.len() {
        press(Key::DownArrow);
    }
    press(Key::Return);
    super::flush();
    let last = ElementHandle::find_by_accessible_label(&welcome, "Recording 10")
        .next()
        .ok_or("Welcome hid recent documents without a Resume entry")?;
    assert!(last.absolute_position().y + last.size().height <= 440.0);
    for _ in 1..rows.len() {
        press(Key::UpArrow);
    }
    super::flush();
    let first = ElementHandle::find_by_accessible_label(&welcome, "Recording 1")
        .next()
        .ok_or("Missing first recording")?;
    first.mock_single_click(PointerEventButton::Left);
    assert_eq!(
        *received.borrow(),
        [
            (rows[9].token.clone(), false),
            (rows[0].token.clone(), false)
        ]
    );
    welcome.hide()?;
    Ok(())
}
