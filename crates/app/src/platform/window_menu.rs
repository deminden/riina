//! AppKit owns its Window menu. Other desktops share a player catalog.
#[cfg(not(target_os = "macos"))]
mod catalog;

use crate::{PlayerWindow, player::surface::PlayerDisplay};

#[derive(Clone, Default)]
pub(crate) struct Application {
    #[cfg(not(target_os = "macos"))]
    catalog: std::rc::Rc<catalog::ApplicationWindowMenu>,
}

#[cfg(not(target_os = "macos"))]
pub(crate) use catalog::Registration;

#[cfg(target_os = "macos")]
pub(crate) struct Registration;

#[cfg(target_os = "macos")]
impl Registration {
    pub(crate) fn retire(&self) {}
}

impl Application {
    pub(crate) fn register(&self, app: &PlayerWindow, display: &PlayerDisplay) -> Registration {
        #[cfg(not(target_os = "macos"))]
        {
            self.catalog.register(app, display.clone())
        }
        #[cfg(target_os = "macos")]
        {
            let _ = (app, display);
            Registration
        }
    }
}
