//! Let the application reconsider its lifetime after a tool window hides.
use std::{cell::OnceCell, rc::Rc, time::Duration};

#[derive(Default)]
pub(crate) struct CloseObserver(OnceCell<Rc<dyn Fn()>>);

impl CloseObserver {
    pub(crate) fn bind(&self, callback: Rc<dyn Fn()>) {
        let _ = self.0.set(callback);
    }

    pub(crate) fn notify(&self) {
        if let Some(callback) = self.0.get().cloned() {
            // Slint hides a native window after its close callback returns.
            slint::Timer::single_shot(Duration::ZERO, move || callback());
        }
    }
}
