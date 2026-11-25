//! Platform-specific Settings form ownership.
#[cfg(target_os = "macos")]
use crate::PreferencesWindow;
use riina_core::osc_preferences::ControllerOptions;
use std::path::Path;

#[derive(Default)]
pub(crate) struct ControllerOptionsForm {
    #[cfg(target_os = "macos")]
    current: std::cell::RefCell<Option<(std::path::PathBuf, slint::Weak<PreferencesWindow>)>>,
}
impl ControllerOptionsForm {
    #[cfg(target_os = "macos")]
    pub(crate) fn register(&self, path: &Path, window: &PreferencesWindow) {
        use slint::ComponentHandle;
        self.current
            .replace(Some((path.to_owned(), window.as_weak())));
    }

    pub(crate) fn publish(&self, path: &Path, options: ControllerOptions) {
        #[cfg(target_os = "macos")]
        if let Some((owner, form)) = self.current.borrow().as_ref()
            && owner == path
            && let Some(form) = form.upgrade()
        {
            crate::osc_preferences::install_form_controller_options(&form, options);
            form.invoke_controller_options_adopted();
        }
        #[cfg(not(target_os = "macos"))]
        let _ = (path, options);
    }
}
