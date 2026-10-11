//! Keep the player palette aligned with the user's native appearance choice.

use crate::PlayerWindow;
use riina_core::ThemeMaterial;
use slint::{
    ComponentHandle,
    winit_030::{WinitWindowAccessor, winit::window::Theme},
};
use std::{cell::Cell, rc::Rc, time::Duration};

/// Apply explicit themes on choice changes, and refresh the native override while
/// following the system because Winit's Windows theme value is a snapshot. Some
/// window systems expose no theme value.
pub(crate) fn connect(app: &PlayerWindow) -> Rc<slint::Timer> {
    let last_mode = Rc::new(Cell::new(None::<ThemeMaterial>));
    let observed_mode = last_mode.clone();
    let weak = app.as_weak();
    let timer = Rc::new(slint::Timer::default());
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(500),
        move || {
            let Some(app) = weak.upgrade() else { return };
            let Some(mode) = ThemeMaterial::from_index(app.get_appearance_mode()) else {
                return;
            };
            let apply_mode = observed_mode.get() != Some(mode);
            if apply_mode {
                observed_mode.set(Some(mode));
            }
            let previous_dark = app.get_dark_theme();
            app.window().with_winit_window(|window| {
                if apply_mode || mode == ThemeMaterial::System {
                    let override_theme = match mode {
                        ThemeMaterial::Dark => Some(Theme::Dark),
                        ThemeMaterial::Light => Some(Theme::Light),
                        ThemeMaterial::System => None,
                    };
                    window.set_theme(override_theme);
                }
                let system_is_dark = window
                    .theme()
                    .map(|theme| theme == Theme::Dark)
                    .unwrap_or(previous_dark);
                let effective_dark = mode.dark_for(system_is_dark);
                if previous_dark != effective_dark {
                    app.set_dark_theme(effective_dark);
                }
            });
        },
    );
    timer
}
