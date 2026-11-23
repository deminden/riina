use super::{flush, player};
use i_slint_backend_testing::ElementHandle;
use i_slint_backend_testing::testing_backend::mock_elapsed_time;
use riina_ui::{
    CatalogRow, PlayerCatalogState, PlayerMusicModeState, PlayerSidebarActions, PlayerSidebarState,
    PlayerWindow, PlaylistDropHover, PlaylistDropViewport, PlaylistInteraction,
};
use slint::{
    ComponentHandle, LogicalPosition, Model, ModelRc, SharedString, VecModel,
    platform::{PointerEventButton, WindowEvent},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

fn advance_drag() {
    for _ in 0..12 {
        mock_elapsed_time(32);
    }
}

#[test]
fn sidebar_playlist_drag_scrolls_and_tracks_a_stationary_pointer_until_release() {
    exercise_drag(false);
}

#[test]
fn music_mode_playlist_drag_scrolls_and_tracks_a_stationary_pointer_until_release() {
    exercise_drag(true);
}

fn playlist_player(music_mode: bool) -> (PlayerWindow, Rc<VecModel<CatalogRow>>) {
    let app = player(if music_mode { 0 } else { 4 });
    if music_mode {
        let music = app.global::<PlayerMusicModeState>();
        music.set_music_mode(true);
        music.set_music_mode_show_playlist(true);
    }
    let rows = Rc::new(VecModel::from(
        (0..70)
            .map(|index| CatalogRow {
                token: index.to_string().into(),
                label: format!("Playlist clip {index}").into(),
                ..Default::default()
            })
            .collect::<Vec<_>>(),
    ));
    app.global::<PlayerCatalogState>()
        .set_playlist(ModelRc::from(rows.clone()));
    // Construct the conditional Music Mode view before reading its published viewport.
    assert!(
        ElementHandle::find_by_accessible_label(&app, "Playlist clip 0")
            .any(|row| row.size().height > 0.0)
    );
    flush();
    mock_elapsed_time(4);
    flush();
    (app, rows)
}

fn viewport(app: &PlayerWindow, music_mode: bool) -> PlaylistDropViewport {
    let sidebar = app.global::<PlayerSidebarState>();
    if music_mode {
        sidebar.get_music_list_drop_viewport()
    } else {
        sidebar.get_trailing_list_drop_viewport()
    }
}

fn exercise_drag(music_mode: bool) {
    let (app, rows) = playlist_player(music_mode);
    let actions = app.global::<PlayerSidebarActions>();
    let selections = Rc::new(Cell::new(0));
    let updates = selections.clone();
    actions.on_select_playlist_row(move |index, _, _| {
        updates.set(updates.get() + 1);
        let index = index as usize;
        let Some(mut row) = rows.row_data(index) else {
            return ModelRc::default();
        };
        row.selected = true;
        let token = row.token.clone();
        rows.set_row_data(index, row);
        ModelRc::new(VecModel::from(vec![token]))
    });
    let drops = Rc::new(RefCell::new(Vec::new()));
    let observed = drops.clone();
    actions.on_reorder_playlist(move |tokens, slot| {
        observed
            .borrow_mut()
            .push((tokens.iter().collect::<Vec<_>>(), slot));
    });
    let sidebar = app.global::<PlayerSidebarState>();
    let interaction = app.global::<PlaylistInteraction>();
    let viewport = || viewport(&app, music_mode);
    let list = viewport();
    assert!(list.width > 60.0 && list.height > 100.0, "{list:?}");
    let bottom = LogicalPosition::new(list.x + 30.0, list.y + list.height - 2.0);
    let top = LogicalPosition::new(list.x + 30.0, list.y + 2.0);
    let start = LogicalPosition::new(list.x + 30.0, list.y + 8.0);
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position: start,
        button: PointerEventButton::Left,
    });
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position: bottom });
    flush();
    assert!(interaction.get_playlist_drag_active());
    let before_scroll = sidebar.get_playlist_reorder_slot();
    advance_drag();
    assert!(viewport().content_y < list.content_y);
    let after_scroll = sidebar.get_playlist_reorder_slot();
    assert!(after_scroll > before_scroll);
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position: bottom,
        button: PointerEventButton::Left,
    });
    flush();
    assert_eq!(selections.get(), 1);
    assert_eq!(
        drops.borrow().as_slice(),
        &[(vec![SharedString::from("0")], after_scroll)]
    );
    assert!(!interaction.get_playlist_drag_active());

    app.window().dispatch_event(WindowEvent::PointerPressed {
        position: LogicalPosition::new(top.x, top.y + 60.0),
        button: PointerEventButton::Left,
    });
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position: top });
    flush();
    let before_up = viewport().content_y;
    advance_drag();
    assert!(viewport().content_y > before_up);

    app.window().dispatch_event(WindowEvent::PointerExited);
    flush();
    let after_cancel = viewport().content_y;
    advance_drag();
    assert_eq!(viewport().content_y, after_cancel);
    assert!(!interaction.get_playlist_drag_active());
    assert_eq!(drops.borrow().len(), 1);
}

#[test]
fn external_drop_hover_scrolls_the_sidebar_and_clears_the_insertion_gap_on_exit() {
    exercise_external_hover(false);
}

#[test]
fn external_drop_hover_scrolls_music_mode_and_clears_the_insertion_gap_on_exit() {
    exercise_external_hover(true);
}

fn exercise_external_hover(music_mode: bool) {
    let (app, rows) = playlist_player(music_mode);
    let state = app.global::<PlayerSidebarState>();
    let interaction = app.global::<PlaylistInteraction>();
    let view = || viewport(&app, music_mode);
    let start = view();
    let hover = |y| {
        interaction.set_external_drop_hover(PlaylistDropHover {
            active: true,
            x: start.x + 30.0,
            y,
        });
        flush();
    };

    hover(start.y + start.height - 2.0);
    let before_scroll = state.get_playlist_insertion_slot();
    assert!(before_scroll >= 0);
    assert_eq!(state.get_playlist_reorder_slot(), -1);
    advance_drag();
    let after_scroll = state.get_playlist_insertion_slot();
    assert!(after_scroll > before_scroll);
    assert!(view().content_y < start.content_y);

    hover(start.y + 2.0);
    let before_up = view().content_y;
    mock_elapsed_time(32);
    assert!(view().content_y > before_up);
    interaction.set_external_drop_hover(PlaylistDropHover::default());
    flush();
    let after_exit = view().content_y;
    advance_drag();
    assert_eq!(view().content_y, after_exit);
    assert_eq!(state.get_playlist_insertion_slot(), -1);

    rows.set_vec(Vec::new());
    flush();
    hover(start.y + 2.0);
    assert_eq!(state.get_playlist_insertion_slot(), 0);
}
