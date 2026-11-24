//! Own the History window and resolve live playback destinations at activation.
use super::{CollapsedGroups, HistoryState, report, revision};
use crate::{
    PlaybackHistoryWindow, PlayerWindow, media_open::HistoryDestination,
    player::surface::PlayerDisplay, player_ui::InitialMedia,
};
use riina_core::{RecentMedia, playback_history::Entry};
use riina_ui::{PlayerMenuActions, PlayerPreferences};
use slint::{CloseRequestResponse, ComponentHandle, Timer, winit_030::WinitWindowAccessor};
use std::{
    cell::{Cell, OnceCell, RefCell},
    path::PathBuf,
    rc::{Rc, Weak},
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

pub(crate) type HistorySync = Arc<dyn Fn(&[Entry]) -> Result<(), String> + Send + Sync>;
type Launch = Box<dyn Fn(&PlaybackHistoryWindow, InitialMedia) -> Result<(), String>>;

#[derive(Clone)]
pub(crate) struct Archive {
    pub(crate) history: HistoryState,
    pub(crate) preferences_path: PathBuf,
    pub(crate) cleared: Arc<AtomicBool>,
}

#[derive(Default)]
pub(crate) struct ApplicationHistory {
    window: RefCell<Option<PlaybackHistoryWindow>>,
    timer: Rc<Timer>,
    groups: CollapsedGroups,
    destinations: RefCell<Vec<PlayerRoute>>,
    next_id: Cell<u64>,
    launch: OnceCell<Launch>,
    close: OnceCell<Box<dyn Fn()>>,
}

#[derive(Clone)]
struct PlayerRoute {
    id: u64,
    window: slint::Weak<PlayerWindow>,
    player: riina_mpv::CommandSender,
    sync: HistorySync,
    display: PlayerDisplay,
}

pub(crate) struct Registration {
    owner: Weak<ApplicationHistory>,
    id: u64,
}

impl Drop for Registration {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.upgrade() {
            owner
                .destinations
                .borrow_mut()
                .retain(|route| route.id != self.id);
        }
    }
}

impl ApplicationHistory {
    pub(crate) fn bind(
        &self,
        launch: impl Fn(&PlaybackHistoryWindow, InitialMedia) -> Result<(), String> + 'static,
        close: impl Fn() + 'static,
    ) {
        let _ = self.launch.set(Box::new(launch));
        let _ = self.close.set(Box::new(close));
    }

    pub(crate) fn install(
        self: &Rc<Self>,
        app: &PlayerWindow,
        player: riina_mpv::CommandSender,
        archive: Archive,
        sync: HistorySync,
        display: PlayerDisplay,
    ) -> Registration {
        let id = self.next_id.get().wrapping_add(1);
        self.next_id.set(id);
        self.destinations.borrow_mut().insert(
            0,
            PlayerRoute {
                id,
                window: app.as_weak(),
                player,
                sync,
                display,
            },
        );
        let weak = Rc::downgrade(self);
        app.on_history_player_activated(move || {
            if let Some(owner) = weak.upgrade() {
                owner.activate(id);
            }
        });
        let weak = Rc::downgrade(self);
        let app_handle = app.as_weak();
        app.global::<PlayerMenuActions>()
            .on_show_playback_history(move || {
                let (Some(owner), Some(app)) = (weak.upgrade(), app_handle.upgrade()) else {
                    return;
                };
                owner.activate(id);
                if let Err(error) = owner.show(&app, &archive) {
                    crate::diagnostics::report(
                        &app,
                        format!("Could not open Playback History: {error}"),
                    );
                }
            });
        Registration {
            owner: Rc::downgrade(self),
            id,
        }
    }

    fn activate(&self, id: u64) {
        let mut destinations = self.destinations.borrow_mut();
        if let Some(index) = destinations.iter().position(|route| route.id == id) {
            destinations[index..].rotate_left(1);
        }
    }

    fn active_player(&self) -> Option<(PlayerWindow, PlayerRoute)> {
        self.destinations
            .borrow()
            .iter()
            .rev()
            .find_map(|route| Some((route.window.upgrade()?, route.clone())))
    }

    pub(crate) fn browser_destination(&self) -> Option<(PlayerWindow, PlayerDisplay)> {
        self.active_player()
            .map(|(app, route)| (app, route.display))
    }

    pub(crate) fn sync_plugins(&self, entries: &[Entry]) -> Result<(), String> {
        let routes = self.destinations.borrow().clone();
        let mut failure = None;
        for route in routes {
            if let Err(error) = (route.sync)(entries) {
                failure.get_or_insert(error);
            }
        }
        failure.map_or(Ok(()), Err)
    }

    pub(super) fn open_single(&self, window: &PlaybackHistoryWindow, path: String, position: f64) {
        if let Some((app, route)) = self.active_player() {
            if let Err(error) = route.display.reveal(&app) {
                report(window, format!("Could not show the player: {error}"));
                return;
            }
            crate::media_open::open_recent(
                &app,
                RecentMedia {
                    path,
                    title: String::new(),
                    position,
                },
                app.global::<PlayerPreferences>().get_resume_last_position(),
                route.player,
            );
        } else {
            self.launch_from_history(window, vec![path], Some(position), false);
        }
    }

    pub(super) fn open_selection(
        &self,
        window: &PlaybackHistoryWindow,
        paths: Vec<String>,
        destination: HistoryDestination,
    ) {
        if paths.is_empty() {
            return;
        }
        if let Some((app, route)) = self.active_player() {
            if matches!(destination, HistoryDestination::CurrentPlayer)
                && let Err(error) = route.display.reveal(&app)
            {
                report(window, format!("Could not show the player: {error}"));
                return;
            }
            crate::media_open::history_selection(&app, paths, destination, route.player);
        } else {
            self.launch_from_history(window, paths, None, true);
        }
    }

    fn launch_from_history(
        &self,
        window: &PlaybackHistoryWindow,
        paths: Vec<String>,
        resume_position: Option<f64>,
        explicit_playlist: bool,
    ) {
        let launch = self
            .launch
            .get()
            .ok_or_else(|| "Riina is closing".to_owned());
        let media = InitialMedia {
            files: paths.into_iter().map(Into::into).collect(),
            resume_position,
            explicit_playlist,
            ..InitialMedia::default()
        };
        if let Err(error) = launch.and_then(|launch| launch(window, media)) {
            report(
                window,
                format!("Could not open media from Playback History: {error}"),
            );
        }
    }

    fn show(
        self: &Rc<Self>,
        app: &PlayerWindow,
        archive: &Archive,
    ) -> Result<(), slint::PlatformError> {
        let existing = self
            .window
            .borrow()
            .as_ref()
            .map(ComponentHandle::clone_strong);
        let window = if let Some(window) = existing {
            window
        } else {
            let window = PlaybackHistoryWindow::new()?;
            crate::appearance_preferences::install_system_typography(&window);
            super::connect_window(
                &window,
                super::WindowConnection {
                    archive: archive.clone(),
                    collapsed_groups: self.groups.clone(),
                    owner: Rc::downgrade(self),
                    confirmation: Rc::default(),
                },
            );
            crate::platform::center_auxiliary_window(&window, app);
            self.connect_close(&window);
            *self.window.borrow_mut() = Some(window.clone_strong());
            window
        };
        let mut last_revision = revision(&archive.history);
        super::refresh(&window, &archive.history, &self.groups);
        crate::platform::show_window(&window)?;
        crate::platform::enable_history_acrylic(&window);
        window.window().with_winit_window(|window| {
            window.set_minimized(false);
            window.focus_window();
        });
        let history = archive.history.clone();
        let groups = self.groups.clone();
        super::super::visible_refresh::run(
            &window,
            &self.timer,
            Duration::from_millis(500),
            move |window| {
                let current_revision = revision(&history);
                if last_revision != current_revision {
                    super::refresh(window, &history, &groups);
                    last_revision = current_revision;
                }
            },
        );
        Ok(())
    }

    fn connect_close(self: &Rc<Self>, window: &PlaybackHistoryWindow) {
        let weak = Rc::downgrade(self);
        let history = window.as_weak();
        window.window().on_close_requested(move || {
            if let Some(history) = history.upgrade() {
                history.invoke_answer_confirmation(false);
            }
            let weak = weak.clone();
            // The application can create or release windows during this action.
            if slint::spawn_local(async move {
                if let Some(owner) = weak.upgrade() {
                    owner.hide();
                    if let Some(closed) = owner.close.get() {
                        closed();
                    }
                }
            })
            .is_ok()
            {
                CloseRequestResponse::KeepWindowShown
            } else {
                CloseRequestResponse::HideWindow
            }
        });
    }

    pub(crate) fn is_visible(&self) -> bool {
        self.window
            .borrow()
            .as_ref()
            .is_some_and(|window| window.window().is_visible())
    }

    pub(crate) fn hide(&self) {
        self.timer.stop();
        if let Some(window) = self.window.borrow().as_ref() {
            window.invoke_answer_confirmation(false);
            let _ = window.hide();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn history_retains_one_window_and_releases_closed_player_routes()
    -> Result<(), slint::PlatformError> {
        i_slint_backend_testing::init_no_event_loop();
        let owner = Rc::new(ApplicationHistory::default());
        let launches = Rc::new(RefCell::new(Vec::new()));
        let launched = launches.clone();
        owner.bind(
            move |_, media| {
                launched.borrow_mut().push(media);
                Ok(())
            },
            || {},
        );
        let archive = Archive {
            history: Arc::default(),
            preferences_path: PathBuf::new(),
            cleared: Arc::default(),
        };
        let updates = Arc::new(AtomicUsize::default());
        let register = |name: &str| -> Result<_, slint::PlatformError> {
            let window = PlayerWindow::new()?;
            window.set_window_title(name.into());
            let (sender, _test_engine) = riina_mpv::CommandSender::recording(1);
            let updates = updates.clone();
            let route = owner.install(
                &window,
                sender,
                archive.clone(),
                Arc::new(move |_| {
                    updates.fetch_add(1, Ordering::Relaxed);
                    Ok(())
                }),
                PlayerDisplay::default(),
            );
            Ok((window, route))
        };
        let (first, first_route) = register("First player")?;
        let (second, second_route) = register("Second player")?;
        first
            .global::<PlayerMenuActions>()
            .invoke_show_playback_history();
        let history = owner.window.borrow().as_ref().unwrap().clone_strong();
        history.set_search_text("saved search".into());
        second
            .global::<PlayerMenuActions>()
            .invoke_show_playback_history();
        assert_eq!(
            owner.window.borrow().as_ref().unwrap().get_search_text(),
            "saved search"
        );
        let active_title = || owner.active_player().unwrap().0.get_window_title();
        assert_eq!(active_title(), "Second player");
        first.invoke_history_player_activated();
        assert_eq!(active_title(), "First player");
        assert!(owner.sync_plugins(&[]).is_ok());
        assert_eq!(updates.load(Ordering::Relaxed), 2);
        drop(first_route);
        assert_eq!(active_title(), "Second player");
        drop(second_route);
        assert!(owner.is_visible());
        assert!(owner.active_player().is_none());
        owner.open_selection(
            &history,
            vec!["one.mkv".into(), "two.mkv".into()],
            HistoryDestination::CurrentPlayer,
        );
        assert!(launches.borrow()[0].explicit_playlist);
        assert_eq!(launches.borrow()[0].files, ["one.mkv", "two.mkv"]);
        owner.hide();
        assert!(!owner.is_visible());
        Ok(())
    }
}
