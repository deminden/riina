//! Pass the timeline cache to native consumers without a second decoder.
use crate::thumbnail_preview::Controller;
use std::{cell::RefCell, rc::Rc};

#[cfg(target_os = "macos")]
type Redraw = Box<dyn Fn()>;

#[derive(Clone)]
pub(crate) struct Samples {
    #[cfg(target_os = "macos")]
    pub(super) controller: Rc<RefCell<Controller>>,
    #[cfg(target_os = "macos")]
    redraw: Rc<RefCell<Option<Redraw>>>,
}

impl Samples {
    pub(crate) fn new(controller: &Rc<RefCell<Controller>>) -> Self {
        #[cfg(not(target_os = "macos"))]
        let _ = controller;
        Self {
            #[cfg(target_os = "macos")]
            controller: controller.clone(),
            #[cfg(target_os = "macos")]
            redraw: Rc::default(),
        }
    }

    #[cfg(target_os = "macos")]
    pub(super) fn on_redraw(&self, redraw: impl Fn() + 'static) {
        self.redraw.replace(Some(Box::new(redraw)));
    }

    pub(crate) fn changed(&self) {
        #[cfg(target_os = "macos")]
        if let Some(redraw) = self.redraw.borrow().as_ref() {
            redraw();
        }
    }
}
