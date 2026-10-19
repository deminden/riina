use super::*;

fn track_row(app: &PlayerWindow, label: &str) -> ElementHandle {
    ElementHandle::find_by_accessible_label(app, label)
        .find(|element| {
            element.accessible_role() == Some(i_slint_backend_testing::AccessibleRole::ListItem)
        })
        .unwrap_or_else(|| panic!("Missing track row: {label}"))
}

fn tracks(count: usize, selected: usize) -> slint::ModelRc<CatalogRow> {
    slint::ModelRc::new(slint::VecModel::from(
        (0..count)
            .map(|index| CatalogRow {
                token: if index == 0 {
                    "no".into()
                } else {
                    index.to_string().into()
                },
                label: format!("Track {index}").into(),
                language: if index == 0 {
                    "".into()
                } else {
                    "English".into()
                },
                selected: index == selected,
                ..Default::default()
            })
            .collect::<Vec<_>>(),
    ))
}

fn set_tracks(app: &PlayerWindow, kind: i32, count: usize, selected: usize) {
    let model = tracks(count, selected);
    match kind {
        1 => {
            app.set_audio_track_selected_index(selected as i32);
            app.set_audio_tracks(model);
        }
        6 => {
            app.set_video_track_selected_index(selected as i32);
            app.set_video_tracks(model);
        }
        2 => {
            app.set_subtitle_track_selected_index(selected as i32);
            app.set_subtitle_tracks(model);
        }
        5 => {
            app.set_secondary_subtitle_track_selected_index(selected as i32);
            app.set_secondary_subtitle_tracks(model);
        }
        _ => panic!("Not a Quick Settings track kind"),
    }
    flush();
}

fn row_label(kind: i32, index: usize) -> String {
    let prefix = match kind {
        2 => "Primary subtitle: ",
        5 => "Secondary subtitle: ",
        _ => "",
    };
    format!("{prefix}Track {index}")
}

fn navigation(kind: i32) {
    let app = player(if kind == 5 { 2 } else { kind });
    set_tracks(&app, kind, 8, 2);
    let selected = Rc::new(RefCell::new(Vec::new()));
    let received = selected.clone();
    app.on_select_item(move |kind, token| received.borrow_mut().push((kind, token.to_string())));
    let playback = Rc::new(std::cell::Cell::new(0));
    let received = playback.clone();
    app.on_dispatch_key_binding(move |_, _, _, _, _| {
        received.set(received.get() + 1);
        false
    });
    focus(&app, &track_row(&app, &row_label(kind, 2)));
    selected.borrow_mut().clear();
    // No backend echo: fast repeated input must advance its own cursor, rather
    // than repeatedly choosing the neighbor of the last observed selection.
    for pressed in [Key::DownArrow, Key::DownArrow, Key::DownArrow] {
        key(&app, pressed);
    }
    assert_eq!(
        &*selected.borrow(),
        &[(kind, "3".into()), (kind, "4".into()), (kind, "5".into())]
    );
    let top = track_row(&app, &row_label(kind, 5)).absolute_position().y;
    let list = field(
        &app,
        match kind {
            1 => "Audio tracks",
            6 => "Video tracks",
            2 => "Primary subtitle tracks",
            _ => "Secondary subtitle tracks",
        },
    );
    assert!(top >= list.absolute_position().y);
    assert!(top + 26.0 <= list.absolute_position().y + list.size().height);
    assert_eq!(
        track_row(&app, &row_label(kind, 2)).accessible_item_selected(),
        Some(true),
        "Focus is not an engine observation"
    );
    assert_eq!(
        track_row(&app, &row_label(kind, 5)).accessible_item_selected(),
        Some(false)
    );
    // A model replacement must preserve the keyboard owner and pending cursor.
    set_tracks(&app, kind, 8, 4);
    selected.borrow_mut().clear();
    for pressed in [
        Key::UpArrow,
        Key::End,
        Key::DownArrow,
        Key::Home,
        Key::UpArrow,
        Key::Return,
    ] {
        key(&app, pressed);
    }
    key(&app, " ");
    assert_eq!(
        &*selected.borrow(),
        &[
            (kind, "4".into()),
            (kind, "7".into()),
            (kind, "7".into()),
            (kind, "no".into()),
            (kind, "no".into()),
            (kind, "no".into()),
            (kind, "no".into())
        ]
    );
    assert_eq!(playback.get(), 0);
}

#[test]
fn audio_arrows_advance_and_reveal_without_waiting_for_observations() {
    navigation(1);
}
#[test]
fn video_arrows_advance_and_reveal_without_waiting_for_observations() {
    navigation(6);
}
#[test]
fn primary_subtitle_arrows_keep_their_lane_and_keyboard_owner() {
    navigation(2);
}
#[test]
fn secondary_subtitle_arrows_keep_their_lane_and_keyboard_owner() {
    navigation(5);
}

#[test]
fn tab_visits_the_list_once_and_focus_alone_does_not_switch_tracks() {
    let app = player(1);
    connect_settings_tabs(&app);
    set_tracks(&app, 1, 8, 2);
    let selected = Rc::new(RefCell::new(Vec::new()));
    let received = selected.clone();
    app.on_select_item(move |_, token| received.borrow_mut().push(token.to_string()));
    let browsed = Rc::new(std::cell::Cell::new(0));
    let received = browsed.clone();
    app.on_browse_files(move |_| received.set(received.get() + 1));
    focus(&app, &button(&app, "Audio"));
    key(&app, Key::Tab);
    assert!(selected.borrow().is_empty());
    key(&app, Key::DownArrow);
    assert_eq!(&*selected.borrow(), &["3"]);
    key(&app, Key::Tab);
    key(&app, Key::Return);
    assert_eq!(browsed.get(), 1, "Rows must not add separate Tab stops");
}

#[test]
fn disabled_and_empty_lists_cannot_dispatch_stale_track_tokens() {
    let app = player(1);
    set_tracks(&app, 1, 8, 2);
    let selected = Rc::new(RefCell::new(Vec::new()));
    let received = selected.clone();
    app.on_select_item(move |_, token| received.borrow_mut().push(token.to_string()));
    let row = track_row(&app, "Track 2");
    focus(&app, &row);
    selected.borrow_mut().clear();
    app.set_loaded(false);
    flush();
    row.invoke_accessible_default_action();
    key(&app, Key::Return);
    assert!(selected.borrow().is_empty());
    app.set_loaded(true);
    set_tracks(&app, 1, 0, 0);
    key(&app, Key::DownArrow);
    key(&app, Key::Return);
    assert!(selected.borrow().is_empty());
}

#[test]
fn language_badges_fit_the_name_and_narrow_card_in_both_themes() {
    let app = player(6);
    app.set_video_tracks(slint::ModelRc::new(slint::VecModel::from(vec![
        CatalogRow {
            token: "1".into(),
            label: "Video with a long track title".into(),
            language: "Português (Brasil)".into(),
            selected: true,
            ..Default::default()
        },
    ])));
    for dark in [false, true] {
        app.set_dark_theme(dark);
        for width in [285.0, 440.0, 960.0] {
            app.window().set_size(LogicalSize::new(width, 900.0));
            flush();
            let row = track_row(&app, "Video with a long track title");
            let name = descendant(&row, "QuickSettingsTrackList::name-caption");
            let badge = descendant(&row, "QuickSettingsTrackList::language-tag");
            assert!(
                badge.size().width > 38.0,
                "The language badge must grow beyond its old fixed frame"
            );
            assert!(badge.size().width <= row.size().width * 0.45 + 0.5);
            assert!(name.size().width > 0.0);
            assert!(
                name.absolute_position().x + name.size().width + 4.0 <= badge.absolute_position().x
            );
            assert!(
                badge.absolute_position().x + badge.size().width
                    <= row.absolute_position().x + row.size().width
            );
        }
    }
}

#[test]
fn page_navigation_and_a_shorter_model_keep_the_cursor_in_bounds() {
    let app = player(1);
    set_tracks(&app, 1, 12, 1);
    let selected = Rc::new(RefCell::new(Vec::new()));
    let received = selected.clone();
    app.on_select_item(move |_, token| received.borrow_mut().push(token.to_string()));
    focus(&app, &track_row(&app, "Track 1"));
    selected.borrow_mut().clear();
    key(&app, Key::PageDown);
    key(&app, Key::PageDown);
    key(&app, Key::PageUp);
    assert_eq!(&*selected.borrow(), &["4", "7", "4"]);
    set_tracks(&app, 1, 2, 0);
    selected.borrow_mut().clear();
    key(&app, Key::Return);
    assert_eq!(
        &*selected.borrow(),
        &["1"],
        "A shortened list must not retain an obsolete token"
    );
}
