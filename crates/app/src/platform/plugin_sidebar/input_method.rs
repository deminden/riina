//! Share the player window's Wayland input method with the focused browser editor.
mod context;

use crate::PlayerWindow;
use crate::platform::wayland_text_input::{self, Deletion, Edits, Field, Surrounding};
pub(in crate::platform) use context::Context;
use gtk::glib::{self, subclass::prelude::*};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::cell::{Cell, OnceCell, RefCell};
use webkit2gtk::{InputMethodContext, InputMethodContextExt, InputPurpose, WebViewExt};

#[derive(Default)]
pub struct InputMethod {
    owner: OnceCell<slint::Weak<PlayerWindow>>,
    surface: Cell<usize>,
    preedit: RefCell<String>,
    offset: Cell<u32>,
    active: Cell<bool>,
    caret: Cell<Option<(i32, i32, i32, i32)>>,
    surrounding: RefCell<Option<Surrounding>>,
    external_change: Cell<bool>,
    awaiting_edit: Cell<bool>,
}

impl ObjectImpl for InputMethod {}

impl Context {
    pub(super) fn attach(
        view: &webkit2gtk::WebView,
        owner: slint::Weak<PlayerWindow>,
        surface: usize,
    ) -> Result<Self, String> {
        use glib::prelude::*;

        let context = glib::Object::new::<Self>();
        let _ = context.imp().owner.set(owner);
        context.imp().surface.set(surface);
        let browser_context = context
            .dynamic_cast_ref::<InputMethodContext>()
            .ok_or("Could not install the sidebar's WebKit input method context")?;
        view.set_input_method_context(Some(browser_context));
        let bound = context.downgrade();
        browser_context.connect_input_purpose_notify(move |_| {
            if let Some(context) = bound.upgrade() {
                context.imp().surrounding.take();
                context.update_position();
            }
        });
        let bound = context.downgrade();
        browser_context.connect_input_hints_notify(move |_| {
            if let Some(context) = bound.upgrade() {
                context.update_position();
            }
        });
        Ok(context)
    }

    pub(super) fn release(&self) {
        self.imp().set_active(false);
    }

    pub(super) fn update_position(&self) {
        let state = self.imp();
        if state.active.get() {
            state.publish();
        }
    }

    pub(super) fn external_edit(&self) {
        self.imp().external_change.set(true);
    }

    pub(in crate::platform) fn apply(&self, edits: Edits, deletion: Option<Deletion>) {
        use glib::prelude::*;

        let state = self.imp();
        if !state.active.get() || (deletion.is_some() && state.awaiting_edit.get()) {
            return;
        }
        state.external_change.set(false);
        if let Some(deletion) = deletion {
            self.finish_preedit();
            self.emit_by_name::<()>("delete-surrounding", &[&deletion.offset, &deletion.length]);
            state.awaiting_edit.set(true);
        }
        if let Some(text) = edits.commit.filter(|text| !text.contains('\0')) {
            self.emit_by_name::<()>("committed", &[&text]);
            state.clear_preedit();
            state.awaiting_edit.set(true);
        }
        if let Some(preedit) = edits.preedit.filter(|preedit| !preedit.text.is_empty()) {
            state.offset.set(utf16_offset(&preedit.text, preedit.end));
            let starting = state.preedit.replace(preedit.text).is_empty();
            if starting {
                self.emit_by_name::<()>("preedit-started", &[]);
            }
            self.emit_by_name::<()>("preedit-changed", &[]);
        } else {
            self.finish_preedit();
        }
    }

    fn finish_preedit(&self) {
        use glib::prelude::*;

        if !self.imp().preedit.borrow().is_empty() {
            self.imp().clear_preedit();
            self.emit_by_name::<()>("preedit-finished", &[]);
        }
    }
}

impl InputMethod {
    fn clear_preedit(&self) {
        self.preedit.borrow_mut().clear();
        self.offset.set(0);
    }

    fn reset(&self) {
        let composing = !self.preedit.borrow().is_empty();
        self.clear_preedit();
        self.awaiting_edit.set(false);
        if composing && self.active.get() {
            wayland_text_input::set_editor(self.surface.get(), &self.obj(), None);
            self.publish();
        }
    }

    fn set_active(&self, active: bool) {
        if self.active.replace(active) == active {
            return;
        }
        if !active {
            self.clear_preedit();
            self.caret.set(None);
            self.surrounding.take();
            self.awaiting_edit.set(false);
            wayland_text_input::set_editor(self.surface.get(), &self.obj(), None);
        } else {
            self.external_change.set(true);
            if let Some(app) = self.owner.get().and_then(slint::Weak::upgrade)
                && super::sidebar_can_show(&app)
            {
                // Browser editing uses its own text input object. Slint keeps
                // Winit's object for the player's own editable controls.
                app.window()
                    .with_winit_window(|window| window.set_ime_allowed(false));
            }
            self.publish();
        }
    }

    fn set_cursor_area(&self, area: (i32, i32, i32, i32)) {
        self.caret.set(Some(area));
        if self.active.get() {
            self.publish();
        }
    }

    fn set_surrounding(&self, text: &str, cursor: usize, anchor: usize) {
        use glib::prelude::*;

        let context = self.obj();
        let Some(browser) = context.dynamic_cast_ref::<InputMethodContext>() else {
            return;
        };
        self.awaiting_edit.set(false);
        if !self.active.get()
            || browser.input_purpose() == InputPurpose::Password
            || !context::supports_surrounding_edits()
        {
            return;
        }
        self.surrounding
            .replace(Surrounding::new(text, cursor, anchor));
        self.publish();
    }

    fn publish(&self) {
        use glib::prelude::*;

        let Some(app) = self.owner.get().and_then(slint::Weak::upgrade) else {
            return;
        };
        if !super::sidebar_can_show(&app)
            || !app
                .window()
                .with_winit_window(|window| window.has_focus())
                .unwrap_or(false)
        {
            return;
        }
        let context = self.obj();
        let Some(browser) = context.dynamic_cast_ref::<InputMethodContext>() else {
            return;
        };
        let (x, y, width, height) = self.caret.get().unwrap_or((0, 0, 1, 1));
        let purpose = browser.input_purpose();
        let field = Field {
            caret: (
                (app.get_plugin_sidebar_view_x().round() as i32).saturating_add(x),
                (app.get_plugin_sidebar_view_y().round() as i32).saturating_add(y),
                width.max(1),
                height.max(1),
            ),
            surrounding: (purpose != InputPurpose::Password)
                .then(|| self.surrounding.borrow().clone())
                .flatten(),
            external_change: self.external_change.get(),
            allows_surrounding: context::supports_surrounding_edits(),
            purpose,
            hints: browser.input_hints(),
        };
        wayland_text_input::set_editor(self.surface.get(), &context, Some(field));
    }
}

fn utf16_offset(text: &str, cursor: i32) -> u32 {
    let byte = usize::try_from(cursor)
        .unwrap_or(text.len())
        .min(text.len());
    text[..text.floor_char_boundary(byte)]
        .encode_utf16()
        .count() as u32
}

#[cfg(test)]
mod tests {
    use super::utf16_offset;

    #[test]
    fn composer_offsets_follow_utf16_units_at_valid_boundaries() {
        assert_eq!(utf16_offset("仮🪷名", 3), 1);
        assert_eq!(utf16_offset("仮🪷名", 5), 1);
        assert_eq!(utf16_offset("仮🪷名", 7), 3);
        assert_eq!(utf16_offset("仮🪷名", -1), 4);
    }
}
