use super::*;
use slint::{ComponentHandle, Model};
use std::{
    cell::Cell,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[test]
fn audio_and_subtitle_actions_activate_on_first_click() {
    let app = player(1);
    let menu = app.global::<PlayerMenuActions>();
    let file_kind = Rc::new(Cell::new(None));
    let recorded = file_kind.clone();
    menu.on_browse_files(move |kind| recorded.set(Some(kind)));
    let searches = Rc::new(Cell::new(0));
    let recorded = searches.clone();
    menu.on_find_online_subtitles(move || recorded.set(recorded.get() + 1));

    field(&app, "Load external audio").mock_single_click(PointerEventButton::Left);
    assert_eq!(file_kind.get(), Some(1));
    switch(&app, 2);
    field(&app, "Search online subtitles").mock_single_click(PointerEventButton::Left);
    assert_eq!(searches.get(), 1);
    key(&app, Key::Return);
    assert_eq!(searches.get(), 2);
}

fn model() -> slint::ModelRc<CatalogRow> {
    playlist_model(3)
}

fn playlist_model(count: usize) -> slint::ModelRc<CatalogRow> {
    slint::ModelRc::new(slint::VecModel::from(
        (0..count)
            .map(|index| CatalogRow {
                token: (index + 1).to_string().into(),
                label: format!("Clip {index}").into(),
                selected: index == 0,
                playing: index == 0,
                ..Default::default()
            })
            .collect::<Vec<_>>(),
    ))
}

fn switch(app: &PlayerWindow, pane: i32) {
    app.global::<riina_ui::PlayerSidebarState>()
        .set_sidebar(pane);
    flush();
    // Headless components are instantiated lazily. Materialize a recreated
    // scrolling body before advancing its deferred-restore timer.
    if pane != 0 && pane != 5 {
        let _ = page_y(app);
    }
    flush();
    i_slint_backend_testing::testing_backend::mock_elapsed_time(4);
    flush();
}

fn scroll(app: &PlayerWindow, delta: f32) {
    app.window().dispatch_event(WindowEvent::PointerScrolled {
        position: LogicalPosition::new(950.0, 200.0),
        delta_x: 0.0,
        delta_y: delta,
    });
    flush();
    let _ = page_y(app);
    i_slint_backend_testing::testing_backend::mock_elapsed_time(500);
    flush();
}

#[expect(
    clippy::expect_used,
    reason = "A missing scroll page must fail the widget test"
)]
fn page_y(app: &PlayerWindow) -> f32 {
    // The page body remains visible when the first card scrolls out of view.
    ElementHandle::find_by_element_id(app, "PlayerWindow::sidebar-page")
        .next()
        .expect("Visible sidebar page")
        .absolute_position()
        .y
}

#[test]
fn settings_pages_keep_independent_scroll_positions() {
    let app = player(6);
    app.window().set_size(LogicalSize::new(1000.0, 320.0));
    app.global::<riina_ui::PlaybackVideoState>()
        .set_video_tracks(model());
    app.global::<riina_ui::PlaybackAudioState>()
        .set_audio_tracks(model());
    app.global::<riina_ui::PlaybackSubtitleState>()
        .set_subtitle_tracks(model());
    app.global::<riina_ui::PlaybackSubtitleState>()
        .set_secondary_subtitle_tracks(model());
    i_slint_backend_testing::testing_backend::mock_elapsed_time(4);
    flush();
    let top = page_y(&app);
    // Distinct, valid offsets test pane ownership without elastic overscroll.
    scroll(&app, -150.0);
    let video = page_y(&app);
    assert!(video < top - 40.0);
    switch(&app, 2);
    assert!(
        page_y(&app) > 30.0,
        "Subtitles must not inherit Video's scroll offset"
    );
    switch(&app, 1);
    let audio_top = page_y(&app);
    assert!((audio_top - top).abs() < 1.0);
    scroll(&app, -100.0);
    let audio = page_y(&app);
    assert!(audio < audio_top - 40.0);
    switch(&app, 6);
    assert!(
        (page_y(&app) - video).abs() < 1.0,
        "restored={} expected={video}, audio={audio}, top={top}",
        page_y(&app)
    );
    switch(&app, 1);
    assert!((page_y(&app) - audio).abs() < 1.0);
    switch(&app, 5);
    switch(&app, 1);
    assert!(
        (page_y(&app) - audio).abs() < 1.0,
        "Layout must not destroy the other panes' saved positions: restored={} expected={audio}",
        page_y(&app)
    );
}

#[test]
fn playlist_rows_and_six_footer_actions_fit_normal_and_narrow_sidebars() {
    let app = player(4);
    app.global::<riina_ui::PlayerCatalogState>()
        .set_playlist(model());
    for width in [1000.0, 285.0] {
        app.window().set_size(LogicalSize::new(width, 700.0));
        for dark in [false, true] {
            app.global::<riina_ui::PlayerAppearance>()
                .set_dark_theme(dark);
            flush();
            for index in 0..3 {
                let row = button(&app, &format!("Clip {index}"));
                assert!(
                    row.size().width >= 180.0,
                    "Playlist rows must not collapse to zero width"
                );
                assert!(row.absolute_position().x + row.size().width <= width);
            }
            let playlist = button(&app, "Playlist");
            let chapters = button(&app, "Chapters");
            assert!(playlist.size().width >= 76.0 && chapters.size().width >= 76.0);
            assert!(
                playlist.absolute_position().x + playlist.size().width
                    <= chapters.absolute_position().x
            );
            let mut right = 0.0;
            for label in [
                "Cycle repeat: Off",
                "Shuffle playlist",
                "Sort playlist",
                "Add files to playlist",
                "Remove selected playlist entries",
                "Clear playlist except current file",
            ] {
                let action = button(&app, label);
                let x = action.absolute_position().x;
                assert!(
                    x >= right && x + action.size().width <= width,
                    "Footer overlap or clipping: {label} at {width}"
                );
                right = x + action.size().width;
            }
        }
    }
}

#[test]
fn playlist_context_requests_keep_row_and_blank_targets_in_both_surfaces() {
    let app = player(4);
    app.global::<PlayerCatalogState>().set_playlist(model());
    let requested = Rc::new(RefCell::new(Vec::new()));
    let observe = requested.clone();
    app.global::<PlaylistInteraction>()
        .on_request_playlist_menu(move |origin, token, x, y| {
            observe.borrow_mut().push((origin, token, x, y));
        });
    flush();
    let music = app.global::<PlayerMusicModeState>();
    let mut expected = Vec::new();
    for origin in [PlaylistMenuOrigin::Sidebar, PlaylistMenuOrigin::MusicMode] {
        if origin == PlaylistMenuOrigin::MusicMode {
            app.global::<PlayerSidebarState>().set_sidebar(0);
            music.set_music_mode(true);
            music.set_music_mode_show_playlist(true);
        }
        flush();
        let row = button(&app, "Clip 1").absolute_position();
        let last = button(&app, "Clip 2");
        let blank_y = last.absolute_position().y + last.size().height + 12.0;
        let row_position = LogicalPosition::new(row.x + 12.0, row.y + 10.0);
        let blank_position = LogicalPosition::new(row.x + 12.0, blank_y);
        for (position, button) in [
            (row_position, PointerEventButton::Left),
            (row_position, PointerEventButton::Right),
            (blank_position, PointerEventButton::Right),
        ] {
            app.window()
                .dispatch_event(WindowEvent::PointerPressed { position, button });
            app.window()
                .dispatch_event(WindowEvent::PointerReleased { position, button });
            flush();
        }
        expected.extend([
            (origin, "2".into(), row_position.x, row_position.y),
            (origin, "".into(), blank_position.x, blank_position.y),
        ]);
    }
    assert_eq!(*requested.borrow(), expected);
}

#[test]
fn playlist_drop_target_tracks_visible_rows_and_scroll_position() {
    let app = player(4);
    app.window().set_size(LogicalSize::new(1000.0, 320.0));
    app.global::<PlayerCatalogState>()
        .set_playlist(slint::ModelRc::new(slint::VecModel::from(
            (0..30)
                .map(|index| CatalogRow {
                    token: index.to_string().into(),
                    label: format!("Clip {index}").into(),
                    ..Default::default()
                })
                .collect::<Vec<_>>(),
        )));
    let sidebar = app.global::<PlayerSidebarState>();
    let first_row = button(&app, "Clip 0");
    flush();
    i_slint_backend_testing::testing_backend::mock_elapsed_time(4);
    flush();
    let first = first_row.absolute_position();
    let x = first.x + 8.0;
    let y = first.y + 5.0;
    assert_eq!(sidebar.invoke_playlist_drop_slot(x, y), 0);
    assert_eq!(sidebar.invoke_playlist_drop_slot(x, first.y + 29.0), 1);
    assert_eq!(sidebar.invoke_playlist_drop_slot(x, first.y - 4.0), -1);
    assert_eq!(sidebar.invoke_playlist_drop_slot(first.x - 4.0, y), -1);

    let top = page_y(&app);
    scroll(&app, -160.0);
    assert!(page_y(&app) < top - 30.0);
    assert!(sidebar.invoke_playlist_drop_slot(x, y) > 0);
    sidebar.set_sidebar(0);
    flush();
    assert_eq!(sidebar.invoke_playlist_drop_slot(x, y), -1);
    sidebar.set_leading_sidebar(4);
    let left_row = button(&app, "Clip 0");
    flush();
    let left = left_row.absolute_position();
    assert_eq!(
        sidebar.invoke_playlist_drop_slot(left.x + 8.0, left.y + 5.0),
        0
    );
    assert_eq!(sidebar.invoke_playlist_drop_slot(x, y), -1);
}

#[test]
fn playlist_keyboard_selection_moves_focus_and_opens_the_selected_token() {
    let app = player(4);
    app.global::<riina_ui::PlayerCatalogState>()
        .set_playlist(model());
    app.global::<riina_ui::PlayerSidebarActions>()
        .on_select_playlist_row(|index, _, _| {
            slint::ModelRc::new(slint::VecModel::from(vec![(index + 1).to_string().into()]))
        });
    let opened = Rc::new(RefCell::new(Vec::new()));
    let observed = opened.clone();
    app.global::<PlayerMenuActions>()
        .on_select_item(move |kind, token| observed.borrow_mut().push((kind, token.to_string())));
    flush();

    focus(&app, &button(&app, "Clip 0"));
    key(&app, Key::DownArrow);
    assert_eq!(
        app.global::<riina_ui::PlaylistInteraction>()
            .get_selected_playlist_token(),
        "2"
    );
    key(&app, Key::Return);
    assert_eq!(
        opened.borrow().as_slice(),
        &[(CatalogSelection::Playlist, String::from("2"))]
    );
    assert!(
        app.global::<riina_ui::PlaylistInteraction>()
            .get_selected_playlist_token()
            .is_empty()
    );
}

#[test]
fn playlist_focus_tracks_its_row_index_after_an_insertion() {
    let app = player(4);
    let rows = Rc::new(slint::VecModel::from(model().iter().collect::<Vec<_>>()));
    app.global::<PlayerCatalogState>()
        .set_playlist(rows.clone().into());
    switch(&app, 4);
    focus(&app, &button(&app, "Clip 1"));
    let interaction = app.global::<riina_ui::PlaylistInteraction>();
    assert_eq!(interaction.get_focused_playlist_token(), "2");
    rows.insert(
        0,
        CatalogRow {
            token: "99".into(),
            label: "Inserted clip".into(),
            ..Default::default()
        },
    );
    flush();
    button(&app, "Inserted clip");
    flush();
    key(&app, Key::UpArrow);
    assert_eq!(interaction.get_selected_playlist_token(), "1");
    assert_eq!(interaction.get_focused_playlist_token(), "1");
}

#[test]
fn playlist_navigation_takes_precedence_over_the_same_player_menu_shortcut() {
    let app = player(4);
    app.global::<PlayerCatalogState>().set_playlist(model());
    let mut shortcuts = vec![MenuShortcutRow::default(); 35];
    shortcuts[34].shortcut = slint::Keys::from_parts(["DownArrow"]).unwrap();
    app.set_menu_shortcuts(slint::ModelRc::new(slint::VecModel::from(shortcuts)));
    let adjustments = Rc::new(Cell::new(0.0));
    let recorded = adjustments.clone();
    app.global::<PlayerMenuActions>()
        .on_adjust_volume(move |delta| recorded.set(recorded.get() + delta));
    switch(&app, 4);
    focus(&app, &button(&app, "Clip 0"));
    key(&app, Key::DownArrow);
    assert_eq!(adjustments.get(), 0.0);
    assert_eq!(
        app.global::<riina_ui::PlaylistInteraction>()
            .get_focused_playlist_token(),
        "2"
    );
    app.global::<PlayerSidebarState>().set_sidebar(0);
    app.invoke_focus_player();
    flush();
    key(&app, Key::DownArrow);
    assert_eq!(adjustments.get(), -5.0);
}

#[test]
fn playlist_keyboard_reveals_rows_and_extends_selection_in_both_surfaces() {
    let app = player(4);
    app.window().set_size(LogicalSize::new(1000.0, 460.0));
    app.global::<PlayerCatalogState>()
        .set_playlist(playlist_model(30));
    let selection = Rc::new(RefCell::new(Vec::new()));
    let observed = selection.clone();
    app.global::<PlayerSidebarActions>()
        .on_select_playlist_row(move |index, _, range| {
            observed.borrow_mut().push((index, range));
            slint::ModelRc::new(slint::VecModel::from(vec![(index + 1).to_string().into()]))
        });
    let opened = Rc::new(RefCell::new(Vec::new()));
    let observed = opened.clone();
    app.global::<PlayerMenuActions>()
        .on_select_item(move |_, token| observed.borrow_mut().push(token.to_string()));
    let edits = Rc::new(RefCell::new(Vec::new()));
    let observed = edits.clone();
    let interaction = app.global::<riina_ui::PlaylistInteraction>();
    interaction.on_edit(move |action, apply| {
        observed.borrow_mut().push((action, apply));
        true
    });
    switch(&app, 4);
    for music_mode in [false, true] {
        if music_mode {
            let sidebar = app.global::<PlayerSidebarState>();
            let top_row = sidebar.get_playlist_scroll_row();
            assert_eq!(sidebar.get_playlist_row_stride(), 31.0);
            switch(&app, 0);
            let music = app.global::<PlayerMusicModeState>();
            music.set_music_mode(true);
            music.set_music_mode_show_playlist(true);
            flush();
            let _ = button(&app, "Clip 0");
            i_slint_backend_testing::testing_backend::mock_elapsed_time(4);
            flush();
            assert!(top_row < -8.0);
            assert_eq!(sidebar.get_playlist_row_stride(), 39.0);
            let realized = sidebar.get_music_list_drop_viewport().content_y;
            assert!((realized - top_row * 39.0).abs() < 1.0);
            let visible_index = (-top_row).ceil() as usize + 1;
            let near_end = button(&app, &format!("Clip {visible_index}"));
            focus(&app, &near_end);
            key(&app, Key::Home);
        }
        let first = button(&app, "Clip 0");
        let initial_y = first.absolute_position().y;
        focus(&app, &first);
        assert_eq!(interaction.get_focused_playlist_token(), "1");
        app.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Control.into(),
        });
        key(&app, "a");
        key(&app, "c");
        key(&app, "x");
        key(&app, "v");
        app.window().dispatch_event(WindowEvent::KeyReleased {
            text: Key::Control.into(),
        });
        key(&app, Key::Delete);
        app.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Shift.into(),
        });
        key(&app, Key::End);
        app.window().dispatch_event(WindowEvent::KeyReleased {
            text: Key::Shift.into(),
        });
        assert_eq!(selection.borrow().last(), Some(&(29, true)));
        assert!(first.absolute_position().y < initial_y - 250.0);
        let last = button(&app, "Clip 29");
        assert!(last.absolute_position().y + last.size().height <= 460.0);
        key(&app, Key::PageUp);
        assert!(
            selection
                .borrow()
                .last()
                .is_some_and(|(index, range)| *index < 29 && !range)
        );
        key(&app, Key::End);
        key(&app, Key::Return);
    }
    assert_eq!(opened.borrow().as_slice(), ["30", "30"]);
    let expected = [
        riina_ui::PlaylistEdit::SelectAll,
        riina_ui::PlaylistEdit::Copy,
        riina_ui::PlaylistEdit::Cut,
        riina_ui::PlaylistEdit::Paste,
        riina_ui::PlaylistEdit::Remove,
    ];
    assert!(
        edits.borrow().iter().copied().eq(expected
            .into_iter()
            .chain(expected)
            .map(|action| (action, true)))
    );
}

#[test]
fn chapter_rows_keep_their_cursor_between_modes_and_seek_on_activation() {
    let app = player(3);
    app.window().set_size(LogicalSize::new(1000.0, 460.0));
    app.global::<PlayerCatalogState>()
        .set_chapters(slint::ModelRc::new(slint::VecModel::from(
            (0..24)
                .map(|index| CatalogRow {
                    token: index.to_string().into(),
                    label: format!("Chapter {index}").into(),
                    detail: format!("{index}:00 → End").into(),
                    selected: index == 0,
                    ..Default::default()
                })
                .collect::<Vec<_>>(),
        )));
    let jumps = Rc::new(RefCell::new(Vec::new()));
    let observed = jumps.clone();
    app.global::<PlayerMenuActions>()
        .on_select_item(move |kind, token| {
            assert_eq!(kind, CatalogSelection::Chapter);
            observed.borrow_mut().push(token.to_string());
        });
    let sidebar = app.global::<PlayerSidebarState>();
    sidebar.set_playlist_at_leading(false);
    switch(&app, 3);
    focus(&app, &button(&app, "Chapter 0"));
    key(&app, Key::End);
    assert!(jumps.borrow().is_empty());
    assert_eq!(sidebar.get_chapter_cursor(), 23);
    let offset = sidebar.get_chapters_scroll_y();
    assert!(offset < -250.0);
    key(&app, Key::Return);
    assert_eq!(jumps.borrow().as_slice(), ["23"]);

    sidebar.set_remembered_playlist_pane(3);
    let music = app.global::<PlayerMusicModeState>();
    music.set_music_mode(true);
    music.set_music_mode_show_playlist(true);
    flush();
    let _ = button(&app, "Chapter 0");
    i_slint_backend_testing::testing_backend::mock_elapsed_time(4);
    flush();
    assert_eq!(sidebar.get_chapter_cursor(), 23);
    assert_eq!(sidebar.get_chapters_scroll_y(), offset);
    assert_eq!(
        button(&app, "Chapters").accessible_item_selected(),
        Some(true)
    );
    let visible_index = (-offset / 40.0).ceil() as usize + 1;
    let near_end = button(&app, &format!("Chapter {visible_index}"));
    let position = near_end.absolute_position();
    let tabs = button(&app, "Chapters");
    let rows_top = tabs.absolute_position().y + tabs.size().height + 2.0;
    let expected_y = rows_top + visible_index as f32 * 40.0 + offset;
    assert!((position.y - expected_y).abs() <= 1.0);
    assert!(position.y + near_end.size().height <= 460.0);
    assert_eq!(
        sidebar.invoke_playlist_drop_slot(position.x + 5.0, position.y + 5.0),
        -1
    );
    focus(&app, &near_end);
    key(&app, Key::PageUp);
    assert!(sidebar.get_chapter_cursor() < 23);
    assert_eq!(jumps.borrow().as_slice(), ["23"]);
    key(&app, Key::Return);
    assert_eq!(jumps.borrow().len(), 2);
}

#[test]
fn quick_settings_tab_reveals_equalizer_and_restores_its_scroll_position() {
    let app = player(1);
    app.window().set_size(LogicalSize::new(1000.0, 290.0));
    app.global::<PlaybackAudioState>().set_audio_tracks(model());
    switch(&app, 1);
    let initial_y = page_y(&app);
    focus(&app, &field(&app, "Audio tracks"));
    for _ in 0..5 {
        key(&app, Key::Tab);
    }
    let revealed_y = page_y(&app);
    assert!(revealed_y < initial_y - 60.0);
    let first_band = field(&app, "32 Hz equalizer gain");
    assert!(first_band.absolute_position().y + first_band.size().height <= 290.0);
    switch(&app, 6);
    switch(&app, 1);
    assert!((page_y(&app) - revealed_y).abs() < 1.0);
}

#[test]
fn paused_catalog_transitions_mature_without_another_engine_notification() {
    let app = player(0);
    let audio: riina_core::Track =
        serde_json::from_value(serde_json::json!({"id":1,"type":"audio"})).unwrap();
    let video: riina_core::Track =
        serde_json::from_value(serde_json::json!({"id":2,"type":"video"})).unwrap();
    let playback = Arc::new(Mutex::new(Arc::new(riina_core::Playback {
        phase: riina_core::Phase::Paused,
        tracks: vec![audio].into(),
        ..Default::default()
    })));
    let mode = Arc::new(Mutex::new(riina_core::music_mode::State::new(true)));
    let past = Instant::now() - Duration::from_secs(1);
    mode.lock()
        .unwrap()
        .observe_at(riina_core::music_mode::AudioStatus::Audio, false, past);
    let _timer = automatic_music_mode::install(&app, playback.clone(), mode.clone());
    i_slint_backend_testing::testing_backend::mock_elapsed_time(110);
    assert!(
        app.global::<riina_ui::PlayerMusicModeState>()
            .get_music_mode(),
        "A quiet paused catalog still needs its debounce completion"
    );
    Arc::make_mut(&mut Arc::make_mut(&mut playback.lock().unwrap()).tracks).push(video);
    mode.lock()
        .unwrap()
        .observe_at(riina_core::music_mode::AudioStatus::Video, false, past);
    i_slint_backend_testing::testing_backend::mock_elapsed_time(110);
    assert!(
        !app.global::<riina_ui::PlayerMusicModeState>()
            .get_music_mode()
    );
    mode.lock().unwrap().toggle_manually();
    app.global::<riina_ui::PlayerMusicModeState>()
        .set_music_mode(true);
    i_slint_backend_testing::testing_backend::mock_elapsed_time(110);
    assert!(
        app.global::<riina_ui::PlayerMusicModeState>()
            .get_music_mode(),
        "Polling must preserve the user's manual override"
    );
}

#[test]
fn music_window_buttons_do_not_cover_transport_or_timeline() {
    let app = player(0);
    app.global::<riina_ui::PlayerMusicModeState>()
        .set_music_mode(true);
    app.global::<riina_ui::PlayerControlsState>()
        .set_controls_visible(true);
    app.global::<riina_ui::PlaybackTimelineState>()
        .set_seekable(true);
    for artwork in [true, false] {
        app.global::<riina_ui::PlayerMusicModeState>()
            .set_music_mode_show_album_art(artwork);
        app.window()
            .set_size(LogicalSize::new(300.0, if artwork { 390.0 } else { 90.0 }));
        flush();
        for label in ["Close player", "Return to player"] {
            let action = button(&app, label);
            let position = action.absolute_position();
            let size = action.size();
            assert!(position.y < 45.0 && size.width <= 13.0);
            for other in ["Volume", "Previous media", "Playback position"] {
                let item = field(&app, other);
                let p = item.absolute_position();
                let s = item.size();
                assert!(
                    position.x + size.width <= p.x
                        || p.x + s.width <= position.x
                        || position.y + size.height <= p.y
                        || p.y + s.height <= position.y,
                    "{label} overlaps {other}, artwork={artwork}"
                );
            }
        }
    }
}

#[test]
fn first_pointer_press_opens_subtitle_color_and_changes_an_equalizer_band() {
    let app = player(1);
    app.global::<riina_ui::PlaybackAudioState>()
        .set_audio_equalizer_values(slint::ModelRc::new(slint::VecModel::from(vec![0.0; 10])));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let recorded = changes.clone();
    app.global::<riina_ui::PlayerSidebarActions>()
        .on_set_audio_equalizer(move |index, value| recorded.borrow_mut().push((index, value)));
    focus(&app, &field(&app, "32 Hz equalizer gain"));
    assert!(!changes.borrow().is_empty());
    assert!(
        changes
            .borrow()
            .iter()
            .all(|(index, gain)| *index == 0 && gain.abs() < 0.5)
    );
    switch(&app, 2);
    app.global::<riina_ui::PlaybackSubtitleState>()
        .set_subtitle_text_color_known(true);
    app.global::<riina_ui::PlaybackSubtitleState>()
        .set_subtitle_text_style_available(true);
    flush();
    focus(&app, &field(&app, "Subtitle text color"));
    assert!(
        ElementHandle::find_by_accessible_label(&app, "Subtitle text color red")
            .next()
            .is_some()
    );
}

#[test]
fn auxiliary_acrylic_fills_are_neutral_white_in_both_themes() {
    let _player = player(0);
    let history = PlaybackHistoryWindow::new().unwrap();
    assert_eq!(
        history
            .global::<AuxiliaryPalette>()
            .get_light_acrylic_fill(),
        slint::Color::from_argb_u8(64, 255, 255, 255)
    );
    assert_eq!(
        history.global::<AuxiliaryPalette>().get_dark_acrylic_fill(),
        slint::Color::from_argb_u8(9, 255, 255, 255)
    );
}
