//! Own the History window and resolve live playback destinations at activation.
use super::{
    CollapsedGroups, HistoryState, availability::Availability, open_request, report, revision,
};
use crate::{
    PlaybackHistoryWindow, PlayerWindow, player::surface::PlayerDisplay, player_ui::InitialMedia,
};
use riina_core::playback_history::Entry;
use riina_ui::PlayerMenuActions;
use slint::{CloseRequestResponse, ComponentHandle, Timer, winit_030::WinitWindowAccessor};
use std::{
    cell::{Cell, OnceCell, RefCell},
    path::PathBuf,
    rc::{Rc, Weak},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
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

#[cfg(any(test, target_os = "macos"))]
impl Archive {
    fn load(preferences_path: PathBuf) -> Result<Self, String> {
        let preferences = crate::preference_load::read_valid(&preferences_path)
            .ok()
            .flatten()
            .unwrap_or_default();
        let path = crate::playback_history_store::path_for_preferences(&preferences_path);
        let entries = crate::playback_history_store::load(&path, &preferences.playback_history)?;
        let history = crate::playback_history::shared_playback_history_state(
            path.clone(),
            entries,
            Some(path),
            Some(preferences_path.with_file_name("watch-later")),
        );
        crate::playback_history::progress::configure_path_policy(&history, &preferences.advanced);
        let cleared = crate::playback_history::shared_history_merge_flag(&preferences_path);
        Ok(Self {
            history,
            preferences_path,
            cleared,
        })
    }
}

#[derive(Default)]
pub(crate) struct ApplicationHistory {
    window: RefCell<Option<PlaybackHistoryWindow>>,
    timer: Rc<Timer>,
    groups: CollapsedGroups,
    availability: Availability,
    destinations: RefCell<Vec<PlayerRoute>>,
    next_id: Cell<u64>,
    open_generation: Arc<AtomicU64>,
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

impl PlayerRoute {
    fn visible_window(&self) -> Option<PlayerWindow> {
        let app = self.window.upgrade()?;
        self.display.switchable(&app)?;
        Some(app)
    }
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
            if let Some(window) = owner.window.borrow().as_ref() {
                owner.refresh_destinations(window);
            }
        }
    }
}

impl ApplicationHistory {
    #[cfg(target_os = "macos")]
    pub(crate) fn clear_from_settings(&self, path: &std::path::Path) -> Result<usize, String> {
        let archive = Archive::load(path.to_owned())?;
        let mut history = archive
            .history
            .lock()
            .map_err(|_| "Playback History is unavailable")?;
        let removed = crate::utilities_preferences::clear_playback_history(path)?;
        history.clear();
        archive
            .cleared
            .store(true, std::sync::atomic::Ordering::Release);
        Ok(removed)
    }

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
                if let Err(error) = owner.show(Some(&app), &archive) {
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

    fn history_destination(&self) -> Option<(PlayerWindow, PlayerRoute)> {
        self.destinations
            .borrow()
            .iter()
            .rev()
            .find_map(|route| Some((route.visible_window()?, route.clone())))
    }

    fn refresh_destinations(&self, window: &PlaybackHistoryWindow) {
        let playing = self
            .destinations
            .borrow()
            .iter()
            .filter_map(PlayerRoute::visible_window)
            .any(|app| app.get_loaded());
        window.set_has_playing_destination(playing);
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

    pub(super) fn request_open(
        self: &Rc<Self>,
        window: &PlaybackHistoryWindow,
        paths: Vec<String>,
        intent: open_request::Intent,
    ) {
        if paths.is_empty() {
            return;
        }
        let generation = self
            .open_generation
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1);
        let current = self.open_generation.clone();
        let (ready, result) = tokio::sync::oneshot::channel();
        if let Err(error) = crate::background::spawn_named("riina-history-open", move || {
            let prepared =
                open_request::prepare(paths, || current.load(Ordering::Relaxed) != generation);
            let _ = ready.send(prepared);
        }) {
            report(
                window,
                format!("Could not prepare Playback History files: {error}"),
            );
            return;
        }

        window.set_status_text("Opening…".into());
        let owner = Rc::downgrade(self);
        let target = window.as_weak();
        if let Err(error) = slint::spawn_local(async move {
            let result = result.await;
            let (Some(owner), Some(window)) = (owner.upgrade(), target.upgrade()) else {
                return;
            };
            if owner.open_generation.load(Ordering::Relaxed) != generation
                || !window.window().is_visible()
            {
                return;
            }
            if let Ok(Some(prepared)) = result {
                if prepared.missing_local_file {
                    owner.availability.clear();
                }
                if prepared.paths.is_empty() {
                    window.set_status_text("No playable history items are available".into());
                    return;
                }
                window.set_status_text(Default::default());
                owner.dispatch_open(&window, prepared.paths, intent);
            } else {
                window.set_status_text("Could not prepare Playback History files".into());
            }
        }) {
            self.cancel_pending_open();
            report(
                window,
                format!("Could not open Playback History files: {error}"),
            );
        }
    }

    fn cancel_pending_open(&self) {
        self.open_generation.fetch_add(1, Ordering::Relaxed);
    }

    fn dispatch_open(
        &self,
        window: &PlaybackHistoryWindow,
        paths: Vec<String>,
        intent: open_request::Intent,
    ) {
        use open_request::Intent;

        if !matches!(intent, Intent::PlayInNewWindow)
            && let Some((app, route)) = self.history_destination()
        {
            let result = match intent {
                Intent::Single { position } => crate::media_open::history_entry(
                    &app,
                    &route.display,
                    &paths[0],
                    position,
                    route.player,
                ),
                _ => crate::media_open::history_playlist(&app, &route.display, paths, route.player),
            };
            if let Err(error) = result {
                report(
                    window,
                    format!("Could not open media from Playback History: {error}"),
                );
            }
            return;
        }

        match intent {
            Intent::Single { position } => self.launch_from_history(window, paths, position, false),
            _ => self.launch_from_history(window, paths, None, true),
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

    #[cfg(target_os = "macos")]
    pub(crate) fn show_from_application(
        self: &Rc<Self>,
        preferences_path: PathBuf,
    ) -> Result<(), String> {
        let archive = Archive::load(preferences_path)?;
        self.show(None, &archive).map_err(|error| error.to_string())
    }

    fn show(
        self: &Rc<Self>,
        app: Option<&PlayerWindow>,
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
                    availability: self.availability.clone(),
                },
            );
            if let Some(app) = app {
                crate::platform::center_auxiliary_window(&window, app);
            }
            self.connect_close(&window);
            *self.window.borrow_mut() = Some(window.clone_strong());
            window
        };
        self.availability.clear();
        self.refresh_destinations(&window);
        let mut last_revision = (revision(&archive.history), self.availability.revision());
        crate::platform::show_window(&window)?;
        crate::playback_history::progress::refresh_on_open(&archive.history);
        super::refresh(&window, &archive.history, &self.groups, &self.availability);
        crate::platform::enable_history_acrylic(&window);
        window.window().with_winit_window(|window| {
            window.set_minimized(false);
            window.focus_window();
        });
        let history = archive.history.clone();
        let groups = self.groups.clone();
        let availability = self.availability.clone();
        let owner = Rc::downgrade(self);
        super::super::visible_refresh::run(
            &window,
            &self.timer,
            Duration::from_millis(500),
            move |window| {
                if let Some(owner) = owner.upgrade() {
                    owner.refresh_destinations(window);
                }
                let current_revision = (revision(&history), availability.revision());
                if last_revision != current_revision {
                    super::refresh(window, &history, &groups, &availability);
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
            if let Some(owner) = weak.upgrade() {
                owner.cancel_pending_open();
            }
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
        self.cancel_pending_open();
        self.timer.stop();
        self.availability.clear();
        if let Some(window) = self.window.borrow().as_ref() {
            window.set_status_text(Default::default());
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
    fn windowless_history_uses_the_players_shared_persistent_archive()
    -> Result<(), Box<dyn std::error::Error>> {
        let profile = riina_test_support::TempDir::new("windowless-history-archive")?;
        let preferences = profile.path().join("preferences.json");
        let history_path = crate::playback_history_store::path_for_preferences(&preferences);
        let entry = Entry {
            path: "saved-film.mkv".into(),
            title: Some("Saved Film".into()),
            date_ms: 42,
            progress: None,
            duration: 8.0,
        };
        crate::playback_history_store::save(&history_path, std::slice::from_ref(&entry))?;

        let archive = Archive::load(preferences.clone())?;
        let player_state = crate::playback_history::shared_playback_history_state(
            history_path.clone(),
            Vec::new(),
            Some(history_path.clone()),
            Some(preferences.with_file_name("watch-later")),
        );
        assert!(Arc::ptr_eq(&archive.history, &player_state));
        let mut history = riina_core::sync::lock(&player_state);
        assert_eq!(history.entries(), std::slice::from_ref(&entry));
        assert_eq!(history.remove_paths([entry.path.as_str()])?, 1);
        assert!(crate::playback_history_store::load(&history_path, &[])?.is_empty());
        Ok(())
    }
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
        owner.show(None, &archive)?;
        let history = owner.window.borrow().as_ref().unwrap().clone_strong();
        assert!(history.window().is_visible());
        assert!(owner.active_player().is_none());
        history.set_search_text("saved search".into());
        let updates = Arc::new(AtomicUsize::default());
        let register = |name: &str| -> Result<_, slint::PlatformError> {
            let window = PlayerWindow::new()?;
            window.set_document_title(name.into());
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
        first.show()?;
        first.set_loaded(true);
        first
            .global::<PlayerMenuActions>()
            .invoke_show_playback_history();
        second
            .global::<PlayerMenuActions>()
            .invoke_show_playback_history();
        assert_eq!(
            owner.window.borrow().as_ref().unwrap().get_search_text(),
            "saved search"
        );
        let active_title = || owner.active_player().unwrap().0.get_window_title();
        assert_eq!(active_title(), "Second player");
        assert!(history.get_has_playing_destination());
        assert_eq!(
            owner.history_destination().unwrap().0.get_window_title(),
            "First player"
        );
        first.invoke_history_player_activated();
        assert_eq!(active_title(), "First player");
        assert!(owner.sync_plugins(&[]).is_ok());
        assert_eq!(updates.load(Ordering::Relaxed), 2);
        drop(first_route);
        assert_eq!(active_title(), "Second player");
        assert!(!history.get_has_playing_destination());
        assert!(owner.history_destination().is_none());
        drop(second_route);
        assert!(owner.is_visible());
        assert!(owner.active_player().is_none());
        owner.dispatch_open(
            &history,
            vec!["one.mkv".into(), "two.mkv".into()],
            open_request::Intent::Play,
        );
        assert!(launches.borrow()[0].explicit_playlist);
        assert_eq!(launches.borrow()[0].files, ["one.mkv", "two.mkv"]);
        owner.dispatch_open(
            &history,
            vec!["three.mkv".into()],
            open_request::Intent::Single { position: None },
        );
        assert!(launches.borrow()[1].resume_position.is_none());
        assert!(!launches.borrow()[1].explicit_playlist);
        first.hide()?;
        owner.hide();
        assert!(!owner.is_visible());
        Ok(())
    }
}
