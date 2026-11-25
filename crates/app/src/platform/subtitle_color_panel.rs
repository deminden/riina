//! Connect portable color-well intents to AppKit's shared panel.
use crate::{PlayerWindow, PreferencesWindow};

#[cfg(not(target_os = "macos"))]
pub(crate) fn connect_player(_: &PlayerWindow) {}

#[cfg(not(target_os = "macos"))]
pub(crate) fn connect_preferences(_: &PreferencesWindow, _: impl Fn(String) + 'static) {}

#[cfg(target_os = "macos")]
pub(crate) use native::{connect_player, connect_preferences};

#[cfg(target_os = "macos")]
mod native {
    use super::{PlayerWindow, PreferencesWindow};
    use crate::platform::macos::MacSubtitleColorPanel;
    use riina_ui::{PlayerSidebarActions, SubtitleColorTarget, SubtitleSettingsState};
    use slint::{Color, ComponentHandle};
    use std::{cell::RefCell, rc::Rc};

    #[derive(Clone, Default)]
    struct PanelOwner(Rc<RefCell<Option<MacSubtitleColorPanel>>>);

    impl PanelOwner {
        fn open(&self, color: Color, changed: impl Fn(Color) + 'static) -> Result<(), String> {
            let initial = [color.red(), color.green(), color.blue(), color.alpha()];
            let next = MacSubtitleColorPanel::open(initial, move |[red, green, blue, alpha]| {
                changed(Color::from_argb_u8(alpha, red, green, blue));
            })?;
            // Drop outside the RefCell borrow because AppKit delivers focus callbacks.
            drop(self.0.replace(Some(next)));
            Ok(())
        }

        fn dismiss(&self) {
            drop(self.0.take());
        }
    }

    pub(crate) fn connect_player(app: &PlayerWindow) {
        let owner = PanelOwner::default();
        let dismiss = owner.clone();
        let actions = app.global::<PlayerSidebarActions>();
        actions.on_dismiss_native_subtitle_color(move || dismiss.dismiss());
        let weak = app.as_weak();
        actions.on_request_native_subtitle_color(move |target, color| {
            let Some(app) = weak.upgrade().filter(|app| app.get_loaded()) else {
                return;
            };
            let weak = app.as_weak();
            if let Err(error) = owner.open(color, move |value| {
                if let Some(app) = weak
                    .upgrade()
                    .filter(|app| app.get_loaded() && app.window().is_visible())
                {
                    app.global::<PlayerSidebarActions>()
                        .invoke_set_subtitle_color(target, value);
                }
            }) {
                crate::diagnostics::report(&app, error);
            }
        });
    }

    pub(crate) fn connect_preferences(
        window: &PreferencesWindow,
        report: impl Fn(String) + 'static,
    ) {
        let owner = PanelOwner::default();
        let dismiss = owner.clone();
        window.on_dismiss_native_subtitle_color(move || dismiss.dismiss());
        let weak = window.as_weak();
        window.on_native_subtitle_color_requested(move |target, color| {
            let weak = weak.clone();
            if let Err(error) = owner.open(color, move |value| {
                let Some(window) = weak.upgrade().filter(|window| window.window().is_visible())
                else {
                    return;
                };
                let subtitles = window.global::<SubtitleSettingsState>();
                match target {
                    SubtitleColorTarget::Text => subtitles.set_subtitle_text_color(value),
                    SubtitleColorTarget::Background => {
                        subtitles.set_subtitle_background_color(value);
                        subtitles.set_subtitle_shadow_color(value);
                    }
                    SubtitleColorTarget::Outline => subtitles.set_subtitle_outline_color(value),
                }
                window.invoke_settings_changed();
            }) {
                report(error);
            }
        });
    }
}
