//! Route outline selection through stable history references.
use super::{PlaybackHistoryWindow, read_selected_paths, write_selected_paths};
use riina_ui::PlaybackHistoryNavigation as Navigation;
use slint::{ComponentHandle, Model, SharedString};
use std::{cell::RefCell, collections::HashSet, rc::Rc};

pub(super) type Anchor = Rc<RefCell<Option<SharedString>>>;

pub(super) fn connect(window: &PlaybackHistoryWindow, anchor: &Anchor) {
    let weak = window.as_weak();
    let pointer_anchor = anchor.clone();
    window.on_select_entry(move |path, index, extend, range| {
        let (Some(window), Ok(index)) = (weak.upgrade(), usize::try_from(index)) else {
            return;
        };
        if !window.get_confirmation_visible()
            && window
                .get_rows()
                .row_data(index)
                .is_some_and(|row| !row.is_group && row.path == path)
        {
            select(&window, &pointer_anchor, path, index, extend, range);
        }
    });

    let weak = window.as_weak();
    let navigation_anchor = anchor.clone();
    window.on_navigate(move |direction, range, page| {
        let Some(window) = weak.upgrade() else { return };
        if !window.get_confirmation_visible() {
            navigate(&window, &navigation_anchor, direction, range, page);
        }
    });

    let weak = window.as_weak();
    let key_anchor = anchor.clone();
    window.on_select_all(move || {
        let Some(window) = weak.upgrade() else { return };
        if window.get_confirmation_visible() {
            return;
        }
        let rows = window.get_rows();
        let first = rows.iter().find(|row| !row.is_group).map(|row| row.path);
        window.set_selected_path(first.clone().unwrap_or_default());
        *key_anchor.borrow_mut() = first;
        let selected = rows
            .iter()
            .filter(|row| !row.is_group)
            .map(|row| row.path.to_string());
        write_selected_paths(&window, &selected.collect());
    });
}

fn select(
    window: &PlaybackHistoryWindow,
    anchor: &Anchor,
    path: SharedString,
    index: usize,
    extend: bool,
    range: bool,
) {
    let rows = window.get_rows();
    let mut selected = if extend {
        read_selected_paths(window)
    } else {
        HashSet::new()
    };
    let start = if range {
        anchor.borrow().as_ref().and_then(|reference| {
            rows.iter()
                .position(|row| !row.is_group && row.path == *reference)
        })
    } else {
        None
    };
    if let Some(start) = start {
        let start_index = start.min(index);
        let end_index = start.max(index);
        selected.extend((start_index..=end_index).filter_map(|index| {
            rows.row_data(index)
                .filter(|row| !row.is_group)
                .map(|row| row.path.to_string())
        }));
    } else {
        if !extend || range || !selected.remove(path.as_str()) {
            selected.insert(path.to_string());
        }
        *anchor.borrow_mut() = Some(path.clone());
    }
    window.set_selected_path(path);
    write_selected_paths(window, &selected);
}

fn navigate(
    window: &PlaybackHistoryWindow,
    anchor: &Anchor,
    direction: Navigation,
    range: bool,
    page: i32,
) {
    let rows: Vec<_> = window
        .get_rows()
        .iter()
        .enumerate()
        .filter(|(_, row)| !row.is_group)
        .map(|(index, row)| (index, row.path))
        .collect();
    let Some(first) = rows.first() else { return };
    let last = rows.last().unwrap_or(first);
    let current_path = window.get_selected_path();
    let current = rows.iter().position(|(_, path)| *path == current_path);
    let (index, path) = match (direction, current) {
        (Navigation::First | Navigation::Next | Navigation::PageDown, None)
        | (Navigation::First, _) => first,
        (Navigation::Last | Navigation::Previous | Navigation::PageUp, None)
        | (Navigation::Last, _) => last,
        (Navigation::Previous, Some(position)) => &rows[position.saturating_sub(1)],
        (Navigation::Next, Some(position)) => &rows[(position + 1).min(rows.len() - 1)],
        (Navigation::PageUp, Some(position)) => {
            let target = rows[position].0.saturating_sub(page.max(1) as usize);
            rows.iter()
                .rev()
                .find(|(index, _)| *index <= target)
                .unwrap_or(first)
        }
        (Navigation::PageDown, Some(position)) => {
            let target = rows[position].0.saturating_add(page.max(1) as usize);
            rows.iter()
                .find(|(index, _)| *index >= target)
                .unwrap_or(last)
        }
    };
    select(window, anchor, path.clone(), *index, false, range);
    if let Ok(index) = i32::try_from(*index) {
        window.invoke_reveal_row(index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PlaybackHistoryRow;
    use i_slint_backend_testing::{AccessibleRole, ElementHandle};
    use riina_ui::PlaybackHistoryAction;
    use slint::{
        ModelRc, VecModel,
        platform::{Key, WindowEvent},
    };

    #[test]
    fn keyboard_range_uses_the_original_path_after_rows_shift() -> Result<(), slint::PlatformError>
    {
        i_slint_backend_testing::init_no_event_loop();
        let window = PlaybackHistoryWindow::new()?;
        let anchor = Anchor::default();
        connect(&window, &anchor);
        let row = |path: &str, available| PlaybackHistoryRow {
            path: path.into(),
            label: path.into(),
            available,
            local_file: available,
            ..Default::default()
        };
        let group = || PlaybackHistoryRow {
            is_group: true,
            ..Default::default()
        };
        window.set_rows(ModelRc::new(VecModel::from(vec![
            group(),
            row("one", true),
            group(),
            row("two", false),
            row("three", true),
        ])));
        window.invoke_navigate(Navigation::First, false, 4);
        window.invoke_navigate(Navigation::Next, false, 4);
        assert_eq!(read_selected_paths(&window), HashSet::from(["two".into()]));
        assert!(!window.get_has_playable_selection());

        window.set_rows(ModelRc::new(VecModel::from(vec![
            group(),
            row("new", true),
            row("one", true),
            group(),
            row("two", false),
            row("three", true),
        ])));
        window.invoke_navigate(Navigation::Next, true, 4);
        assert_eq!(
            read_selected_paths(&window),
            ["two".into(), "three".into()].into()
        );
        assert!(window.get_has_playable_selection() && window.get_has_local_selection());
        assert_eq!(window.get_selected_menu_row(), 5);
        let model = window.get_rows();
        let mut updated = model.iter().collect::<Vec<_>>();
        updated[5].progress = "00:13".into();
        super::super::publish_rows(&window, updated);
        assert_eq!(model.row_data(5).unwrap().progress, "00:13");
        assert!(model.row_data(5).unwrap().selected);
        window.invoke_navigate(Navigation::PageUp, false, 4);
        assert_eq!(window.get_selected_path(), "new");
        assert_eq!(window.get_selected_menu_row(), 1);

        window.show()?;
        ElementHandle::find_by_accessible_label(&window, "three")
            .find(|row| row.accessible_role() == Some(AccessibleRole::ListItem))
            .unwrap()
            .invoke_accessible_default_action();
        assert_eq!(window.get_selected_path(), "three");
        let observed = Rc::new(RefCell::new(Vec::new()));
        let capture = observed.clone();
        window
            .on_context_action(move |action, clicked| capture.borrow_mut().push((action, clicked)));
        for (key, action) in [
            (Key::Return, PlaybackHistoryAction::Play),
            (Key::Delete, PlaybackHistoryAction::Remove),
        ] {
            let text: SharedString = key.into();
            window
                .window()
                .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
            window
                .window()
                .dispatch_event(WindowEvent::KeyPressRepeated { text: text.clone() });
            window
                .window()
                .dispatch_event(WindowEvent::KeyReleased { text });
            assert_eq!(
                observed.borrow().as_slice(),
                &[(action, SharedString::default())]
            );
            observed.borrow_mut().clear();
        }
        window.hide()?;
        Ok(())
    }
}
