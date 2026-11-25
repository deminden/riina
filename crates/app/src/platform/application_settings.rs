//! Route player settings requests to the application-owned window when present.
#[cfg(any(target_os = "macos", test))]
mod native;

use crate::{PlayerWindow, PreferencesWindow};
use std::{cell::RefCell, rc::Rc};

#[cfg(target_os = "macos")]
pub(crate) use native::{ProfileChanges, application_status, configure_application_actions};

#[cfg(target_os = "macos")]
pub(crate) const PLUGINS_PANE: i32 = 9;

#[derive(Clone, Default)]
pub(crate) struct Owner {
    font_picker: Rc<crate::font_picker::ApplicationFontPicker>,
    #[cfg(target_os = "macos")]
    profile: Rc<native::ApplicationSettings>,
}

impl Owner {
    pub(crate) fn font_picker(&self) -> &Rc<crate::font_picker::ApplicationFontPicker> {
        &self.font_picker
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn profile(&self) -> &Rc<native::ApplicationSettings> {
        &self.profile
    }

    pub(crate) fn bind_plugin_archive(
        &self,
        app: &PlayerWindow,
        local: &Rc<RefCell<Option<PreferencesWindow>>>,
    ) {
        #[cfg(target_os = "macos")]
        native::bind_plugin_archive(app, local, &self.profile);
        #[cfg(not(target_os = "macos"))]
        let _ = (app, local);
    }

    pub(crate) fn redirect_to_visible(&self, app: &PlayerWindow) -> bool {
        #[cfg(target_os = "macos")]
        {
            let pane = app.get_preferences_target_pane();
            match self.profile.show_visible((pane >= 0).then_some(pane)) {
                Ok(false) => false,
                Ok(true) => {
                    app.set_preferences_target_pane(-1);
                    true
                }
                Err(error) => {
                    app.set_status(error.to_string().into());
                    true
                }
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = app;
            false
        }
    }
}
