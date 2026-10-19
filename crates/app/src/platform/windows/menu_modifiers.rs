//! Reconcile the modifier release consumed by a native Windows menu loop.
use slint::platform::{Key, WindowEvent};

pub(super) fn reconcile_alt(window: &slint::Window, physically_held: bool) {
    if !physically_held {
        // Slint tracks modifiers through key events. Windows can consume Alt-up
        // in its modal menu loop without deactivating the controls window.
        // This is an internal release, not injected system keyboard input.
        window.dispatch_event(WindowEvent::KeyReleased {
            text: Key::Alt.into(),
        });
    }
}
