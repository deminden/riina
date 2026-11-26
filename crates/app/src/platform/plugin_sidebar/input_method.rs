//! Share the player window's Wayland input method with the focused browser editor.
mod context;

use crate::PlayerWindow;
pub(super) use context::Context;
use gtk::glib::{self, subclass::prelude::*};
use riina_ui::{PlayerDialogState, PlayerPluginImePhase as Phase};
use slint::{
    ComponentHandle,
    winit_030::{
        WinitWindowAccessor,
        winit::{
            dpi::{LogicalPosition, LogicalSize},
            window::ImePurpose,
        },
    },
};
use std::{
    cell::{Cell, OnceCell, RefCell},
    time::Duration,
};
use webkit2gtk::{InputMethodContext, InputMethodContextExt, InputPurpose, WebViewExt};

#[derive(Default)]
pub struct InputMethod {
    owner: OnceCell<slint::Weak<PlayerWindow>>,
    preedit: RefCell<String>,
    offset: Cell<u32>,
    revision: Cell<u64>,
    active: Cell<bool>,
    caret: Cell<Option<(i32, i32, i32, i32)>>,
    purpose: Cell<Option<ImePurpose>>,
}

impl ObjectImpl for InputMethod {}

impl Context {
    pub(super) fn attach(
        view: &webkit2gtk::WebView,
        owner: slint::Weak<PlayerWindow>,
    ) -> Result<Self, String> {
        use glib::prelude::*;

        let context = glib::Object::new::<Self>();
        let _ = context.imp().owner.set(owner);
        let browser_context = context
            .dynamic_cast_ref::<InputMethodContext>()
            .ok_or("Could not install the sidebar's WebKit input method context")?;
        view.set_input_method_context(Some(browser_context));
        let bound = context.downgrade();
        browser_context.connect_input_purpose_notify(move |_| {
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
            state.update_window(true);
        }
    }

    pub(super) fn update(&self, phase: Phase, text: &str, cursor: i32) {
        use glib::prelude::*;

        let state = self.imp();
        if !state.active.get() || text.contains('\0') {
            return;
        }
        let revision = state.advance();
        match phase {
            Phase::Commit => {
                self.emit_by_name::<()>("committed", &[&text]);
                state.clear();
            }
            Phase::Preedit if !text.is_empty() => {
                state.offset.set(utf16_offset(text, cursor));
                let starting = state.preedit.replace(text.into()).is_empty();
                if starting {
                    self.emit_by_name::<()>("preedit-started", &[]);
                }
                self.emit_by_name::<()>("preedit-changed", &[]);
            }
            Phase::Preedit => {
                // Winit clears preedit immediately before a commit. Keep the
                // composition range until the commit in the same event batch.
                let weak = self.downgrade();
                slint::Timer::single_shot(Duration::ZERO, move || {
                    if let Some(context) = weak.upgrade()
                        && context.imp().revision.get() == revision
                    {
                        context.finish_preedit();
                    }
                });
            }
            Phase::Disabled => self.finish_preedit(),
        }
    }

    fn finish_preedit(&self) {
        use glib::prelude::*;

        if !self.imp().preedit.borrow().is_empty() {
            self.imp().clear();
            self.emit_by_name::<()>("preedit-finished", &[]);
        }
    }
}

impl InputMethod {
    fn advance(&self) -> u64 {
        let revision = self.revision.get().wrapping_add(1);
        self.revision.set(revision);
        revision
    }

    fn clear(&self) {
        self.advance();
        self.preedit.borrow_mut().clear();
        self.offset.set(0);
    }

    fn set_active(&self, active: bool) {
        if self.active.replace(active) == active {
            return;
        }
        if !active {
            self.clear();
            self.caret.set(None);
            self.purpose.set(None);
        }
        self.update_window(active);
    }

    fn set_cursor_area(&self, area: (i32, i32, i32, i32)) {
        self.caret.set(Some(area));
        if self.active.get() {
            self.update_window(true);
        }
    }

    fn update_window(&self, active: bool) {
        use glib::prelude::*;

        let Some(app) = self.owner.get().and_then(slint::Weak::upgrade) else {
            return;
        };
        let dialog = app.global::<PlayerDialogState>();
        if dialog.get_jump_time_dialog_visible() || dialog.get_url_dialog_visible() {
            // Slint owns the input method once its player editor opens.
            return;
        }
        let context = self.obj();
        let purpose = if context
            .dynamic_cast_ref::<InputMethodContext>()
            .is_some_and(|context| context.input_purpose() == InputPurpose::Password)
        {
            ImePurpose::Password
        } else {
            ImePurpose::Normal
        };
        app.window().with_winit_window(|window| {
            if active && window.has_focus() {
                if self.purpose.replace(Some(purpose)) != Some(purpose) {
                    window.set_ime_purpose(purpose);
                }
                window.set_ime_allowed(true);
                let (x, y, width, height) = self.caret.get().unwrap_or((0, 0, 1, 1));
                window.set_ime_cursor_area(
                    LogicalPosition::new(
                        f64::from(app.get_plugin_sidebar_view_x()) + f64::from(x),
                        f64::from(app.get_plugin_sidebar_view_y()) + f64::from(y),
                    ),
                    LogicalSize::new(width.max(1), height.max(1)),
                );
            } else if !active {
                window.set_ime_allowed(false);
                window.set_ime_purpose(ImePurpose::Normal);
            }
        });
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
