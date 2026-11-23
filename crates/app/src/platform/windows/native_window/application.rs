//! One Windows event loop owns the native player frames in this application.
use super::{NativeLaunch, NativePlayer};
use crate::player_ui::InitialMedia;
use crate::plugins::plugin_players::Ticket;
use crate::{PlayerWindow, media_open, preference_load};
use riina_core::{Preferences, ThemeMaterial, sync::lock};
use riina_ui::{PlayerDialogState, PlayerMenuActions};
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
        *lock(&self.native_theme) = match preferences.effective_theme_material() {
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

pub(super) fn run(initial: NativeLaunch) -> Result<(), Box<dyn std::error::Error>> {
    let application = Rc::new(Application {
        backend: Backend::install()?,
        players: RefCell::default(),
        plugins: Rc::default(),
        initial_error: RefCell::default(),
        quitting: Cell::default(),
    });
    let owner = Rc::downgrade(&application);
    application
        .plugins
        .bind_player_launcher(move |parent, ticket| {
            let app = owner.upgrade().ok_or("The application has closed")?;
            let path = crate::platform::preferences_path().map_err(|error| error.to_string())?;
            let loaded = preference_load::load(&path).map_err(|error| error.to_string())?;
            let launch = NativeLaunch {
                media: InitialMedia::for_plugin(ticket),
                owner_center: crate::platform::player_window_center(parent.window()),
                loaded,
            };
            let player = app
                .launch(launch, Some(parent.as_weak()))
                .map_err(|error| error.to_string())?;
            let player = Rc::downgrade(&player);
            let owner = owner.clone();
            Ok(Box::new(move || {
                if let (Some(app), Some(player)) = (owner.upgrade(), player.upgrade()) {
                    app.close(&player);
                }
            }))
        });
    application.launch(initial, None)?;
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
    plugins: Rc<crate::player_ui::ApplicationPlugins>,
    initial_error: RefCell<Option<Box<dyn std::error::Error>>>,
    quitting: Cell<bool>,
}

impl Application {
    fn launch(
        self: &Rc<Self>,
        launch: NativeLaunch,
        origin: Option<slint::Weak<PlayerWindow>>,
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
            player.start(launch, self.backend.clone(), actions, self.plugins.clone())
        {
            self.close(&player);
            return Err(error);
        }
        Ok(player)
    }

    fn open(
        self: &Rc<Self>,
        owner: &PlayerWindow,
        media: Vec<OsString>,
        reference: &str,
        resume_position: Option<f64>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if media.is_empty() {
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
                media: InitialMedia {
                    files: media,
                    resume_position,
                    managed: None,
                },
                owner_center: crate::platform::player_window_center(owner.window()),
                loaded,
            },
            Some(owner.as_weak()),
        )
        .map(|_| ())
    }

    fn close(&self, target: &Rc<NativePlayer>) {
        let removed = {
            let mut players = self.players.borrow_mut();
            players
                .iter()
                .position(|player| Rc::ptr_eq(player, target))
                .map(|index| players.remove(index))
        };
        if let Some(player) = removed {
            player.finish();
            if self.players.borrow().is_empty() {
                // Windows has no application menu once the final player closes.
                self.quit();
            }
        }
    }

    fn finish(&self) {
        self.quitting.set(true);
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

    fn quit(&self) {
        self.finish();
        // The event loop may already be ending after a shutdown error.
        let _ = slint::quit_event_loop();
    }
}

impl NativePlayer {
    fn current_media(&self) -> Option<String> {
        let app = self.chrome.borrow();
        app.as_ref().and_then(media_open::current_media)
    }
}

#[derive(Clone)]
pub(super) struct WindowActions {
    application: Weak<Application>,
    player: Weak<NativePlayer>,
    origin: Option<slint::Weak<PlayerWindow>>,
    managed: Option<Arc<Ticket>>,
}

impl WindowActions {
    pub(super) fn is_initial_window(&self) -> bool {
        self.origin.is_none()
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
        menus.on_quit_riina(move || {
            let application = application.clone();
            schedule(move || {
                if let Some(application) = application.upgrade() {
                    application.quit();
                }
            });
        });

        app.set_in_process_player_manager(true);
        let owner = app.as_weak();
        let application = self.application.clone();
        let player = self.player.clone();
        app.on_open_player_requested(move |paths, reference, position| {
            let owner = owner.clone();
            let application = application.clone();
            let player = player.clone();
            schedule(move || {
                if let (Some(application), Some(owner), Some(player)) =
                    (application.upgrade(), owner.upgrade(), player.upgrade())
                    && application.owns(&player)
                {
                    let paths = paths
                        .iter()
                        .map(|path| OsString::from(path.as_str()))
                        .collect();
                    let resume =
                        (position.is_finite() && position >= 0.0).then_some(f64::from(position));
                    if let Err(error) = application.open(&owner, paths, reference.as_str(), resume)
                    {
                        crate::diagnostics::report(&owner, error.to_string());
                    }
                }
            });
        });
    }

    pub(super) fn close(&self) {
        let application = self.application.clone();
        let player = self.player.clone();
        let managed = self.managed.clone();
        schedule(move || {
            if let Some(ticket) = managed {
                ticket.close();
            }
            if let (Some(application), Some(player)) = (application.upgrade(), player.upgrade()) {
                application.close(&player);
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
        if let Some(origin) = &self.origin {
            if let Some(app) = origin.upgrade() {
                crate::diagnostics::report(&app, error.to_string());
            } else {
                crate::diagnostics::warn!("Could not open the new player: {error}");
            }
        } else {
            *application.initial_error.borrow_mut() = Some(error);
        }
        application.close(&player);
    }
}

fn schedule(action: impl FnOnce() + 'static) {
    if let Err(error) = slint::spawn_local(async move { action() }) {
        crate::diagnostics::warn!("Could not route the native player action: {error}");
    }
}
