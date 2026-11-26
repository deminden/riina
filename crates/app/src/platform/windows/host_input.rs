//! Keyboard intent arriving on the framed owner instead of the controls popup.
use crate::{PlayerWindow, player::player_ui::embedded_input::route_window_key};
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
                if route_window_key(app, event, modifiers) {
                    return EventResult::PreventDefault;
                }
                let text = binding_text(&event.logical_key);
                if controls.invoke_begin_plugin_key(
                    text.clone(),
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
                        framed_key_down(app, text, &event.logical_key, modifiers, event.repeat);
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

fn framed_key_down(
    app: &PlayerWindow,
    text: slint::SharedString,
    key: &Key,
    modifiers: ModifiersState,
    repeat: bool,
) -> bool {
    let controls = app.global::<PlayerControlActions>();
    let menu = app.global::<PlayerMenuActions>();
    if controls.invoke_dispatch_key_binding(
        text,
        modifiers.control_key(),
        modifiers.alt_key(),
        modifiers.shift_key(),
        modifiers.super_key(),
        repeat,
    ) {
        return true;
    }
    if repeat {
        return false;
    }
    if modifiers == ModifiersState::CONTROL
        && matches!(key, Key::Character(text) if text.eq_ignore_ascii_case("h"))
    {
        menu.invoke_hide_riina();
        return true;
    }
    if *key == Key::Named(NamedKey::Escape) && app.get_fullscreen() {
        menu.invoke_toggle_fullscreen();
        return true;
    }
    false
}

fn accepts_commands(crop_editing: bool, dialog_open: bool) -> bool {
    !crop_editing && !dialog_open
}

fn binding_text(key: &Key) -> slint::SharedString {
    match key {
        Key::Character(text) => text.as_str().into(),
        Key::Named(key) => format!("{key:?}").into(),
        _ => slint::SharedString::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_commands_respect_editor_scope() {
        for crop in [false, true] {
            for dialog in [false, true] {
                assert_eq!(accepts_commands(crop, dialog), !crop && !dialog);
            }
        }
    }

    #[test]
    fn framed_shortcuts_use_the_same_key_names_as_the_shared_profile() {
        for text in ["+", "#", "v", "V", "z", "Z"] {
            assert_eq!(binding_text(&Key::Character(text.into())), text);
        }
        for (key, text) in [
            (NamedKey::ArrowLeft, "ArrowLeft"),
            (NamedKey::Enter, "Enter"),
            (NamedKey::Escape, "Escape"),
            (NamedKey::F8, "F8"),
        ] {
            assert_eq!(binding_text(&Key::Named(key)), text);
        }
    }
}
