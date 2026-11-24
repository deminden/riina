//! Platform-specific Settings controls, including the AppKit color panel.
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

#[cfg(not(target_os = "macos"))]
pub(crate) fn install_subtitle_color_panel(_: &PreferencesWindow, _: impl Fn(String) + 'static) {}

#[cfg(target_os = "macos")]
pub(crate) fn install_subtitle_color_panel(
    window: &PreferencesWindow,
    report: impl Fn(String) + 'static,
) {
    use slint::ComponentHandle;
    use std::{cell::RefCell, rc::Rc};

    let active_color_panel = Rc::new(RefCell::new(None));
    let active_panel_slot = active_color_panel.clone();
    let color_panel_window = window.as_weak();
    window.on_native_subtitle_color_requested(move |target, color| {
        let Some(window) = color_panel_window.upgrade() else {
            return;
        };
        let initial = [color.red(), color.green(), color.blue(), color.alpha()];
        let weak_window = window.as_weak();
        let panel = crate::platform::MacSubtitleColorPanel::open(initial, move |channels| {
            let Some(window) = weak_window.upgrade() else {
                return;
            };
            let color =
                slint::Color::from_argb_u8(channels[3], channels[0], channels[1], channels[2]);
            match target {
                1 => window
                    .global::<crate::SubtitleSettingsState>()
                    .set_subtitle_text_color(color),
                2 => {
                    window
                        .global::<crate::SubtitleSettingsState>()
                        .set_subtitle_background_color(color);
                    window
                        .global::<crate::SubtitleSettingsState>()
                        .set_subtitle_shadow_color(color);
                }
                3 => window
                    .global::<crate::SubtitleSettingsState>()
                    .set_subtitle_outline_color(color),
                4 => {
                    window
                        .global::<crate::SubtitleSettingsState>()
                        .set_subtitle_shadow_color(color);
                    window
                        .global::<crate::SubtitleSettingsState>()
                        .set_subtitle_background_color(color);
                }
                _ => return,
            }
            window.invoke_settings_changed();
        });
        match panel {
            Ok(panel) => *active_panel_slot.borrow_mut() = Some(panel),
            Err(error) => report(error),
        }
    });
}
