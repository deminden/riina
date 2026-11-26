//! Route application menu edits to the focused Slint text item.
use crate::PlayerWindow;
#[cfg(any(target_os = "linux", target_os = "windows", test))]
use riina_ui::PlayerEditMenuState;
use riina_ui::{PlayerMenuActions, PlaylistEdit, PlaylistInteraction};
use slint::ComponentHandle;
use slint::private_unstable_api::re_exports::{
    ItemRc, ItemRef, TextChangeNotify, TextInput, WindowAdapter, WindowInner,
};
use std::{
    pin::Pin,
    rc::{Rc, Weak},
};

pub(super) use riina_ui::PlayerEdit as Edit;

mod popup_focus;
#[cfg(target_os = "windows")]
mod windows;

pub(super) use popup_focus::PopupFocus;

pub(crate) fn install_player_menu(app: &PlayerWindow) {
    let editor = Editor::new(app.window());
    let owner = app.as_weak();
    app.global::<PlayerMenuActions>().on_edit(move |edit| {
        if let Some(app) = owner.upgrade() {
            editor.player_edit(&app, edit, true);
        }
    });
    #[cfg(target_os = "windows")]
    if !app.get_native_video() {
        windows::track_shared_player(app);
    }
}

#[cfg(any(target_os = "linux", target_os = "windows", test))]
pub(super) fn menu_state(app: &PlayerWindow) -> PlayerEditMenuState {
    let editor = Editor::new(app.window());
    let enabled = |edit| editor.player_edit(app, edit, false);
    PlayerEditMenuState {
        undo: enabled(Edit::Undo),
        redo: enabled(Edit::Redo),
        cut: enabled(Edit::Cut),
        copy: enabled(Edit::Copy),
        paste: enabled(Edit::Paste),
        delete: enabled(Edit::Delete),
        select_all: enabled(Edit::SelectAll),
    }
}

#[derive(Clone)]
pub(super) struct Editor(Weak<dyn WindowAdapter>);

impl Editor {
    pub(super) fn new(window: &slint::Window) -> Self {
        Self(Rc::downgrade(
            &WindowInner::from_pub(window).window_adapter(),
        ))
    }

    #[cfg(any(target_os = "macos", test))]
    pub(super) fn is_live(&self) -> bool {
        self.0.strong_count() != 0
    }

    pub(super) fn has_text_focus(&self) -> bool {
        self.0
            .upgrade()
            .and_then(|adapter| focused_text(&adapter))
            .is_some()
    }

    pub(super) fn player_edit(&self, app: &PlayerWindow, edit: Edit, apply: bool) -> bool {
        if self.has_text_focus() {
            return self.perform(edit, apply);
        }
        let edit = match edit {
            Edit::Cut => PlaylistEdit::Cut,
            Edit::Copy => PlaylistEdit::Copy,
            Edit::Paste => PlaylistEdit::Paste,
            Edit::Delete => PlaylistEdit::Remove,
            Edit::SelectAll => PlaylistEdit::SelectAll,
            Edit::Undo | Edit::Redo => return false,
        };
        app.global::<PlaylistInteraction>().invoke_edit(edit, apply)
    }

    pub(super) fn perform(&self, edit: Edit, apply: bool) -> bool {
        let Some(adapter) = self.0.upgrade() else {
            return false;
        };
        let Some(item) = focused_text(&adapter) else {
            return false;
        };
        let Some(input) = ItemRef::downcast_pin::<TextInput>(item.borrow()) else {
            return false;
        };
        if !can_edit(input, edit) {
            return false;
        }
        if apply {
            match edit {
                Edit::Undo => input.undo(&adapter, &item),
                Edit::Redo => input.redo(&adapter, &item),
                Edit::Cut => input.cut(&adapter, &item),
                Edit::Copy => input.copy(&adapter, &item),
                Edit::Paste => input.paste(&adapter, &item),
                Edit::Delete => {
                    input.delete_selection(&adapter, &item, TextChangeNotify::TriggerCallbacks);
                }
                Edit::SelectAll => input.select_all(&adapter, &item),
            }
        }
        true
    }
}

pub(super) fn focused_text(adapter: &Rc<dyn WindowAdapter>) -> Option<ItemRc> {
    // The Windows framed owner can take native focus while the controls retain
    // their focused item and selection for the duration of the menu.
    WindowInner::from_pub(adapter.window())
        .focus_item
        .borrow()
        .upgrade()
        .filter(|item| {
            item.is_visible()
                && ItemRef::downcast_pin::<TextInput>(item.borrow())
                    .is_some_and(|input| TextInput::FIELD_OFFSETS.enabled().apply_pin(input).get())
        })
}

fn can_edit(input: Pin<&TextInput>, edit: Edit) -> bool {
    let read_only = TextInput::FIELD_OFFSETS.read_only().apply_pin(input).get();
    let (anchor, cursor) = input.selection_anchor_and_cursor();
    match edit {
        Edit::SelectAll => true,
        Edit::Copy => anchor != cursor,
        Edit::Cut | Edit::Delete => !read_only && anchor != cursor,
        Edit::Paste | Edit::Undo | Edit::Redo => !read_only,
    }
}

#[cfg(test)]
mod tests;
