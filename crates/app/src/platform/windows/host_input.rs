//! Keyboard intent arriving on the framed owner instead of the controls popup.
use crate::PlayerWindow;
use riina_ui::{PlayerControlActions, PlayerMenuActions};
use slint::ComponentHandle;
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
                if event.logical_key != Key::Named(NamedKey::F10)
                    && accepts_commands(
                        app.global::<riina_ui::PlayerCropState>().get_crop_editing(),
                        app.global::<riina_ui::PlayerDialogState>()
                            .get_dialog_open(),
                    ) =>
            {
                let modifiers = self.modifiers.get();
                let controls = app.global::<PlayerControlActions>();
                let released = event.state == ElementState::Released;
                let text = match &event.logical_key {
                    Key::Character(text) => text.as_str().into(),
                    Key::Named(key) => format!("{key:?}").into(),
                    _ => slint::SharedString::default(),
                };
                if controls.invoke_begin_plugin_key(
                    text,
                    modifiers.control_key(),
                    modifiers.alt_key(),
                    modifiers.shift_key(),
                    modifiers.super_key(),
                    event.repeat,
                    released,
                ) {
                    return EventResult::PreventDefault;
                }
                if !released {
                    let mapped =
                        !event.repeat && framed_key_down(app, &event.logical_key, modifiers);
                    let handled = controls.invoke_finish_plugin_key();
                    if mapped || handled {
                        return EventResult::PreventDefault;
                    }
                }
            }
            _ => {}
        }
        EventResult::Propagate
    }
}

fn framed_key_down(app: &PlayerWindow, key: &Key, modifiers: ModifiersState) -> bool {
    let controls = app.global::<PlayerControlActions>();
    let menu = app.global::<PlayerMenuActions>();
    if let Some(text) = control_binding(key, modifiers)
        && controls.invoke_dispatch_key_binding(
            text,
            true,
            modifiers.alt_key(),
            modifiers.shift_key(),
            false,
        )
    {
        return true;
    }
    if modifiers == ModifiersState::CONTROL
        && matches!(key, Key::Character(text) if text.eq_ignore_ascii_case("h"))
    {
        menu.invoke_hide_riina();
        return true;
    }
    if *key == Key::Named(NamedKey::Escape) && app.get_fullscreen() {
        dispatch_escape(
            || {
                controls.invoke_dispatch_key_binding(
                    "Escape".into(),
                    modifiers.control_key(),
                    modifiers.alt_key(),
                    modifiers.shift_key(),
                    modifiers.super_key(),
                )
            },
            || menu.invoke_toggle_fullscreen(),
        );
        return true;
    }
    false
}

fn accepts_commands(crop_editing: bool, dialog_open: bool) -> bool {
    !crop_editing && !dialog_open
}

fn control_binding(key: &Key, modifiers: ModifiersState) -> Option<slint::SharedString> {
    if !modifiers.control_key() || modifiers.super_key() {
        return None;
    }
    match key {
        Key::Character(text) => Some(text.as_str().into()),
        Key::Named(NamedKey::ArrowRight) => Some(slint::platform::Key::RightArrow.into()),
        Key::Named(NamedKey::ArrowLeft) => Some(slint::platform::Key::LeftArrow.into()),
        Key::Named(NamedKey::ArrowUp) => Some(slint::platform::Key::UpArrow.into()),
        Key::Named(NamedKey::ArrowDown) => Some(slint::platform::Key::DownArrow.into()),
        _ => None,
    }
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
    fn host_commands_preserve_editor_scope_and_escape_uses_custom_bindings() {
        for crop in [false, true] {
            for dialog in [false, true] {
                assert_eq!(accepts_commands(crop, dialog), !crop && !dialog);
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

    #[test]
    fn framed_control_bindings_preserve_shift_and_leave_system_menu_keys_native() {
        for text in ["0", "1", "2", "3", "=", "-", "v", "V", "a", "s"] {
            let key = Key::Character(text.into());
            for modifiers in [
                ModifiersState::CONTROL,
                ModifiersState::CONTROL | ModifiersState::SHIFT,
                ModifiersState::CONTROL | ModifiersState::ALT,
            ] {
                assert_eq!(control_binding(&key, modifiers).as_deref(), Some(text));
            }
            for modifiers in [
                ModifiersState::empty(),
                ModifiersState::ALT,
                ModifiersState::CONTROL | ModifiersState::SUPER,
            ] {
                assert_eq!(control_binding(&key, modifiers), None);
            }
        }
        assert_eq!(
            control_binding(&Key::Named(NamedKey::Escape), ModifiersState::CONTROL),
            None
        );
        assert_eq!(
            control_binding(&Key::Named(NamedKey::F10), ModifiersState::CONTROL),
            None
        );
    }
}
