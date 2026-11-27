//! One Windows event loop owns the native player frames in this application.
use super::{NativeLaunch, NativePlayer};
use crate::platform::application_lifecycle::{ClosedWindow, LastWindowAction, last_window_action};
use crate::player_ui::InitialMedia;
use crate::plugins::plugin_players::Ticket;
use crate::{PlayerWindow, media_open, preference_load};
use riina_core::{Preferences, ThemeMaterial, sync::lock};
use riina_ui::{PlayerDialogState, PlayerMenuActions, PlayerPreferences};
use slint::{
    CloseRequestResponse, ComponentHandle, Model, PlatformError,
    winit_030::{WinitWindowAccessor, winit},
};
use std::{
    cell::{Cell, RefCell},
    ffi::OsString,
    rc::{Rc, Weak},
    sync::{
        Arc, Mutex,
        atomic::{AtomicIsize, Ordering},
    },
};
use winit::{platform::windows::WindowAttributesExtWindows, window::Theme};

#[derive(Clone, Default)]
pub(super) struct Backend {
    next_owner: Arc<AtomicIsize>,
    native_theme: Arc<Mutex<Option<Theme>>>,
}

impl Backend {
    fn install() -> Result<Self, PlatformError> {
        let backend = Self::default();
        let attributes = backend.clone();
        slint::BackendSelector::new()
            .backend_name("winit".into())
            .renderer_name("femtovg".into())
            .with_winit_custom_application_handler(
                crate::platform::plugin_standalone::NativeWindowEvents,
            )
            .with_winit_window_attributes_hook(move |window| {
                let window = window.with_drag_and_drop(false).with_transparent(true);
                // Only the controls adapter receives this frame's HWND.
                let owner = attributes.next_owner.swap(0, Ordering::Relaxed);
                if owner == 0 {
                    // Muda uses the frame's initial theme when it attaches the menu.
                    window.with_theme(*lock(&attributes.native_theme))
                } else {
                    window
                        .with_owner_window(owner)
                        .with_decorations(false)
                        .with_resizable(false)
                        .with_skip_taskbar(true)
                }
            })
            .select()?;
        Ok(backend)
    }

    fn set_theme(&self, preferences: &Preferences) {
        *lock(&self.native_theme) = match preferences.theme_material {
            ThemeMaterial::Dark => Some(Theme::Dark),
            ThemeMaterial::Light => Some(Theme::Light),
            ThemeMaterial::System => None,
        };
    }

    pub(super) fn create_controls(&self, owner: isize) -> Result<PlayerWindow, PlatformError> {
        self.next_owner.store(owner, Ordering::Relaxed);
        let controls = PlayerWindow::new();
        // The adapter captures the hook synchronously. Clear the hint on failure too.
        self.next_owner.store(0, Ordering::Relaxed);
        controls
    }
}

pub(super) fn run(
    launches: impl Iterator<Item = NativeLaunch>,
    browser: &crate::browser_links::Inbox,
) -> Result<(), Box<dyn std::error::Error>> {
    let application = Rc::new(Application {
        backend: Backend::install()?,
        players: RefCell::default(),
        services: Default::default(),
        initial_error: RefCell::default(),
        quitting: Cell::default(),
        pending_close: Cell::default(),
    });
    let owner = Rc::downgrade(&application);
    application
        .services
        .plugins
        .bind_player_launcher(move |parent, ticket| {
            let app = owner.upgrade().ok_or("The application has closed")?;
            let path = crate::platform::preferences_path().map_err(|error| error.to_string())?;
            let loaded = preference_load::load(&path).map_err(|error| error.to_string())?;
            let launch = NativeLaunch {
                media: InitialMedia::for_plugin(ticket),
                owner_center: parent
                    .and_then(|app| crate::platform::player_window_center(app.window())),
                loaded,
            };
            let player = app
                .launch(launch, parent.map(|app| Origin::Player(app.as_weak())))
                .map_err(|error| error.to_string())?;
            let player = Rc::downgrade(&player);
            let owner = owner.clone();
            Ok(Box::new(move || {
                if let (Some(app), Some(player)) = (owner.upgrade(), player.upgrade()) {
                    app.close(&player, ClosedWindow::Retired);
                }
            }))
        });
    let owner = Rc::downgrade(&application);
    let open_url = owner.clone();
    application.services.open_url.bind(move |owner, stream| {
        let application = open_url.upgrade().ok_or("Riina is closing")?;
        let path = crate::platform::preferences_path().map_err(|error| error.to_string())?;
        let loaded = preference_load::load(&path).map_err(|error| error.to_string())?;
        let media = InitialMedia {
            stream: Some(stream),
            ..InitialMedia::default()
        };
        let launch = NativeLaunch {
            media,
            loaded,
            owner_center: owner.and_then(|app| crate::platform::player_window_center(app.window())),
        };
        application
            .launch(launch, owner.map(|app| Origin::Player(app.as_weak())))
            .map(|_| ())
            .map_err(|error| error.to_string())
    });
    let closed = owner.clone();
    let filters_closed = owner.clone();
    application.services.filters.closed.bind(Rc::new(move || {
        if let Some(application) = filters_closed.upgrade() {
            application.finish_when_empty();
        }
    }));
    application.services.history.bind(
        move |window, mut media| {
            let application = owner.upgrade().ok_or("Riina is closing")?;
            let path = crate::platform::preferences_path().map_err(|error| error.to_string())?;
            let loaded = preference_load::load(&path).map_err(|error| error.to_string())?;
            if !loaded.preferences.resume_last_position {
                media.resume_position = None;
            }
            let launch = NativeLaunch {
                media,
                loaded,
                owner_center: crate::platform::player_window_center(window.window()),
            };
            application
                .launch(launch, Some(Origin::History(window.as_weak())))
                .map(|_| ())
                .map_err(|error| error.to_string())
        },
        move || {
            if let Some(application) = closed.upgrade() {
                application.finish_when_empty();
            }
        },
    );
    for (index, launch) in launches.enumerate() {
        let origin = (index > 0).then_some(Origin::CommandLine);
        if let Err(error) = application.launch(launch, origin) {
            application.finish();
            return Err(error);
        }
    }
    let owner = Rc::downgrade(&application);
    let _browser = browser.register(move |link| {
        if let Some(application) = owner.upgrade()
            && let Err(error) = application.open_browser_link(link)
        {
            crate::diagnostics::warn!("Could not open browser link: {error}");
        }
    });
    let result = slint::run_event_loop_until_quit();
    application.finish();
    if let Some(error) = application.initial_error.borrow_mut().take() {
        return Err(error);
    }
    Ok(result?)
}

struct Application {
    backend: Backend,
    players: RefCell<Vec<Rc<NativePlayer>>>,
    services: crate::player_ui::ApplicationServices,
    initial_error: RefCell<Option<Box<dyn std::error::Error>>>,
    quitting: Cell<bool>,
    pending_close: Cell<Option<LastWindowAction>>,
}

impl Application {
    fn open_browser_link(
        self: &Rc<Self>,
        link: crate::browser_links::Link,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let active = self.services.history.browser_destination();
        if let Some((app, display)) = &active
            && link.reuse_window(app)
            && let Some(player) = self
                .players
                .borrow()
                .iter()
                .find(|player| player.owns_controls(app))
                .cloned()
            && let Some(session) = player.session.borrow().as_ref()
        {
            display.reveal(app)?;
            display.focus(app);
            return session.open_browser_link(app, link).map_err(Into::into);
        }
        let owner_center = active
            .as_ref()
            .and_then(|(app, _)| crate::platform::player_window_center(app.window()));
        self.launch(
            NativeLaunch {
                media: link.initial_media(),
                owner_center,
                loaded: preference_load::load(&crate::platform::preferences_path()?)?,
            },
            Some(Origin::BrowserLink),
        )?;
        Ok(())
    }

    fn launch(
        self: &Rc<Self>,
        launch: NativeLaunch,
        origin: Option<Origin>,
    ) -> Result<Rc<NativePlayer>, Box<dyn std::error::Error>> {
        if self.quitting.get() {
            return Err("Riina is closing".into());
        }
        self.backend.set_theme(&launch.loaded.preferences);
        let player = Rc::new(NativePlayer::create_host()?);
        let actions = WindowActions {
            application: Rc::downgrade(self),
            player: Rc::downgrade(&player),
            origin,
            managed: launch.media.managed.clone(),
        };
        // Keep the host owned while Slint is realizing its frame and controls.
        self.players.borrow_mut().push(player.clone());
        if let Err(error) =
            player.start(launch, self.backend.clone(), actions, self.services.clone())
        {
            self.close(&player, ClosedWindow::Retired);
            return Err(error);
        }
        self.pending_close.set(None);
        Ok(player)
    }

    fn open(
        self: &Rc<Self>,
        owner: &PlayerWindow,
        media: InitialMedia,
        reference: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if media.files.is_empty() {
            return Ok(());
        }
        let loaded = preference_load::load(&crate::platform::preferences_path()?)?;
        let existing = self.players.borrow().clone();
        let duplicate = media_open::duplicate_player(
            reference,
            loaded.preferences.allow_duplicate_players,
            existing.iter().map(|player| player.current_media()),
        );
        if let Some(player) = duplicate.and_then(|index| existing.get(index)) {
            player.host.window().with_winit_window(|window| {
                window.set_minimized(false);
                window.focus_window();
            });
            owner.set_status("The selected media is already open.".into());
            return Ok(());
        }
        self.launch(
            NativeLaunch {
                media,
                owner_center: crate::platform::player_window_center(owner.window()),
                loaded,
            },
            Some(Origin::Player(owner.as_weak())),
        )
        .map(|_| ())
    }

    fn open_welcome(
        self: &Rc<Self>,
        owner: &PlayerWindow,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let loaded = preference_load::load(&crate::platform::preferences_path()?)?;
        self.launch(
            NativeLaunch {
                media: InitialMedia::default(),
                owner_center: crate::platform::player_window_center(owner.window()),
                loaded,
            },
            Some(Origin::Welcome(owner.as_weak())),
        )
        .map(|_| ())
    }

    fn close(self: &Rc<Self>, target: &Rc<NativePlayer>, closed: ClosedWindow) {
        let removed = {
            let mut players = self.players.borrow_mut();
            players
                .iter()
                .position(|player| Rc::ptr_eq(player, target))
                .map(|index| players.remove(index))
        };
        if let Some(player) = removed {
            let quit = player.chrome.borrow().as_ref().is_none_or(|app| {
                app.global::<PlayerPreferences>()
                    .get_quit_when_no_opened_window()
            });
            player.finish();
            if self.players.borrow().is_empty() {
                self.pending_close
                    .set(Some(last_window_action(quit, closed, false)));
                self.finish_when_empty();
            }
        }
    }

    fn finish(&self) {
        self.quitting.set(true);
        self.pending_close.set(None);
        self.services.history.hide();
        self.services.filters.hide();
        let players = std::mem::take(&mut *self.players.borrow_mut());
        for player in players {
            player.finish();
        }
    }

    fn owns(&self, target: &Rc<NativePlayer>) -> bool {
        self.players
            .borrow()
            .iter()
            .any(|player| Rc::ptr_eq(player, target))
    }

    fn finish_when_empty(self: &Rc<Self>) {
        if self.quitting.get()
            || !self.players.borrow().is_empty()
            || self.services.history.is_visible()
            || self.services.filters.is_visible()
        {
            return;
        }
        match self.pending_close.take() {
            Some(LastWindowAction::Welcome) => {
                if let Err(error) = self.return_to_welcome() {
                    crate::diagnostics::warn!("Could not reopen the welcome window: {error}");
                    self.quit();
                }
            }
            Some(LastWindowAction::Quit) => self.quit(),
            // A tool may close while the old player's session is being released.
            Some(LastWindowAction::IdleMenu) | None => {}
        }
    }

    fn return_to_welcome(self: &Rc<Self>) -> Result<(), Box<dyn std::error::Error>> {
        let path = crate::platform::preferences_path()?;
        self.launch(
            NativeLaunch {
                media: InitialMedia::default(),
                owner_center: None,
                loaded: preference_load::load(&path)?,
            },
            Some(Origin::ReturnToWelcome),
        )?;
        Ok(())
    }

    fn quit(&self) {
        self.finish();
        // The event loop may already be ending after a shutdown error.
        let _ = slint::quit_event_loop();
    }
}

impl NativePlayer {
    fn owns_controls(&self, target: &PlayerWindow) -> bool {
        self.chrome
            .borrow()
            .as_ref()
            .is_some_and(|app| std::ptr::eq(app.window(), target.window()))
    }

    fn current_media(&self) -> Option<String> {
        let app = self.chrome.borrow();
        app.as_ref().and_then(media_open::current_media)
    }
}

#[derive(Clone)]
enum Origin {
    Player(slint::Weak<PlayerWindow>),
    Welcome(slint::Weak<PlayerWindow>),
    History(slint::Weak<crate::PlaybackHistoryWindow>),
    CommandLine,
    BrowserLink,
    ReturnToWelcome,
}

#[derive(Clone)]
pub(super) struct WindowActions {
    application: Weak<Application>,
    player: Weak<NativePlayer>,
    origin: Option<Origin>,
    managed: Option<Arc<Ticket>>,
}

impl WindowActions {
    pub(super) fn is_initial_window(&self) -> bool {
        self.origin.is_none()
    }

    pub(super) fn opens_welcome(&self) -> bool {
        matches!(
            self.origin,
            Some(Origin::Welcome(_) | Origin::ReturnToWelcome)
        )
    }

    pub(super) fn bind(&self, app: &PlayerWindow) {
        let actions = self.clone();
        let controls = app.as_weak();
        app.window().on_close_requested(move || {
            if !controls
                .upgrade()
                .is_some_and(|app| app.global::<PlayerDialogState>().get_dialog_open())
            {
                actions.close();
            }
            CloseRequestResponse::KeepWindowShown
        });

        let menus = app.global::<PlayerMenuActions>();
        let actions = self.clone();
        menus.on_close_player(move || actions.close());
        let application = self.application.clone();
        let welcome_owner = app.as_weak();
        menus.on_create_welcome_window(move || {
            let (application, owner) = (application.clone(), welcome_owner.clone());
            schedule(move || {
                if let (Some(application), Some(owner)) = (application.upgrade(), owner.upgrade())
                    && owner.get_player_manager_ready()
                    && let Err(error) = application.open_welcome(&owner)
                {
                    crate::diagnostics::report(&owner, error.to_string());
                }
            });
        });
        let application = self.application.clone();
        menus.on_quit_riina(move || {
            let application = application.clone();
            schedule(move || {
                if let Some(application) = application.upgrade() {
                    application.quit();
                }
            });
        });

        let owner = app.as_weak();
        let application = self.application.clone();
        let player = self.player.clone();
        app.on_open_player_requested(move |paths, reference, position, explicit_playlist| {
            let owner = owner.clone();
            let application = application.clone();
            let player = player.clone();
            schedule(move || {
                if let (Some(application), Some(owner), Some(player)) =
                    (application.upgrade(), owner.upgrade(), player.upgrade())
                    && owner.get_player_manager_ready()
                    && !application.quitting.get()
                    && application.owns(&player)
                {
                    let paths = paths
                        .iter()
                        .map(|path| OsString::from(path.as_str()))
                        .collect();
                    let resume_position =
                        (position.is_finite() && position >= 0.0).then_some(f64::from(position));
                    let media = InitialMedia {
                        files: paths,
                        resume_position,
                        explicit_playlist,
                        ..InitialMedia::default()
                    };
                    if let Err(error) = application.open(&owner, media, reference.as_str()) {
                        crate::diagnostics::report(&owner, error.to_string());
                    }
                }
            });
        });
        app.set_player_manager_ready(true);
    }

    pub(super) fn close(&self) {
        self.close_for(ClosedWindow::Player);
    }

    pub(super) fn close_welcome(&self) {
        self.close_for(ClosedWindow::Welcome);
    }

    fn close_for(&self, closed: ClosedWindow) {
        if let Some(player) = self.player.upgrade()
            && let Some(app) = player.chrome.borrow().as_ref()
        {
            app.set_player_manager_ready(false);
        }
        let application = self.application.clone();
        let player = self.player.clone();
        let managed = self.managed.clone();
        schedule(move || {
            if let Some(ticket) = managed {
                ticket.close();
            }
            if let (Some(application), Some(player)) = (application.upgrade(), player.upgrade()) {
                application.close(&player, closed);
            }
        });
    }

    pub(super) fn startup_failed(&self, error: Box<dyn std::error::Error>) {
        if let Some(ticket) = &self.managed {
            ticket.close();
        }
        let (Some(application), Some(player)) = (self.application.upgrade(), self.player.upgrade())
        else {
            return;
        };
        match &self.origin {
            Some(Origin::Player(origin) | Origin::Welcome(origin)) => {
                if let Some(app) = origin.upgrade() {
                    crate::diagnostics::report(&app, error.to_string());
                } else {
                    crate::diagnostics::warn!("Could not open the new player: {error}");
                }
            }
            Some(Origin::History(origin)) => {
                if let Some(window) = origin.upgrade() {
                    crate::playback_history_window::report(&window, error.to_string());
                } else {
                    crate::diagnostics::warn!(
                        "Could not open media from Playback History: {error}"
                    );
                }
            }
            Some(Origin::BrowserLink) => {
                crate::diagnostics::warn!("Could not initialize the browser-link player: {error}");
            }
            Some(Origin::ReturnToWelcome) => {
                crate::diagnostics::warn!("Could not reopen the welcome window: {error}");
            }
            None | Some(Origin::CommandLine) => {
                *application.initial_error.borrow_mut() = Some(error);
                application.quit();
                return;
            }
        }
        application.close(&player, ClosedWindow::Retired);
    }
}

fn schedule(action: impl FnOnce() + 'static) {
    if let Err(error) = slint::spawn_local(async move { action() }) {
        crate::diagnostics::warn!("Could not route the native player action: {error}");
    }
}
