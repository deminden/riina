//! Connect player-scoped listeners without moving a QuickJS realm onto the UI thread.
#[cfg(test)]
mod tests;

use crate::plugin_runtime::input::{Arguments, Kind, Router};
use crate::{PlayerWindow, key_binding_preferences::plugin_event_key};
use riina_ui::{
    PlayerControlActions, PlayerCropState, PlayerDialogState, PlayerPluginPointerPhase,
};
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};

const MOUSE_CODES: [&str; 3] = ["*mouse", "*rightMouse", "*otherMouse"];

pub(super) fn install(app: &PlayerWindow, router: &Router) {
    router
        .bindings
        .replace(app.get_plugin_input_bindings().to_string());
    let bindings = router.bindings.clone();
    app.on_plugin_input_bindings_changed(move |value| bindings.replace(value.to_string()));

    let actions = app.global::<PlayerControlActions>();
    let position = Rc::new(Cell::new((0.0, 0.0)));
    let cursor = position.clone();
    actions.on_plugin_pointer_location(move |x, y| cursor.set((x, y)));

    let keys = router.clone();
    let location = position.clone();
    let pending = Rc::new(RefCell::new(None));
    let begin_pending = pending.clone();
    actions.on_begin_plugin_key(
        move |text, control, alt, shift, meta, is_repeat, released| {
            begin_pending.take();
            let Some(code) = plugin_event_key(text.as_str(), control, alt, shift, meta) else {
                return false;
            };
            let kind = if released { Kind::KeyUp } else { Kind::KeyDown };
            let (x, y) = location.get();
            let Some(dispatch) = keys.begin(kind, code, Arguments::Key { x, y, is_repeat }) else {
                return false;
            };
            if dispatch.handled(true) {
                return true;
            }
            if released {
                return dispatch.handled(false);
            }
            begin_pending.replace(Some(dispatch));
            false
        },
    );
    actions.on_finish_plugin_key(move || {
        pending
            .take()
            .is_some_and(|dispatch| dispatch.handled(false))
    });

    let pointer = router.clone();
    let player = app.as_weak();
    let state = RefCell::new(MouseState::default());
    actions.on_plugin_pointer(move |phase, code, x, y, clicks| {
        position.set((x, y));
        let mut state = state.borrow_mut();
        if let Some(app) = player.upgrade()
            && (app.global::<PlayerCropState>().get_crop_editing()
                || app.global::<PlayerDialogState>().get_dialog_open())
        {
            *state = MouseState::default();
            return false;
        }
        let Some((kind, index, click_count)) = state.event(phase, code.as_str(), (x, y), clicks)
        else {
            return false;
        };
        let arguments = Arguments::Mouse {
            x,
            y,
            click_count,
            pressure: 0.0,
        };
        pointer
            .begin(kind, MOUSE_CODES[index].to_owned(), arguments)
            .is_some_and(|event| event.handled(true) || event.handled(false))
    });
}

#[derive(Default)]
struct MouseState {
    buttons: [MouseButton; 3],
}

#[derive(Default)]
struct MouseButton {
    last_down: Option<(Instant, (f32, f32))>,
    clicks: u8,
    pressed: bool,
}

impl MouseState {
    fn event(
        &mut self,
        phase: PlayerPluginPointerPhase,
        code: &str,
        position: (f32, f32),
        recognized_clicks: i32,
    ) -> Option<(Kind, usize, u8)> {
        if phase == PlayerPluginPointerPhase::Cancel {
            for button in &mut self.buttons {
                button.pressed = false;
            }
            return None;
        }
        if phase == PlayerPluginPointerPhase::Move {
            return self
                .buttons
                .iter()
                .enumerate()
                .filter(|(_, button)| button.pressed)
                .max_by_key(|(_, button)| button.last_down.map(|(time, _)| time))
                .map(|(index, button)| (Kind::MouseDrag, index, button.clicks));
        }
        let index = MOUSE_CODES.iter().position(|key| *key == code)?;
        let button = &mut self.buttons[index];
        if phase == PlayerPluginPointerPhase::Down {
            button.press(position);
            return Some((Kind::MouseDown, index, button.clicks));
        }
        button.pressed = false;
        let count = match recognized_clicks {
            2 => 2,
            1 => 1,
            _ => button.clicks.max(1),
        };
        Some((Kind::MouseUp, index, count))
    }
}

impl MouseButton {
    fn press(&mut self, position: (f32, f32)) {
        let now = Instant::now();
        let nearby_click = self.last_down.is_some_and(|(previous, (x, y))| {
            now.duration_since(previous) <= crate::platform::double_click_interval()
                && (position.0 - x).abs() <= 4.0
                && (position.1 - y).abs() <= 4.0
        });
        self.clicks = if nearby_click {
            self.clicks.saturating_add(1)
        } else {
            1
        };
        self.last_down = Some((now, position));
        self.pressed = true;
    }
}
