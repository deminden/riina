use slint::private_unstable_api::re_exports::{
    FocusReason, ItemRef, ItemWeak, TextInput, WindowInner,
};

pub(in crate::platform) struct PopupFocus {
    item: ItemWeak,
    selection: Option<(i32, i32)>,
}

impl PopupFocus {
    pub(in crate::platform) fn capture(window: &slint::Window) -> Option<Self> {
        let item = WindowInner::from_pub(window)
            .focus_item
            .borrow()
            .upgrade()?;
        let selection = ItemRef::downcast_pin::<TextInput>(item.borrow()).and_then(|input| {
            let (anchor, cursor) = input.selection_anchor_and_cursor();
            Some((i32::try_from(anchor).ok()?, i32::try_from(cursor).ok()?))
        });
        Some(Self {
            item: item.downgrade(),
            selection,
        })
    }

    pub(in crate::platform) fn restore(self, window: &slint::Window) -> bool {
        let Some(item) = self.item.upgrade().filter(|item| item.is_visible()) else {
            return false;
        };
        let inner = WindowInner::from_pub(window);
        inner.set_focus_item(&item, true, FocusReason::PopupActivation);
        if let Some((anchor, cursor)) = self.selection
            && let Some(input) = ItemRef::downcast_pin::<TextInput>(item.borrow())
        {
            input.set_selection_offsets(&inner.window_adapter(), &item, anchor, cursor);
        }
        true
    }
}
