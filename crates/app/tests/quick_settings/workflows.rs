use super::*;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

fn model() -> slint::ModelRc<CatalogRow> {
    slint::ModelRc::new(slint::VecModel::from(
        (0..3)
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
    app.set_sidebar(pane);
    flush();
    // Headless components are instantiated lazily. Materialize a recreated
    // scrolling body before advancing its deferred-restore timer.
    if pane != 5 {
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
    app.set_video_tracks(model());
    app.set_audio_tracks(model());
    app.set_subtitle_tracks(model());
    app.set_secondary_subtitle_tracks(model());
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
    app.set_playlist(model());
    for width in [1000.0, 285.0] {
        app.window().set_size(LogicalSize::new(width, 700.0));
        for dark in [false, true] {
            app.set_dark_theme(dark);
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
fn paused_catalog_transitions_mature_without_another_engine_notification() {
    let app = player(0);
    let audio: riina_core::Track =
        serde_json::from_value(serde_json::json!({"id":1,"type":"audio"})).unwrap();
    let video: riina_core::Track =
        serde_json::from_value(serde_json::json!({"id":2,"type":"video"})).unwrap();
    let playback = Arc::new(Mutex::new(riina_core::Playback {
        phase: riina_core::Phase::Paused,
        tracks: vec![audio],
        ..Default::default()
    }));
    let mode = Arc::new(Mutex::new(riina_core::music_mode::State::new(true)));
    let past = Instant::now() - Duration::from_secs(1);
    mode.lock()
        .unwrap()
        .observe_at(riina_core::music_mode::AudioStatus::Audio, false, past);
    let _timer = automatic_music_mode::install(&app, playback.clone(), mode.clone());
    i_slint_backend_testing::testing_backend::mock_elapsed_time(110);
    assert!(
        app.get_music_mode(),
        "A quiet paused catalog still needs its debounce completion"
    );
    playback.lock().unwrap().tracks.push(video);
    mode.lock()
        .unwrap()
        .observe_at(riina_core::music_mode::AudioStatus::Video, false, past);
    i_slint_backend_testing::testing_backend::mock_elapsed_time(110);
    assert!(!app.get_music_mode());
    mode.lock().unwrap().toggle_manually();
    app.set_music_mode(true);
    i_slint_backend_testing::testing_backend::mock_elapsed_time(110);
    assert!(
        app.get_music_mode(),
        "Polling must preserve the user's manual override"
    );
}

#[test]
fn music_window_buttons_do_not_cover_transport_or_timeline() {
    let app = player(0);
    app.set_music_mode(true);
    app.set_controls_visible(true);
    app.set_seekable(true);
    for artwork in [true, false] {
        app.set_music_mode_show_album_art(artwork);
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
    app.set_audio_equalizer_values(slint::ModelRc::new(slint::VecModel::from(vec![0.0; 10])));
    let changes = Rc::new(RefCell::new(Vec::new()));
    let recorded = changes.clone();
    app.on_set_audio_equalizer(move |index, value| recorded.borrow_mut().push((index, value)));
    focus(&app, &field(&app, "32 Hz equalizer gain"));
    assert!(!changes.borrow().is_empty());
    assert!(
        changes
            .borrow()
            .iter()
            .all(|(index, gain)| *index == 0 && gain.abs() < 0.5)
    );
    switch(&app, 2);
    app.set_subtitle_text_color_known(true);
    app.set_subtitle_text_style_available(true);
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
