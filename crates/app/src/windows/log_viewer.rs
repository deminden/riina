//! One application-owned Log Viewer, shared by player menus and Settings.
mod export;
mod reader;
mod selection;
mod view;

use super::close_observer::CloseObserver;
use crate::{LogViewerWindow, PlayerWindow};
use riina_core::advanced_preferences::LogLevel;
use riina_ui::PlayerMenuActions;
use slint::{CloseRequestResponse, ComponentHandle, Timer, winit_030::WinitWindowAccessor};
use std::{cell::RefCell, path::Path, rc::Rc, time::Duration};
use view::View;

const REFRESH_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Default)]
pub(crate) struct ApplicationLogViewer {
    session: RefCell<Option<Session>>,
    timer: Rc<Timer>,
    pub(crate) closed: CloseObserver,
}

#[derive(Clone)]
struct Session {
    window: Rc<LogViewerWindow>,
    view: Rc<RefCell<View>>,
}

impl ApplicationLogViewer {
    pub(crate) fn connect(
        self: &Rc<Self>,
        app: &PlayerWindow,
        preferences: &Path,
        level: LogLevel,
    ) {
        let owner = self.clone();
        let source = app.as_weak();
        let preferences = preferences.to_path_buf();
        app.global::<PlayerMenuActions>()
            .on_show_log_viewer(move || {
                if let Some(app) = source.upgrade()
                    && let Err(error) = owner.show(Some(&app), &preferences, level)
                {
                    crate::diagnostics::report(&app, format!("Could not show Log Viewer: {error}"));
                }
            });
    }

    pub(crate) fn show(
        self: &Rc<Self>,
        app: Option<&PlayerWindow>,
        preferences: &Path,
        level: LogLevel,
    ) -> Result<(), slint::PlatformError> {
        let existing = self.session.borrow().clone();
        let session = if let Some(session) = existing {
            session
        } else {
            let window = Rc::new(LogViewerWindow::new()?);
            crate::appearance_preferences::install_system_typography(window.as_ref());
            window.set_level_index(match level {
                LogLevel::Verbose => 5,
                LogLevel::Debug => 6,
                LogLevel::Warning => 3,
                LogLevel::Error => 2,
            });
            let view = Rc::new(RefCell::new(View::new(&window, preferences)));
            bind(&window, &view, preferences);
            if let Some(app) = app {
                crate::platform::center_auxiliary_window(window.as_ref(), app);
            }
            let window_ref = window.as_weak();
            let owner = Rc::downgrade(self);
            window.window().on_close_requested(move || {
                if window_ref
                    .upgrade()
                    .is_some_and(|window| window.get_save_in_progress())
                {
                    return CloseRequestResponse::KeepWindowShown;
                }
                if let Some(owner) = owner.upgrade() {
                    owner.timer.stop();
                    owner.closed.notify();
                }
                CloseRequestResponse::HideWindow
            });
            let session = Session { window, view };
            self.session.borrow_mut().replace(session.clone());
            session
        };

        let window = &*session.window;
        session.view.borrow_mut().refresh(window, true);
        crate::platform::show_window(window)?;
        crate::platform::enable_log_viewer_acrylic(window);
        window.window().with_winit_window(|window| {
            window.set_minimized(false);
            window.focus_window();
        });
        if !self.timer.running() {
            let view = session.view.clone();
            super::visible_refresh::run(window, &self.timer, REFRESH_INTERVAL, move |window| {
                view.borrow_mut().refresh(window, false);
            });
        }
        Ok(())
    }

    pub(crate) fn is_visible(&self) -> bool {
        self.session
            .borrow()
            .as_ref()
            .is_some_and(|session| session.window.window().is_visible())
    }

    pub(crate) fn hide(&self) {
        self.timer.stop();
        if let Some(session) = self.session.borrow().as_ref() {
            let _ = session.window.hide();
        }
    }
}

fn bind(window: &LogViewerWindow, view: &Rc<RefCell<View>>, preferences: &Path) {
    let weak = window.as_weak();
    let state = view.clone();
    window.on_refresh_logs(move || {
        if let Some(window) = weak.upgrade() {
            state.borrow_mut().refresh(&window, true);
        }
    });
    let weak = window.as_weak();
    let state = view.clone();
    window.on_filters_changed(move || {
        if let Some(window) = weak.upgrade() {
            state.borrow_mut().render(&window);
        }
    });
    let weak = window.as_weak();
    let state = view.clone();
    window.on_subsystem_toggled(move |name| {
        if let Some(window) = weak.upgrade() {
            state.borrow_mut().toggle_subsystem(&window, name.as_str());
        }
    });
    let weak = window.as_weak();
    let state = view.clone();
    window.on_subsystem_filter_cleared(move || {
        if let Some(window) = weak.upgrade() {
            state.borrow_mut().clear_subsystems(&window);
        }
    });
    bind_selection(window, view);

    let weak = window.as_weak();
    let directory = preferences.parent().unwrap_or(Path::new(".")).to_path_buf();
    window.on_open_log_folder(move || {
        if let Some(window) = weak.upgrade()
            && let Err(error) = crate::platform::open_directory(&directory)
        {
            window.set_feedback_text(format!("Could not open the log directory: {error}").into());
        }
    });
    export::bind(window, view, preferences);
}

fn bind_selection(window: &LogViewerWindow, view: &Rc<RefCell<View>>) {
    let weak = window.as_weak();
    let state = view.clone();
    window.on_select_row(move |index, toggle, range| {
        if let Some(window) = weak.upgrade() {
            state.borrow_mut().select(&window, index, toggle, range);
        }
    });
    let weak = window.as_weak();
    let state = view.clone();
    window.on_select_all_rows(move || {
        if let Some(window) = weak.upgrade() {
            state.borrow_mut().select_all(&window);
        }
    });
    let weak = window.as_weak();
    let state = view.clone();
    window.on_navigate_rows(move |action, range| {
        if let Some(window) = weak.upgrade() {
            state.borrow_mut().navigate(&window, action, range);
        }
    });
    let weak = window.as_weak();
    let state = view.clone();
    window.on_copy_rows(move |index| {
        if let Some(window) = weak.upgrade() {
            let content = state.borrow().copy(index);
            if !content.is_empty()
                && let Err(error) = crate::platform::clipboard::copy_text(&content)
            {
                window.set_feedback_text(format!("Could not copy log entries: {error}").into());
            }
        }
    });
}

#[cfg(test)]
mod tests;
