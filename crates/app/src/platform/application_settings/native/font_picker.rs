//! Own the font chooser opened from application Settings.
use crate::{
    FontPicker,
    windows::font_picker::{discover_catalog, refresh_font, selected_name},
};
use slint::ComponentHandle;
use std::{cell::RefCell, rc::Rc};

pub(super) fn connect(window: &crate::PreferencesWindow, report: impl Fn(String) + 'static) {
    let picker = Rc::new(RefCell::new(None::<FontPicker>));
    let closing = picker.clone();
    window.on_close_profile_font_picker(move || {
        if let Some(picker) = closing.borrow_mut().take() {
            let _ = picker.hide();
        }
    });
    let weak = window.as_weak();
    window.on_choose_subtitle_font(move || {
        let Some(window) = weak.upgrade() else { return };
        let result = (|| -> Result<(), slint::PlatformError> {
            if picker.borrow().is_none() {
                let created = FontPicker::new()?;
                crate::appearance_preferences::install_system_typography(&created);
                let parent = window.as_weak();
                let target = created.as_weak();
                created.on_accept_font(move |name| {
                    if let Some(window) = parent.upgrade() {
                        window
                            .global::<crate::SubtitleSettingsState>()
                            .set_subtitle_font(selected_name(name).into());
                        window.invoke_settings_changed();
                    }
                    if let Some(picker) = target.upgrade() {
                        picker.set_picker_active(false);
                        let _ = picker.hide();
                    }
                });
                let target = created.as_weak();
                created.on_cancel(move || {
                    if let Some(picker) = target.upgrade() {
                        picker.set_picker_active(false);
                        let _ = picker.hide();
                    }
                });
                discover_catalog(created.as_weak(), &report);
                picker.replace(Some(created));
            }
            if let Some(picker) = picker.borrow().as_ref() {
                refresh_font(
                    picker,
                    window
                        .global::<crate::SubtitleSettingsState>()
                        .get_subtitle_font(),
                );
                picker.set_picker_active(true);
                crate::platform::show_window(picker)?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            report(error.to_string());
        }
    });
}
