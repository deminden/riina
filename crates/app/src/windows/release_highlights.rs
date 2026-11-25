//! Present the installed release's guide and track its automatic opening.
mod first_launch;
mod notes;

use super::close_observer::CloseObserver;
use crate::{PlayerWindow, WelcomeWindow, platform};
use riina_ui::{PlayerAppearance, PlayerMenuActions, ReleaseHighlightsWindow};
use slint::{
    CloseRequestResponse, ComponentHandle, ModelRc, VecModel, winit_030::WinitWindowAccessor,
};
use std::{cell::RefCell, rc::Rc};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const NOTES: &str = include_str!(concat!(env!("OUT_DIR"), "/current-release.md"));

#[derive(Default)]
pub(crate) struct ReleaseHighlights {
    window: RefCell<Option<ReleaseHighlightsWindow>>,
    pub(crate) closed: CloseObserver,
}

impl ReleaseHighlights {
    pub(crate) fn connect(self: &Rc<Self>, player: &PlayerWindow) {
        let owner = self.clone();
        let weak = player.as_weak();
        player
            .global::<PlayerMenuActions>()
            .on_show_release_highlights(move || {
                if let Some(player) = weak.upgrade()
                    && let Err(error) = owner.show_for_player(&player, None)
                {
                    crate::diagnostics::report(
                        &player,
                        format!("Could not open Release Highlights: {error}"),
                    );
                }
            });
    }

    pub(crate) fn show_first_launch(
        self: &Rc<Self>,
        player: &PlayerWindow,
        welcome: Option<&WelcomeWindow>,
    ) {
        if cfg!(debug_assertions) || NOTES.trim().is_empty() {
            return;
        }
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let path = platform::preferences_path()?;
            if let Some(profile) = path.parent()
                && let Some(launch) = first_launch::FirstLaunch::claim(profile, VERSION)?
            {
                self.show_for_player(player, welcome)?;
                launch.record_shown()?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            crate::diagnostics::warn!("Could not present Release Highlights on launch: {error}");
        }
    }

    fn show_for_player(
        self: &Rc<Self>,
        player: &PlayerWindow,
        welcome: Option<&WelcomeWindow>,
    ) -> Result<(), slint::PlatformError> {
        let dark = player.global::<PlayerAppearance>().get_dark_theme();
        self.show(Some(player), welcome, dark).map(|_| ())
    }

    pub(crate) fn is_visible(&self) -> bool {
        self.window
            .borrow()
            .as_ref()
            .is_some_and(|window| window.window().is_visible())
    }

    pub(crate) fn hide(&self) {
        if let Some(window) = self.window.borrow().as_ref() {
            let _ = window.hide();
        }
    }

    pub(crate) fn show(
        self: &Rc<Self>,
        player: Option<&PlayerWindow>,
        welcome: Option<&WelcomeWindow>,
        dark: bool,
    ) -> Result<ReleaseHighlightsWindow, slint::PlatformError> {
        let existing = self
            .window
            .borrow()
            .as_ref()
            .map(ComponentHandle::clone_strong);
        let window = if let Some(window) = existing {
            window
        } else {
            let window = ReleaseHighlightsWindow::new()?;
            crate::appearance_preferences::install_system_typography(&window);
            window.set_release_name(format!("Riina {VERSION}").into());
            window.set_notes(ModelRc::new(VecModel::from(notes::rows(NOTES))));
            let weak = window.as_weak();
            window.on_browse_release(move || {
                if let Some(window) = weak.upgrade()
                    && let Err(error) = platform::open_http_url_in_browser(&format!(
                        "https://github.com/deminden/riina/releases/tag/v{VERSION}"
                    ))
                {
                    window.set_message(format!("Could not open release notes: {error}").into());
                }
            });
            let weak = window.as_weak();
            let owner = Rc::downgrade(self);
            window.on_dismiss(move || {
                if let Some(window) = weak.upgrade() {
                    let _ = window.hide();
                    if let Some(owner) = owner.upgrade() {
                        owner.closed.notify();
                    }
                }
            });
            let owner = Rc::downgrade(self);
            window.window().on_close_requested(move || {
                if let Some(owner) = owner.upgrade() {
                    owner.closed.notify();
                }
                CloseRequestResponse::HideWindow
            });
            if let Some(welcome) = welcome {
                platform::center_auxiliary_window(&window, welcome);
            } else if let Some(player) = player {
                platform::center_auxiliary_window(&window, player);
            }
            self.window.borrow_mut().replace(window.clone_strong());
            window
        };
        window.set_dark_theme(dark);
        window.set_message(if NOTES.trim().is_empty() {
            "Release notes are available online.".into()
        } else {
            "".into()
        });
        platform::show_window(&window)?;
        window.window().with_winit_window(|window| {
            window.set_minimized(false);
            window.focus_window();
        });
        window.invoke_focus_continue();
        Ok(window)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_menu_opens_one_versioned_guide() -> Result<(), Box<dyn std::error::Error>> {
        i_slint_backend_testing::init_no_event_loop();
        let owner = Rc::new(ReleaseHighlights::default());
        let first = PlayerWindow::new()?;
        let second = PlayerWindow::new()?;
        owner.connect(&first);
        owner.connect(&second);
        assert!(owner.window.borrow().is_none());
        let window = owner.show(None, None, true)?;
        assert!(window.window().is_visible());
        assert!(window.get_dark_theme());

        first
            .global::<PlayerMenuActions>()
            .invoke_show_release_highlights();
        assert_eq!(window.get_release_name(), format!("Riina {VERSION}"));
        window.invoke_dismiss();
        assert!(!window.window().is_visible());
        second
            .global::<PlayerMenuActions>()
            .invoke_show_release_highlights();
        assert!(window.window().is_visible());
        window.hide()?;
        Ok(())
    }
}
