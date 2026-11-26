//! Connect player-scoped listeners without moving a QuickJS realm onto the UI thread.
mod pointer;
#[cfg(test)]
mod tests;

use crate::plugin_runtime::input::{Arguments, Kind, Router};
use crate::{PlayerWindow, key_binding_preferences::plugin_event_key};
use riina_ui::{PlayerControlActions, PlayerCropState, PlayerDialogState};
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub(in crate::player::player_ui) use pointer::PointerInput;

pub(super) fn install(app: &PlayerWindow, router: &Router) -> PointerInput {
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
    let input = PointerInput::default();
    let state = input.clone();
    actions.on_plugin_pointer(move |phase, code, x, y, clicks| {
        position.set((x, y));
        if let Some(app) = player.upgrade()
            && (app.global::<PlayerCropState>().get_crop_editing()
                || app.global::<PlayerDialogState>().get_dialog_open())
        {
            state.reset();
            return false;
        }
        let Some((kind, code, arguments)) = state.event(phase, code.as_str(), (x, y), clicks)
        else {
            return false;
        };
        pointer
            .begin(kind, code.to_owned(), arguments)
            .is_some_and(|event| event.handled(true) || event.handled(false))
    });
    input
}
