use super::{Entry, Target, client};
use crate::PlayerWindow;
use riina_ui::{PlayerMenuActions, PlayerMenuCatalog, PlayerMenuItem};
use slint::{ComponentHandle, LogicalPosition, PlatformError, SharedString};
use std::rc::Rc;

#[cfg(test)]
mod edit_tests;

pub(crate) fn new_player() -> Result<PlayerWindow, PlatformError> {
    let (app, source) = PlayerMenuCatalog::new_player()?;
    let source = Rc::new(source);
    let window = app.as_weak();
    app.on_open_player_menu(move |family, x, y| {
        let Some(app) = window.upgrade() else {
            return;
        };
        client::dismiss(&app);
        app.global::<PlayerMenuActions>()
            .set_edit_menu_state(crate::platform::text_edit::menu_state(&app));
        app.set_snapshot_menu_shortcuts(true);
        let items = usize::try_from(family)
            .map_or_else(|_| source.items(), |family| source.items_in_family(family));
        app.set_snapshot_menu_shortcuts(false);
        if items.is_empty() {
            app.global::<PlayerMenuActions>()
                .set_edit_menu_state(Default::default());
            return;
        }
        let entries = items.into_iter().map(project).collect();
        let source = source.clone();
        client::show_player_menu(
            &app,
            LogicalPosition::new(x, y),
            family,
            entries,
            move |id| {
                source.activate(id);
            },
        );
    });
    Ok(app)
}

fn project(item: PlayerMenuItem) -> Entry<SharedString> {
    if item.separator {
        return Entry::Separator;
    }
    let target = if item.children.is_empty() {
        Target::Action(item.id)
    } else {
        Target::Submenu(item.children.into_iter().map(project).collect())
    };
    Entry::Item {
        title: item.title.to_string(),
        shortcut: item.shortcut,
        enabled: item.enabled,
        checkable: item.checkable,
        checked: item.checked,
        target,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_ui::{
        PlayerMenuActions, PlaylistPluginPopupState, PlaylistPopupInput, PlaylistPopupPanel,
    };
    use slint::{
        LogicalSize, Model,
        platform::{Key, PointerEventButton, WindowEvent},
    };
    use std::cell::Cell;

    #[test]
    fn compact_bar_keeps_nested_commands_in_a_scrollable_client()
    -> Result<(), Box<dyn std::error::Error>> {
        i_slint_backend_testing::init_no_event_loop();
        let app = new_player()?;
        slint::select_bundled_translation("en")?;
        app.window().set_size(LogicalSize::new(320.0, 180.0));
        app.set_loaded(true);
        let opened = Rc::new(Cell::new(0));
        let calls = opened.clone();
        app.global::<PlayerMenuActions>()
            .on_open_video_filters(move || calls.set(calls.get() + 1));
        app.show()?;
        slint::platform::update_timers_and_animations();
        assert!(
            app.get_compact_menu_visible(),
            "Compact bar stayed hidden with {} captions and client size {:?}",
            app.get_player_menu_labels().row_count(),
            app.window().size(),
        );
        let click = |x, y| {
            let position = LogicalPosition::new(x, y);
            let button = PointerEventButton::Left;
            app.window()
                .dispatch_event(WindowEvent::PointerPressed { position, button });
            app.window()
                .dispatch_event(WindowEvent::PointerReleased { position, button });
            slint::platform::update_timers_and_animations();
        };
        click(28.0, 20.0);
        let popup = app.global::<PlaylistPluginPopupState>();
        let panel = |index| {
            popup
                .get_panels()
                .row_data(index)
                .ok_or("Missing menu panel")
        };
        let choose = |parent: i32, title: &str| -> Result<(), Box<dyn std::error::Error>> {
            let group = panel(usize::try_from(parent)?)?;
            let index = group
                .rows
                .iter()
                .position(|row| row.title == title)
                .ok_or("Missing menu item")?;
            popup.invoke_input(
                PlaylistPopupInput::Choose,
                parent,
                i32::try_from(index)?,
                44.0,
            );
            slint::platform::update_timers_and_animations();
            Ok(())
        };
        let assert_bounds = |panel: &PlaylistPopupPanel| {
            assert!(panel.bounds.x >= 0.0 && panel.bounds.y >= 0.0);
            assert!(panel.bounds.x + panel.bounds.width <= 320.0);
            assert!(panel.bounds.y + panel.bounds.height <= 180.0);
        };
        let top = panel(0)?;
        assert_bounds(&top);
        assert_eq!(top.rows.row_count(), 10);
        assert!(top.content_height > top.bounds.height);
        choose(0, "Video")?;
        let video = panel(1)?;
        assert!(video.back);
        assert_bounds(&video);
        app.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::LeftArrow.into(),
        });
        app.window().dispatch_event(WindowEvent::KeyReleased {
            text: Key::LeftArrow.into(),
        });
        assert_eq!(popup.get_panels().row_count(), 1);
        choose(0, "Video")?;
        let filters = panel(1)?
            .rows
            .iter()
            .find(|row| row.title == "Video Filters…")
            .ok_or("Missing Video Filters")?;
        assert!(!filters.shortcut.is_empty());
        choose(1, "Video Filters…")?;
        assert_eq!(opened.get(), 1);
        assert!(!popup.get_shown());
        click(28.0, 20.0);
        assert!(popup.get_shown());
        app.window().set_size(LogicalSize::new(1080.0, 660.0));
        slint::platform::update_timers_and_animations();
        assert!(!popup.get_shown());
        assert!(!app.get_compact_menu_visible());
        app.hide()?;
        Ok(())
    }

    #[test]
    fn short_wide_player_scrolls_families_and_switches_on_hover()
    -> Result<(), Box<dyn std::error::Error>> {
        use i_slint_backend_testing::ElementHandle;

        i_slint_backend_testing::init_no_event_loop();
        let app = new_player()?;
        slint::select_bundled_translation("en")?;
        app.window().set_size(LogicalSize::new(1070.0, 180.0));
        app.set_loaded(true);
        app.set_saved_video_filter_rows(slint::ModelRc::new(slint::VecModel::from(vec![
            riina_ui::SavedFilterMenuRow {
                label: "Fixture flip".into(),
                selected: true,
                ..Default::default()
            },
        ])));
        let opened = Rc::new(Cell::new(false));
        let observed = opened.clone();
        app.global::<PlayerMenuActions>()
            .on_open_video_filters(move || observed.set(true));
        app.show()?;
        slint::platform::update_timers_and_animations();
        assert!(!app.get_compact_menu_visible());

        let center = |label: &str| {
            let item = ElementHandle::find_by_accessible_label(&app, label).next()?;
            let position = item.absolute_position();
            let size = item.size();
            Some(LogicalPosition::new(
                position.x + size.width / 2.0,
                position.y + size.height / 2.0,
            ))
        };
        let video = center("Video").ok_or("Missing Video family")?;
        let file = center("File").ok_or("Missing File family")?;
        assert!((0.0..40.0).contains(&video.y) && (0.0..40.0).contains(&file.y));
        let click = |position| {
            let button = PointerEventButton::Left;
            app.window()
                .dispatch_event(WindowEvent::PointerPressed { position, button });
            app.window()
                .dispatch_event(WindowEvent::PointerReleased { position, button });
            slint::platform::update_timers_and_animations();
        };
        let press = |key: Key| {
            app.window()
                .dispatch_event(WindowEvent::KeyPressed { text: key.into() });
            app.window()
                .dispatch_event(WindowEvent::KeyReleased { text: key.into() });
            slint::platform::update_timers_and_animations();
        };
        click(video);
        let popup = app.global::<PlaylistPluginPopupState>();
        assert_eq!(popup.get_player_menu_family(), 4);
        let panel = || popup.get_panels().row_data(0).ok_or("Missing menu panel");
        let video_menu = panel()?;
        assert!(video_menu.content_height > video_menu.bounds.height);
        assert!(
            video_menu.bounds.y >= 40.0 && video_menu.bounds.y + video_menu.bounds.height <= 180.0
        );
        press(Key::End);
        let last = panel()?;
        assert!(last.reveal >= last.bounds.height);
        assert!(
            last.rows
                .iter()
                .any(|row| row.highlighted && row.title == "Saved Video Filters")
        );
        press(Key::End);
        assert_ne!(panel()?.reveal_serial, last.reveal_serial);
        press(Key::RightArrow);
        let saved = popup
            .get_panels()
            .row_data(1)
            .ok_or("Missing saved filters submenu")?;
        assert!(saved.bounds.x >= last.bounds.x + last.bounds.width);
        assert!(!saved.back);
        let preset = saved.rows.row_data(0).ok_or("Missing fixture preset")?;
        assert_eq!(preset.title, "Fixture flip");
        assert!(preset.checked);
        press(Key::LeftArrow);
        press(Key::UpArrow);
        press(Key::Return);
        assert!(opened.get());
        assert!(!popup.get_shown());

        click(video);
        app.window()
            .dispatch_event(WindowEvent::PointerMoved { position: file });
        slint::platform::update_timers_and_animations();
        assert_eq!(popup.get_player_menu_family(), 1);
        assert!(
            panel()?
                .rows
                .iter()
                .any(|row| row.title == "Playback History")
        );
        click(file);
        assert!(!popup.get_shown());
        click(video);
        press(Key::Escape);
        assert!(!popup.get_shown());
        app.hide()?;
        Ok(())
    }
}
