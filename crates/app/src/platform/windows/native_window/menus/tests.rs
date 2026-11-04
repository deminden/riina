use super::*;

#[test]
fn opening_menu_refreshes_availability_without_waiting_for_coalescing_timer() {
    i_slint_backend_testing::init_no_event_loop();
    let host = NativeVideoHost::new().unwrap();
    let app = PlayerWindow::new().unwrap();
    install(&host, &app);
    app.set_loaded(true);
    app.set_reflection_known(true);
    app.set_video_mirrored(true);
    app.set_screenshot_available(true);
    app.invoke_native_menu_will_open();
    assert!(host.get_playback_loaded());
    assert!(host.get_reflection_menu_enabled());
    assert!(host.get_video_mirrored());
    assert!(host.get_screenshot_available());
    app.set_crop_editing(true);
    app.invoke_native_menu_will_open();
    assert!(!host.get_playback_loaded());
    assert!(!host.get_reflection_menu_enabled());
    assert!(!host.get_screenshot_available());
}

#[test]
fn opening_menu_refreshes_rows_and_shortcuts_without_replaying_actions() {
    i_slint_backend_testing::init_no_event_loop();
    let host = NativeVideoHost::new().unwrap();
    let app = PlayerWindow::new().unwrap();
    install(&host, &app);
    app.set_loop_label("AB loop".into());
    app.set_recent_open_menu_rows(slint::ModelRc::new(slint::VecModel::from(vec![
        crate::MenuCatalogRow {
            token: "new.mp4".into(),
            label: "New media".into(),
            selected: false,
        },
    ])));
    app.invoke_native_menu_will_open();
    assert_eq!(host.get_loop_label(), "AB loop");
    assert_eq!(
        host.get_recent_open_menu_rows().row_data(0).unwrap().token,
        "new.mp4"
    );
    app.set_recent_open_menu_rows(slint::ModelRc::new(slint::VecModel::from(Vec::<
        crate::MenuCatalogRow,
    >::new())));
    app.invoke_native_menu_will_open();
    assert_eq!(host.get_recent_open_menu_rows().row_count(), 0);
}
