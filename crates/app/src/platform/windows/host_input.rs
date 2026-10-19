//! Keyboard intent arriving on the framed owner instead of the controls popup.
use crate::PlayerWindow;
use slint::winit_030::{
    EventResult,
    winit::{
        event::{ElementState, WindowEvent},
        keyboard::{Key, ModifiersState, NamedKey},
    },
};
use std::cell::Cell;

#[derive(Default)]
pub(super) struct HostInput {
    modifiers: Cell<ModifiersState>,
}

impl HostInput {
    pub(super) fn handle(&self, app: &PlayerWindow, event: &WindowEvent) -> EventResult {
        match event {
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers.set(modifiers.state()),
            WindowEvent::Focused(false) => self.modifiers.set(ModifiersState::empty()),
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed
                    && !event.repeat
                    && event.logical_key == Key::Named(NamedKey::Escape)
                    && fullscreen_input_scope(
                        app.get_fullscreen(),
                        app.get_crop_editing(),
                        app.get_dialog_open(),
                    ) =>
            {
                let modifiers = self.modifiers.get();
                // The host has no controls FocusScope. Honor the same custom
                // binding before the shared player's default Escape fallback.
                dispatch_escape(
                    || {
                        app.invoke_dispatch_key_binding(
                            "Escape".into(),
                            modifiers.control_key(),
                            modifiers.alt_key(),
                            modifiers.shift_key(),
                            modifiers.super_key(),
                        )
                    },
                    || app.invoke_toggle_fullscreen(),
                );
                return EventResult::PreventDefault;
            }
            _ => {}
        }
        EventResult::Propagate
    }
}

fn fullscreen_input_scope(fullscreen: bool, crop_editing: bool, dialog_open: bool) -> bool {
    fullscreen && !crop_editing && !dialog_open
}

fn dispatch_escape(binding: impl FnOnce() -> bool, fallback: impl FnOnce()) {
    if !binding() {
        fallback();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_escape_preserves_editor_scope_and_custom_bindings() {
        for fullscreen in [false, true] {
            for crop in [false, true] {
                for dialog in [false, true] {
                    assert_eq!(
                        fullscreen_input_scope(fullscreen, crop, dialog),
                        (fullscreen, crop, dialog) == (true, false, false),
                    );
                }
            }
        }
        for handled in [false, true] {
            let bindings = Cell::new(0);
            let exits = Cell::new(0);
            dispatch_escape(
                || {
                    bindings.set(bindings.get() + 1);
                    handled
                },
                || exits.set(exits.get() + 1),
            );
            assert_eq!(bindings.get(), 1);
            assert_eq!(exits.get(), u32::from(!handled));
        }
    }
}
