use super::{Entry, Target, client};
use crate::PlayerWindow;
use riina_ui::{CompactPlayerMenu, PlayerMenuItem};
use slint::{ComponentHandle, LogicalPosition, PlatformError, SharedString};
use std::rc::Rc;

pub(crate) fn new_player() -> Result<PlayerWindow, PlatformError> {
    let (app, source) = CompactPlayerMenu::new_player()?;
    let source = Rc::new(source);
    let window = app.as_weak();
    app.on_open_compact_menu(move |x, y| {
        let Some(app) = window
            .upgrade()
            .filter(PlayerWindow::get_compact_menu_visible)
        else {
            return;
        };
        client::dismiss(&app);
        let entries = source.items().into_iter().map(project).collect();
        let source = source.clone();
        client::show_compact(&app, LogicalPosition::new(x, y), entries, move |id| {
            source.activate(id);
        });
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
            app.get_compact_menu_labels().row_count(),
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
}
