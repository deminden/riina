//! Crop editing temporarily owns the window's standard controls.
use crate::{NativeVideoHost, PlayerWindow};
use slint::{
    ComponentHandle,
    winit_030::{WinitWindowAccessor, winit::window::WindowButtons},
};
use std::cell::Cell;

fn update(window: &slint::Window, saved: &Cell<Option<WindowButtons>>, editing: bool) {
    window.with_winit_window(|window| {
        if editing && saved.get().is_none() {
            // Retain the prior mask: leaving crop mode must not enable a control
            // that another window policy had already disabled.
            saved.set(Some(window.enabled_buttons()));
            window.set_enabled_buttons(WindowButtons::empty());
        } else if !editing && let Some(buttons) = saved.take() {
            window.set_enabled_buttons(buttons);
        }
    });
}

pub(crate) fn install(app: &PlayerWindow, host: Option<slint::Weak<NativeVideoHost>>) {
    let weak = app.as_weak();
    let app_buttons = Cell::new(None);
    let host_buttons = Cell::new(None);
    app.on_crop_window_controls_changed(move || {
        if let Some(app) = weak.upgrade() {
            let editing = app.get_crop_editing();
            update(app.window(), &app_buttons, editing);
            if let Some(host) = host.as_ref().and_then(slint::Weak::upgrade) {
                update(host.window(), &host_buttons, editing);
            }
        }
    });
}
