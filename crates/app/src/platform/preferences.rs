//! Platform-specific Settings controls, including the AppKit color panel.
use crate::{PlayerWindow, PreferencesWindow};

#[cfg(not(target_os = "macos"))]
pub(crate) fn install_subtitle_color_panel(_: &PlayerWindow, _: &PreferencesWindow) {}

#[cfg(target_os = "macos")]
pub(crate) fn install_subtitle_color_panel(app: &PlayerWindow, window: &PreferencesWindow) {
    use slint::ComponentHandle;
    use std::{cell::RefCell, rc::Rc};

    let active_color_panel = Rc::new(RefCell::new(None));
    let active_panel_slot = active_color_panel.clone();
    let color_panel_window = window.as_weak();
    let color_panel_parent = app.as_weak();
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
            Err(error) => {
                if let Some(app) = color_panel_parent.upgrade() {
                    app.set_status(error.into());
                }
            }
        }
    });
}
