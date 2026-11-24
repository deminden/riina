//! Own independent shared-renderer player windows for the application lifetime.
#[cfg(target_os = "macos")]
mod launch;

use crate::{
    PlayerWindow, WelcomeWindow,
    player_ui::{ApplicationServices, InitialMedia, UiSession},
    preference_load::LoadedPreferences,
};
use riina_ui::PlayerMenuActions;
use slint::{CloseRequestResponse, ComponentHandle, Model, winit_030::WinitWindowAccessor};
use std::{
    cell::RefCell,
    ffi::OsString,
    rc::{Rc, Weak},
};

struct ManagedPlayer {
    id: u64,
    session: UiSession,
    welcome: Option<WelcomeWindow>,
    app: PlayerWindow,
}

impl ManagedPlayer {
    fn hide_windows(&self) {
        self.app.invoke_window_will_close();
        self.session.prepare_to_close();
        if let Some(welcome) = &self.welcome {
            // Keep releasing the session if a window is already closing.
            let _ = welcome.hide();
        }
        if self.app.window().is_visible() {
            self.app.invoke_dismiss_font_picker();
            // Slint holds a strong reference while the player is visible.
            // Release its borrowed texture before unmapping its GL surface.
            self.app
                .global::<riina_ui::PlayerRenderedVideo>()
                .set_video_frame(slint::Image::default());
            self.app.set_engine_ready(false);
            if let Err(error) = self.app.hide() {
                crate::diagnostics::warn!("Could not hide the closing player window: {error}");
            }
        }
    }
}

#[derive(Default)]
struct ManagerState {
    next_id: u64,
    players: Vec<ManagedPlayer>,
    services: ApplicationServices,
    quitting: bool,
    #[cfg(target_os = "macos")]
    opening_documents: usize,
    #[cfg(target_os = "macos")]
    open_panel_visible: bool,
    #[cfg(target_os = "macos")]
    last_player_closed_with_tools: bool,
}

pub(crate) const fn should_quit_after_last_window_closed(
    quit_when_no_opened_window: bool,
    windows_remaining: usize,
) -> bool {
    quit_when_no_opened_window && windows_remaining == 0
}

#[derive(Clone, Default)]
pub(crate) struct PlayerManager {
    state: Rc<RefCell<ManagerState>>,
}

impl PlayerManager {
    pub(crate) fn new() -> Self {
        let manager = Self::default();
        let state = Rc::downgrade(&manager.state);
        manager
            .services()
            .plugins
            .bind_player_launcher(move |owner, ticket| {
                let application = state.upgrade().ok_or("The application has closed")?;
                let preferences =
                    crate::platform::preferences_path().map_err(|error| error.to_string())?;
                let loaded = crate::preference_load::load(&preferences)
                    .map_err(|error| error.to_string())?;
                let id = open_new_player(
                    &application,
                    owner,
                    InitialMedia::for_plugin(ticket),
                    loaded,
                )?;
                let state = state.clone();
                Ok(Box::new(move || {
                    if let Some(state) = state.upgrade() {
                        close_player(&state, id, false);
                    }
                }))
            });
        let launcher = Rc::downgrade(&manager.state);
        let open_url = launcher.clone();
        manager.services().open_url.bind(move |owner, stream| {
            let state = open_url.upgrade().ok_or("Riina is closing")?;
            let path = crate::platform::preferences_path().map_err(|error| error.to_string())?;
            let loaded = crate::preference_load::load(&path).map_err(|error| error.to_string())?;
            let media = InitialMedia {
                stream: Some(stream),
                ..InitialMedia::default()
            };
            open_new_player(&state, owner, media, loaded).map(|_| ())
        });
        let on_close = launcher.clone();
        manager.services().history.bind(
            move |window, mut media| {
                let state = launcher.upgrade().ok_or("Riina is closing")?;
                let prefs =
                    crate::platform::preferences_path().map_err(|error| error.to_string())?;
                let loaded =
                    crate::preference_load::load(&prefs).map_err(|error| error.to_string())?;
                if !loaded.preferences.resume_last_position {
                    media.resume_position = None;
                }
                open_new_player(&state, Some(window), media, loaded).map(|_| ())
            },
            move || {
                if let Some(state) = on_close.upgrade() {
                    finish_after_history_close(&state);
                }
            },
        );
        #[cfg(target_os = "macos")]
        {
            let state = Rc::downgrade(&manager.state);
            manager.services().bind_auxiliary_close(Rc::new(move || {
                if let Some(state) = state.upgrade() {
                    finish_after_auxiliary_close(&state);
                }
            }));
        }
        manager
    }

    pub(crate) fn services(&self) -> ApplicationServices {
        self.state.borrow().services.clone()
    }

    pub(crate) fn open_startup_players(
        &self,
        first: &PlayerWindow,
        players: impl Iterator<Item = InitialMedia>,
        loaded: &LoadedPreferences,
    ) -> Result<(), String> {
        for media in players {
            open_new_player(&self.state, Some(first), media, loaded.clone())?;
        }
        Ok(())
    }

    pub(crate) fn register_browser_links(
        &self,
        inbox: &crate::browser_links::Inbox,
    ) -> crate::browser_links::Registration {
        let state = Rc::downgrade(&self.state);
        inbox.register(move |link| {
            if let Some(state) = state.upgrade()
                && let Err(error) = open_browser_link(&state, link)
            {
                crate::diagnostics::warn!("Could not open browser link: {error}");
            }
        })
    }

    pub(crate) fn attach(
        &self,
        app: &PlayerWindow,
        welcome: Option<WelcomeWindow>,
        session: UiSession,
    ) -> u64 {
        let managed_welcome = welcome.as_ref().map(WelcomeWindow::clone_strong);
        let id = {
            let mut state = self.state.borrow_mut();
            state.next_id = state.next_id.wrapping_add(1).max(1);
            let id = state.next_id;
            state.players.push(ManagedPlayer {
                id,
                session,
                welcome: managed_welcome,
                app: app.clone_strong(),
            });
            id
        };

        app.set_in_process_player_manager(true);
        let state = Rc::downgrade(&self.state);
        let owner = app.as_weak();
        app.on_open_player_requested(move |paths, reference, position, explicit_playlist| {
            let Some(state) = state.upgrade() else { return };
            let Some(owner) = owner.upgrade() else { return };
            let paths = paths
                .iter()
                .map(|path| OsString::from(path.as_str()))
                .collect::<Vec<_>>();
            let resume_position =
                (position.is_finite() && position >= 0.0).then_some(f64::from(position));
            let media = InitialMedia {
                files: paths,
                resume_position,
                explicit_playlist,
                ..InitialMedia::default()
            };
            if let Err(error) = open_player(&state, &owner, media, reference.as_str()) {
                owner.set_status(error.into());
            }
        });

        let state = Rc::downgrade(&self.state);
        app.global::<PlayerMenuActions>()
            .on_close_player(move || schedule_close(state.clone(), id, false));
        let state = Rc::downgrade(&self.state);
        app.global::<PlayerMenuActions>()
            .on_quit_riina(move || schedule_quit(state.clone()));
        let state = Rc::downgrade(&self.state);
        let owner = app.as_weak();
        app.global::<PlayerMenuActions>()
            .on_create_welcome_window(move || schedule_welcome(state.clone(), Some(owner.clone())));
        let state = Rc::downgrade(&self.state);
        let owner = app.as_weak();
        app.global::<PlayerMenuActions>()
            .on_bring_all_to_front(move || {
                let (Some(state), Some(owner)) = (state.upgrade(), owner.upgrade()) else {
                    return;
                };
                bring_all_to_front(&state, &owner);
            });
        if let Some(welcome) = welcome {
            let state = Rc::downgrade(&self.state);
            welcome.window().on_close_requested(move || {
                schedule_close(state.clone(), id, true);
                CloseRequestResponse::HideWindow
            });
        }
        id
    }

    pub(crate) fn save_all(&self) -> Result<(), Box<dyn std::error::Error>> {
        for player in &self.state.borrow().players {
            player.session.save(&player.app)?;
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn prepare_application_plugins(&self, path: &std::path::Path) -> Result<(), String> {
        let plugins = self.services().plugins;
        let global = plugins.host(path)?;
        let state = Rc::downgrade(&self.state);
        plugins.bind_windowless(global.clone(), move || {
            state.upgrade().is_some_and(|state| {
                let state = state.borrow();
                !state.quitting && state.players.is_empty()
            })
        });
        global.load_enabled_globals()
    }
}

fn open_browser_link(
    state: &Rc<RefCell<ManagerState>>,
    link: crate::browser_links::Link,
) -> Result<(), String> {
    if state.borrow().quitting {
        return Err("Riina is closing".into());
    }
    let active = state.borrow().services.history.browser_destination();
    if let Some((app, display)) = &active
        && link.reuse_window(app)
        && let Some(target) = state
            .borrow()
            .players
            .iter()
            .find(|player| std::ptr::eq(player.app.window(), app.window()))
    {
        display.reveal(app).map_err(|error| error.to_string())?;
        display.focus(app);
        return target.session.open_browser_link(app, link);
    }
    let path = crate::platform::preferences_path().map_err(|error| error.to_string())?;
    let loaded = crate::preference_load::load(&path).map_err(|error| error.to_string())?;
    let owner = active.as_ref().map(|(app, _)| app);
    open_new_player(state, owner, link.initial_media(), loaded)?;
    Ok(())
}

fn bring_all_to_front(state: &Rc<RefCell<ManagerState>>, owner: &PlayerWindow) {
    let mut windows = state
        .borrow()
        .players
        .iter()
        .filter_map(|player| {
            let is_owner = std::ptr::eq(player.app.window(), owner.window());
            let display = super::surface::PlayerDisplay {
                welcome: player.welcome.as_ref().map(ComponentHandle::as_weak),
                host: None,
            };
            display
                .switchable(&player.app)
                .map(|window| (is_owner, window))
        })
        .collect::<Vec<_>>();
    // Bringing other players forward should leave the initiating player on top
    // without showing the hidden playback frame behind a Welcome window.
    windows.sort_by_key(|(is_owner, _)| *is_owner);
    for (_, window) in windows {
        window.focus();
    }
}

fn show_and_focus(window: &(impl ComponentHandle + 'static), owner: &PlayerWindow) {
    if let Err(error) = crate::platform::show_window(window) {
        crate::diagnostics::report(owner, format!("Could not show a player window: {error}"));
        return;
    }
    window.window().with_winit_window(|window| {
        window.set_minimized(false);
        window.focus_window();
    });
}

fn open_player(
    state: &Rc<RefCell<ManagerState>>,
    owner: &PlayerWindow,
    media: InitialMedia,
    reference: &str,
) -> Result<(), String> {
    if media.files.is_empty() {
        return Err("No media was selected for the new player".into());
    }
    let preferences_path =
        crate::platform::preferences_path().map_err(|error| error.to_string())?;
    let loaded =
        crate::preference_load::load(&preferences_path).map_err(|error| error.to_string())?;
    let preferences = &loaded.preferences;
    let existing = {
        let state = state.borrow();
        crate::media_open::duplicate_player(
            reference,
            preferences.allow_duplicate_players,
            state
                .players
                .iter()
                .map(|player| crate::media_open::current_media(&player.app)),
        )
        .map(|index| state.players[index].app.clone_strong())
    };
    if let Some(player) = existing {
        show_and_focus(&player, owner);
        owner.set_status("The selected media is already open.".into());
        return Ok(());
    }
    open_new_player(state, Some(owner), media, loaded)?;
    Ok(())
}

fn open_new_player<Owner: ComponentHandle + 'static>(
    state: &Rc<RefCell<ManagerState>>,
    owner: Option<&Owner>,
    media: InitialMedia,
    loaded: LoadedPreferences,
) -> Result<u64, String> {
    if state.borrow().quitting {
        return Err("Riina is closing".into());
    }
    let hidden = media.managed.is_some() && media.files.is_empty();
    let disable_animation = media
        .managed
        .as_ref()
        .is_some_and(|ticket| ticket.options.disable_window_animation);
    let app = PlayerWindow::new().map_err(|error| error.to_string())?;
    app.set_native_video(false);
    let media_geometry = crate::window_aspect::MediaGeometry::default();
    let services = state.borrow().services.clone();
    let session = crate::player_ui::configure_player(
        &app,
        None,
        media,
        None,
        loaded,
        media_geometry,
        services,
    )
    .map_err(|error| error.to_string())?;
    let manager = PlayerManager {
        state: state.clone(),
    };
    let id = manager.attach(&app, None, session);
    if !hidden && let Err(error) = show_opened_player(&app, owner, disable_animation) {
        close_player(state, id, false);
        return Err(error);
    }
    Ok(id)
}

fn show_opened_player(
    app: &PlayerWindow,
    owner: Option<&(impl ComponentHandle + 'static)>,
    disable_animation: bool,
) -> Result<(), String> {
    crate::platform::show_window(app).map_err(|error| error.to_string())?;
    if disable_animation {
        crate::platform::window_sizing::disable_plugin_window_animation(app.window());
    }
    let initial_position = app
        .global::<riina_ui::PlayerPreferences>()
        .get_initial_window_position_enabled();
    if let Some(owner) = owner {
        crate::platform::center_player_window_for_owner(app, owner, initial_position);
    } else {
        crate::platform::center_player_window(app, initial_position);
    }
    Ok(())
}

fn schedule_close(state: Weak<RefCell<ManagerState>>, id: u64, from_welcome: bool) {
    let scheduled = slint::spawn_local(async move {
        if let Some(state) = state.upgrade() {
            close_player(&state, id, from_welcome);
        }
    });
    if let Err(error) = scheduled {
        crate::diagnostics::warn!("Could not schedule player close: {error}");
    }
}

fn schedule_welcome(state: Weak<RefCell<ManagerState>>, owner: Option<slint::Weak<PlayerWindow>>) {
    if let Err(error) = slint::spawn_local(async move {
        if let Some(state) = state.upgrade() {
            let owner = owner.as_ref().and_then(slint::Weak::upgrade);
            if let Err(error) = show_welcome(&state, owner.as_ref()) {
                crate::diagnostics::warn!("Could not open a new Welcome window: {error}");
            }
        }
    }) {
        crate::diagnostics::warn!("Could not schedule a new Welcome window: {error}");
    }
}

fn schedule_quit(state: Weak<RefCell<ManagerState>>) {
    let Some(owner) = state.upgrade() else { return };
    {
        let mut owner = owner.borrow_mut();
        if std::mem::replace(&mut owner.quitting, true) {
            return;
        }
    }
    let scheduled = slint::spawn_local(async move {
        let Some(state) = state.upgrade() else { return };
        let services = state.borrow().services.clone();
        services.history.hide();
        #[cfg(target_os = "macos")]
        services.hide_auxiliary_tools();
        let players = std::mem::take(&mut state.borrow_mut().players);
        for player in players {
            if let Err(error) = player.session.save(&player.app) {
                crate::diagnostics::warn!("Could not save player state before quitting: {error}");
            }
            player.hide_windows();
        }
        // Shutdown may already have ended the event loop.
        let _ = slint::quit_event_loop();
    });
    if let Err(error) = scheduled {
        owner.borrow_mut().quitting = false;
        crate::diagnostics::warn!("Could not schedule application quit: {error}");
    }
}

fn close_player(state: &Rc<RefCell<ManagerState>>, id: u64, from_welcome: bool) {
    let removed = {
        let mut state = state.borrow_mut();
        state
            .players
            .iter()
            .position(|player| player.id == id)
            .map(|index| state.players.remove(index))
    };
    let Some(player) = removed else { return };
    #[cfg(target_os = "macos")]
    let player_was_visible = player.app.window().is_visible()
        || player
            .welcome
            .as_ref()
            .is_some_and(|welcome| welcome.window().is_visible());
    if let Err(error) = player.session.save(&player.app) {
        crate::diagnostics::report(&player.app, error.to_string());
    }
    player.hide_windows();
    let empty = state.borrow().players.is_empty();
    let quit_when_empty = should_quit_after_last_window_closed(
        player
            .app
            .global::<riina_ui::PlayerPreferences>()
            .get_quit_when_no_opened_window(),
        state.borrow().players.len(),
    );
    drop(player);
    if !empty {
        return;
    }
    #[cfg(target_os = "macos")]
    state.borrow().services.plugins.request_windowless();
    #[cfg(target_os = "macos")]
    if state.borrow().services.auxiliary_is_visible() {
        state.borrow_mut().last_player_closed_with_tools |= player_was_visible;
        crate::platform::browser_activation::show_idle_menu();
        return;
    }
    #[cfg(not(target_os = "macos"))]
    if state.borrow().services.history.is_visible() {
        return;
    }
    finish_empty_application(state, quit_when_empty, from_welcome);
}

#[cfg(target_os = "macos")]
fn finish_after_history_close(state: &Rc<RefCell<ManagerState>>) {
    finish_after_auxiliary_close(state);
}

#[cfg(target_os = "macos")]
fn finish_after_auxiliary_close(state: &Rc<RefCell<ManagerState>>) {
    let should_check_quit = {
        let mut state = state.borrow_mut();
        if state.quitting {
            return;
        }
        if !state.players.is_empty() {
            state.last_player_closed_with_tools = false;
            return;
        }
        if state.services.auxiliary_is_visible() {
            return;
        }
        std::mem::take(&mut state.last_player_closed_with_tools)
            && state.opening_documents == 0
            && !state.open_panel_visible
    };
    // An application tool opened before playback does not imply a player was closed.
    if !should_check_quit {
        crate::platform::browser_activation::show_idle_menu();
        return;
    }
    finish_after_last_player(state);
}

#[cfg(not(target_os = "macos"))]
fn finish_after_history_close(state: &Rc<RefCell<ManagerState>>) {
    if state.borrow().players.is_empty() {
        finish_after_last_player(state);
    }
}

fn finish_after_last_player(state: &Rc<RefCell<ManagerState>>) {
    let quit = crate::platform::preferences_path()
        .ok()
        .and_then(|path| crate::preference_load::load(&path).ok())
        .is_none_or(|loaded| loaded.preferences.quit_when_no_opened_window);
    finish_empty_application(state, quit, false);
}

fn finish_empty_application(state: &Rc<RefCell<ManagerState>>, quit: bool, from_welcome: bool) {
    if quit {
        state.borrow_mut().quitting = true;
        let _ = slint::quit_event_loop();
        return;
    }
    #[cfg(target_os = "macos")]
    crate::platform::browser_activation::show_idle_menu();
    #[cfg(not(target_os = "macos"))]
    if !from_welcome && let Err(error) = show_welcome(state, None) {
        crate::diagnostics::warn!("Could not reopen the welcome window: {error}");
        let _ = slint::quit_event_loop();
    }
    #[cfg(target_os = "macos")]
    let _ = from_welcome;
}

fn show_welcome(
    state: &Rc<RefCell<ManagerState>>,
    owner: Option<&PlayerWindow>,
) -> Result<(), Box<dyn std::error::Error>> {
    if state.borrow().quitting {
        return Ok(());
    }
    let app = PlayerWindow::new()?;
    app.set_native_video(false);
    let welcome = WelcomeWindow::new()?;
    crate::appearance_preferences::install_system_typography(&welcome);
    let loaded = crate::preference_load::load(&crate::platform::preferences_path()?)?;
    let media_geometry = crate::window_aspect::MediaGeometry::default();
    let services = state.borrow().services.clone();
    let session = crate::player_ui::configure_player(
        &app,
        None,
        InitialMedia::default(),
        Some(&welcome),
        loaded,
        media_geometry,
        services,
    )?;
    let manager = PlayerManager {
        state: state.clone(),
    };
    let id = manager.attach(&app, Some(welcome.clone_strong()), session);
    crate::platform::center_auxiliary_window(&welcome, owner.unwrap_or(&app));
    if let Err(error) = crate::platform::show_window(&welcome) {
        close_player(state, id, true);
        return Err(error.into());
    }
    crate::platform::enable_welcome_mica(&welcome);
    manager
        .services()
        .present_release_highlights(&app, Some(&welcome));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::should_quit_after_last_window_closed;

    #[test]
    fn application_quits_only_when_requested_after_the_last_window_closes() {
        assert!(should_quit_after_last_window_closed(true, 0));
        assert!(!should_quit_after_last_window_closed(false, 0));
        assert!(!should_quit_after_last_window_closed(true, 1));
    }
}
