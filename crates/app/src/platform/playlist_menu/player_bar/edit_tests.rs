use super::new_player;
use i_slint_backend_testing::ElementHandle;
use riina_ui::{
    ApplicationMenuText, CatalogRow, PlayerCatalogState, PlayerSidebarState, PlaylistEdit,
    PlaylistInteraction, PlaylistPluginPopupState, PlaylistPopupInput,
};
use slint::{
    ComponentHandle, LogicalPosition, LogicalSize, Model, ModelRc, VecModel,
    platform::{PointerEventButton, WindowEvent},
};
use std::{cell::Cell, rc::Rc};

#[test]
fn edit_menu_restores_the_playlist_target_before_dispatch() {
    i_slint_backend_testing::init_no_event_loop();
    let app = new_player().unwrap();
    slint::select_bundled_translation("en").unwrap();
    app.window().set_size(LogicalSize::new(1000.0, 600.0));
    app.set_loaded(true);
    app.global::<PlayerSidebarState>().set_sidebar(4);
    app.global::<PlayerCatalogState>()
        .set_playlist(ModelRc::new(VecModel::from(vec![CatalogRow {
            token: "clip-id".into(),
            label: "Fixture clip for editing".into(),
            ..Default::default()
        }])));
    crate::platform::text_edit::install_player_menu(&app);
    let copied = Rc::new(Cell::new(false));
    let seen = copied.clone();
    let weak = app.as_weak();
    app.global::<PlaylistInteraction>()
        .on_edit(move |operation, apply| {
            let current = weak.upgrade().unwrap();
            let focus = current
                .global::<PlaylistInteraction>()
                .get_focused_playlist_token();
            if operation != PlaylistEdit::Copy || focus != "clip-id" {
                return false;
            }
            if apply {
                seen.set(true);
            }
            true
        });

    app.show().unwrap();
    slint::platform::update_timers_and_animations();
    let row = ElementHandle::find_by_accessible_label(&app, "Fixture clip for editing")
        .find(|item| item.size().height > 0.0)
        .unwrap();
    let position = row.absolute_position();
    let position = LogicalPosition::new(position.x + 10.0, position.y + 8.0);
    let button = PointerEventButton::Left;
    app.window()
        .dispatch_event(WindowEvent::PointerPressed { position, button });
    app.window()
        .dispatch_event(WindowEvent::PointerReleased { position, button });
    slint::platform::update_timers_and_animations();
    assert_eq!(
        app.global::<PlaylistInteraction>()
            .get_focused_playlist_token(),
        "clip-id"
    );

    let title = app.global::<ApplicationMenuText>().get_edit_title();
    let family = app
        .get_player_menu_labels()
        .iter()
        .position(|name| name == title)
        .unwrap();
    app.invoke_open_player_menu(i32::try_from(family).unwrap(), 120.0, 38.0);
    slint::platform::update_timers_and_animations();
    let popup = app.global::<PlaylistPluginPopupState>();
    let rows = popup.get_panels().row_data(0).unwrap().rows;
    assert!(
        rows.iter()
            .any(|item| item.title == "Undo" && !item.enabled)
    );
    let copy = rows
        .iter()
        .position(|item| item.title == "Copy" && item.enabled)
        .unwrap();
    popup.invoke_input(
        PlaylistPopupInput::Choose,
        0,
        i32::try_from(copy).unwrap(),
        44.0,
    );
    slint::platform::update_timers_and_animations();
    assert!(copied.get());
    assert_eq!(
        app.global::<PlaylistInteraction>()
            .get_focused_playlist_token(),
        "clip-id"
    );
}
