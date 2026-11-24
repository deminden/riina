//! Route application menu edits to the focused Slint text item.
use slint::private_unstable_api::re_exports::{
    ItemRc, ItemRef, TextChangeNotify, TextInput, WindowAdapter, WindowInner,
};
use std::{
    pin::Pin,
    rc::{Rc, Weak},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Edit {
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Delete,
    SelectAll,
}

#[derive(Clone)]
pub(super) struct Editor(Weak<dyn WindowAdapter>);

impl Editor {
    pub(super) fn new(window: &slint::Window) -> Self {
        Self(Rc::downgrade(
            &WindowInner::from_pub(window).window_adapter(),
        ))
    }

    pub(super) fn is_live(&self) -> bool {
        self.0.strong_count() != 0
    }

    #[cfg(target_os = "macos")]
    pub(super) fn has_text_focus(&self) -> bool {
        self.0
            .upgrade()
            .is_some_and(|adapter| WindowInner::from_pub(adapter.window()).text_input_focused())
    }

    pub(super) fn perform(&self, edit: Edit, apply: bool) -> bool {
        let Some(adapter) = self.0.upgrade() else {
            return false;
        };
        let window = WindowInner::from_pub(adapter.window());
        if !window.text_input_focused() {
            return false;
        }
        let Some(item) = window
            .try_component()
            .and_then(|root| focused(ItemRc::new_root(root)))
        else {
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

fn focused(item: ItemRc) -> Option<ItemRc> {
    if ItemRef::downcast_pin::<TextInput>(item.borrow()).is_some_and(|input| {
        TextInput::FIELD_OFFSETS.has_focus().apply_pin(input).get()
            && TextInput::FIELD_OFFSETS.enabled().apply_pin(input).get()
    }) && item.is_visible()
    {
        return Some(item);
    }
    let mut child = item.first_child();
    while let Some(next) = child {
        child = next.next_sibling();
        if let Some(input) = focused(next) {
            return Some(input);
        }
    }
    None
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
